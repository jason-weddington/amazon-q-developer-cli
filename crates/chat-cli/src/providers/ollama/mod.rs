use async_trait::async_trait;
use eyre::Result;
use tracing::{debug, warn};

use crate::api_client::model::ConversationState;
use crate::api_client::ApiClientError;
use crate::providers::MessageProvider;

pub use self::client::OllamaClient;
pub use self::types::*;

mod client;
mod types;

/// Ollama provider implementation
pub struct OllamaProvider {
    client: OllamaClient,
    base_url: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    pub fn new(base_url: String) -> Self {
        let client = OllamaClient::new(base_url.clone());
        Self { client, base_url }
    }
    
    /// Create with default localhost URL
    pub fn new_default() -> Self {
        Self::new("http://localhost:11434".to_string())
    }
    
    /// Get the Ollama client
    pub fn client(&self) -> &OllamaClient {
        &self.client
    }
    
    /// Convert AWS ConversationState to Ollama message format
    fn convert_conversation_to_ollama(&self, conversation: ConversationState) -> Result<(Vec<OllamaMessage>, String), ApiClientError> {
        let mut ollama_messages = Vec::new();
        
        // Convert conversation history
        if let Some(history) = conversation.history {
            for chat_message in history.iter() {
                match chat_message {
                    crate::api_client::model::ChatMessage::UserInputMessage(user_msg) => {
                        ollama_messages.push(OllamaMessage {
                            role: "user".to_string(),
                            content: Some(user_msg.content.clone()),
                            thinking: None,
                            images: self.convert_images_to_ollama(user_msg.images.clone())?,
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    },
                    crate::api_client::model::ChatMessage::AssistantResponseMessage(assistant_msg) => {
                        // Convert tool uses to Ollama tool calls
                        if let Some(tool_uses) = &assistant_msg.tool_uses {
                            let ollama_tool_calls = tool_uses.iter().map(|tool_use| {
                                // Convert FigDocument to JSON string for arguments
                                let input_json = serde_json::to_string(&tool_use.input)
                                    .unwrap_or_else(|_| "{}".to_string());
                                
                                OllamaToolCall {
                                    id: Some(tool_use.tool_use_id.clone()),
                                    tool_type: Some("function".to_string()),
                                    function: OllamaFunctionCall {
                                        name: tool_use.name.clone(),
                                        arguments: serde_json::from_str(&input_json)
                                            .unwrap_or_else(|_| serde_json::Value::String(input_json)),
                                    },
                                }
                            }).collect();
                            
                            ollama_messages.push(OllamaMessage {
                                role: "assistant".to_string(),
                                content: if assistant_msg.content.is_empty() { None } else { Some(assistant_msg.content.clone()) },
                                thinking: None,
                                images: None, // Assistants don't send images in Ollama
                                tool_calls: Some(ollama_tool_calls),
                                tool_call_id: None,
                            });
                        } else {
                            // Regular assistant message without tools
                            ollama_messages.push(OllamaMessage {
                                role: "assistant".to_string(),
                                content: Some(assistant_msg.content.clone()),
                                thinking: None,
                                images: None, // Assistants don't send images in Ollama
                                tool_calls: None,
                                tool_call_id: None,
                            });
                        }
                    },
                }
            }
        }
        
        // Add current user message with tool results
        let user_content = conversation.user_input_message.content.clone();
        let current_message = OllamaMessage {
            role: "user".to_string(),
            content: Some(user_content.clone()),
            thinking: None,
            images: self.convert_images_to_ollama(conversation.user_input_message.images)?,
            tool_calls: None,
            tool_call_id: None,
        };
        
        // Check for tool results in user message context and add them BEFORE the user message
        if let Some(context) = &conversation.user_input_message.user_input_message_context {
            if let Some(tool_results) = &context.tool_results {
                for tool_result in tool_results.iter() {
                    let content = if !tool_result.content.is_empty() {
                        match &tool_result.content[0] {
                            crate::api_client::model::ToolResultContentBlock::Text(text) => {
                                // Handle empty text content from AWS tools - this is our key fix
                                if text.is_empty() {
                                    match tool_result.status {
                                        crate::api_client::model::ToolResultStatus::Success => {
                                            "Tool execution completed successfully".to_string()
                                        },
                                        crate::api_client::model::ToolResultStatus::Error => {
                                            "Tool execution failed".to_string()
                                        },
                                    }
                                } else {
                                    text.clone()
                                }
                            },
                            crate::api_client::model::ToolResultContentBlock::Json(json) => {
                                format!("{:?}", json)
                            },
                        }
                    } else {
                        "Tool execution completed".to_string() // Fallback for empty results
                    };
                    
                    ollama_messages.push(OllamaMessage {
                        role: "tool".to_string(),
                        content: Some(content),
                        thinking: None,
                        images: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_result.tool_use_id.clone()),
                    });
                }
            }
        }
        
        // Add the current user message after tool results
        ollama_messages.push(current_message);
        
        // Determine model to use
        let model = conversation.user_input_message.model_id
            .unwrap_or_else(|| "gpt-oss:20b".to_string()); // Default model
        
        Ok((ollama_messages, model))
    }
    
    /// Convert AWS ImageBlock format to Ollama base64 format
    fn convert_images_to_ollama(&self, aws_images: Option<Vec<crate::api_client::model::ImageBlock>>) -> Result<Option<Vec<String>>, ApiClientError> {
        match aws_images {
            Some(images) => {
                let mut ollama_images = Vec::new();
                for image in images {
                    // Convert AWS ImageBlock to Ollama base64 format
                    let base64_image = match image.source {
                        crate::api_client::model::ImageSource::Bytes(bytes) => {
                            use base64::{Engine as _, engine::general_purpose};
                            let base64_data = general_purpose::STANDARD.encode(&bytes);
                            let format_str = match image.format {
                                crate::api_client::model::ImageFormat::Png => "png",
                                crate::api_client::model::ImageFormat::Jpeg => "jpeg",
                                crate::api_client::model::ImageFormat::Gif => "gif",
                                crate::api_client::model::ImageFormat::Webp => "webp",
                            };
                            format!("data:image/{};base64,{}", format_str, base64_data)
                        },
                        crate::api_client::model::ImageSource::Unknown => {
                            warn!("Unknown image source, skipping image");
                            continue;
                        }
                    };
                    ollama_images.push(base64_image);
                }
                Ok(if ollama_images.is_empty() { None } else { Some(ollama_images) })
            },
            None => Ok(None),
        }
    }
    
    /// Get tools based on model capabilities
    async fn get_ollama_tools(&self, model: &str) -> Result<Vec<OllamaTool>, ApiClientError> {
        let mut tools = Vec::new();
        
        // Always include core functional tools
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "fs_read".to_string(),
                description: "Read files, directories and images. Always provide an 'operations' array.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operations": {
                            "type": "array",
                            "description": "Array of operations to execute",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "mode": {
                                        "type": "string", 
                                        "enum": ["Line", "Directory", "Search", "Image"],
                                        "description": "The operation mode to run in"
                                    },
                                    "path": {
                                        "type": "string",
                                        "description": "Path to the file or directory"
                                    },
                                    "start_line": {
                                        "type": "integer", 
                                        "default": 1,
                                        "description": "Starting line number (for Line mode)"
                                    },
                                    "end_line": {
                                        "type": "integer", 
                                        "default": -1,
                                        "description": "Ending line number (for Line mode)"
                                    },
                                    "pattern": {
                                        "type": "string",
                                        "description": "Pattern to search for (for Search mode)"
                                    }
                                },
                                "required": ["mode", "path"]
                            }
                        },
                        "summary": {
                            "type": "string",
                            "description": "Optional description of the purpose of this operation"
                        }
                    },
                    "required": ["operations"]
                }),
            },
        });
        
        // execute_bash tool
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: if cfg!(windows) { "execute_cmd" } else { "execute_bash" }.to_string(),
                description: "Execute shell commands on the user's system".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string", 
                            "description": "Shell command to execute"
                        },
                        "summary": {
                            "type": "string", 
                            "description": "Brief explanation of what the command does"
                        }
                    },
                    "required": ["command"]
                }),
            },
        });
        
        // fs_write tool
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "fs_write".to_string(),
                description: "Create and edit files. Use command='create' to create new files, 'str_replace' to replace text, 'insert' to insert at specific lines, 'append' to add to end.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string", 
                            "enum": ["create", "str_replace", "insert", "append"],
                            "description": "The command to run. Use 'create' to create new files."
                        },
                        "path": {
                            "type": "string",
                            "description": "Absolute path to file or directory"
                        },
                        "file_text": {
                            "type": "string",
                            "description": "Content of the file to be created (for create command)"
                        },
                        "old_str": {
                            "type": "string",
                            "description": "String to replace (for str_replace command)"
                        },
                        "new_str": {
                            "type": "string",
                            "description": "New string (for str_replace and insert commands)"
                        },
                        "insert_line": {
                            "type": "integer",
                            "description": "Line number to insert after (for insert command)"
                        },
                        "summary": {
                            "type": "string",
                            "description": "Brief explanation of what the file change does"
                        }
                    },
                    "required": ["command", "path"]
                }),
            },
        });
        
        // use_aws tool
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "use_aws".to_string(),
                description: "Make AWS CLI api calls".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "service_name": {
                            "type": "string",
                            "description": "The name of the AWS service"
                        },
                        "operation_name": {
                            "type": "string",
                            "description": "The name of the operation to perform"
                        },
                        "parameters": {
                            "type": "object",
                            "description": "The parameters for the operation"
                        },
                        "region": {
                            "type": "string",
                            "description": "Region name for calling the operation on AWS"
                        },
                        "label": {
                            "type": "string",
                            "description": "Human readable description of the api being called"
                        }
                    },
                    "required": ["service_name", "operation_name", "region", "label"]
                }),
            },
        });
        
        // Check if model supports tools via capability detection
        // For now, skip thinking tool - will add capability detection later
        // TODO: Add thinking tool based on model capabilities and settings
        debug!("Model {} tool capabilities will be checked in future implementation", model);
        
        Ok(tools)
    }
}

#[async_trait]
impl MessageProvider for OllamaProvider {
    async fn send_message(&self, conversation: ConversationState) -> Result<crate::providers::ProviderResponse, ApiClientError> {
        let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
        let ollama_tools = self.get_ollama_tools(&model).await?;
        
        let request = OllamaChatRequest {
            model,
            messages: ollama_messages,
            tools: Some(ollama_tools), // Include tools in request
            stream: Some(true), // Enable streaming for proper tool call handling
            format: None,
            options: None,
            keep_alive: None,
        };
        
        // Use streaming chat for proper tool call and result handling
        let stream_receiver = self.client.chat_stream(request).await?;
        Ok(crate::providers::ProviderResponse::Streaming(Box::new(stream_receiver)))
    }
    
    fn provider_name(&self) -> &'static str {
        "ollama"
    }
    
    fn requires_auth(&self) -> bool {
        false // Ollama doesn't require authentication
    }
    
    fn supports_streaming(&self) -> bool {
        true
    }
    
    fn supports_tools(&self) -> bool {
        true // Most Ollama models support tools
    }
    
    async fn list_models(&self) -> Result<Vec<String>, ApiClientError> {
        let response = self.client.list_models().await?;
        Ok(response.models.into_iter().map(|m| m.name).collect())
    }
    
    async fn test_connection(&self) -> Result<bool, ApiClientError> {
        Ok(self.client.health_check().await.unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_client::model::*;

    fn create_test_provider() -> OllamaProvider {
        OllamaProvider {
            client: OllamaClient::new("http://localhost:11434".to_string()),
            base_url: "http://localhost:11434".to_string(),
        }
    }

    #[test]
    fn test_convert_conversation_with_tool_uses() {
        let provider = create_test_provider();
        
        // Create conversation with assistant message containing tool uses
        let tool_use = ToolUse {
            tool_use_id: "test_tool_id".to_string(),
            name: "fs_write".to_string(),
            input: {
                use aws_smithy_types::Document as AwsDocument;
                use std::collections::HashMap;
                let mut map = HashMap::new();
                map.insert("command".to_string(), AwsDocument::String("create".to_string()));
                map.insert("path".to_string(), AwsDocument::String("test.txt".to_string()));
                map.insert("file_text".to_string(), AwsDocument::String("Hello World".to_string()));
                FigDocument::from(AwsDocument::Object(map))
            },
        };
        
        let assistant_msg = AssistantResponseMessage {
            message_id: Some("msg_123".to_string()),
            content: "I'll create the file for you.".to_string(),
            tool_uses: Some(vec![tool_use]),
        };
        
        let conversation = ConversationState {
            conversation_id: Some("conv_123".to_string()),
            user_input_message: UserInputMessage {
                content: "What did you create?".to_string(),
                user_input_message_context: None,
                user_intent: None,
                images: None,
                model_id: Some("gpt-oss:20b".to_string()),
            },
            history: Some(vec![
                ChatMessage::UserInputMessage(UserInputMessage {
                    content: "Create a file called test.txt".to_string(),
                    user_input_message_context: None,
                    user_intent: None,
                    images: None,
                    model_id: None,
                }),
                ChatMessage::AssistantResponseMessage(assistant_msg),
            ]),
        };
        
        let (messages, model) = provider.convert_conversation_to_ollama(conversation).unwrap();
        
        // Should have: user → assistant (with tool_calls) → new user
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, Some("Create a file called test.txt".to_string()));
        
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, Some("I'll create the file for you.".to_string()));
        assert!(messages[1].tool_calls.is_some());
        let tool_calls = messages[1].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, Some("test_tool_id".to_string()));
        assert_eq!(tool_calls[0].function.name, "fs_write");
        
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, Some("What did you create?".to_string()));
        assert_eq!(model, "gpt-oss:20b");
    }

    #[test]
    fn test_convert_conversation_with_tool_results() {
        let provider = create_test_provider();
        
        // Create conversation with tool results in user message context
        let tool_result = ToolResult {
            tool_use_id: "test_tool_id".to_string(),
            content: vec![ToolResultContentBlock::Text("File created successfully: test.txt".to_string())],
            status: ToolResultStatus::Success,
        };
        
        let user_context = UserInputMessageContext {
            env_state: None,
            git_state: None,
            tool_results: Some(vec![tool_result]),
            tools: None,
        };
        
        let conversation = ConversationState {
            conversation_id: Some("conv_123".to_string()),
            user_input_message: UserInputMessage {
                content: "What did you create?".to_string(),
                user_input_message_context: Some(user_context),
                user_intent: None,
                images: None,
                model_id: Some("gpt-oss:20b".to_string()),
            },
            history: None,
        };
        
        let (messages, _) = provider.convert_conversation_to_ollama(conversation).unwrap();
        
        // Should have: tool result → user message
        assert_eq!(messages.len(), 2);
        
        // First message should be tool result
        assert_eq!(messages[0].role, "tool");
        assert_eq!(messages[0].content, Some("File created successfully: test.txt".to_string()));
        assert_eq!(messages[0].tool_call_id, Some("test_tool_id".to_string()));
        
        // Second message should be user message
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, Some("What did you create?".to_string()));
    }

    #[test]
    fn test_convert_conversation_full_tool_flow() {
        let provider = create_test_provider();
        
        // Create a complete tool flow: user → assistant with tool → tool result → user follow-up
        let tool_use = ToolUse {
            tool_use_id: "test_tool_id".to_string(),
            name: "fs_write".to_string(),
            input: {
                use aws_smithy_types::Document as AwsDocument;
                use std::collections::HashMap;
                let mut map = HashMap::new();
                map.insert("command".to_string(), AwsDocument::String("create".to_string()));
                map.insert("path".to_string(), AwsDocument::String("test.txt".to_string()));
                FigDocument::from(AwsDocument::Object(map))
            },
        };
        
        let assistant_msg = AssistantResponseMessage {
            message_id: Some("msg_123".to_string()),
            content: "".to_string(), // Empty content when tool calls are present
            tool_uses: Some(vec![tool_use]),
        };
        
        let tool_result = ToolResult {
            tool_use_id: "test_tool_id".to_string(),
            content: vec![ToolResultContentBlock::Text("File created: test.txt".to_string())],
            status: ToolResultStatus::Success,
        };
        
        let user_context = UserInputMessageContext {
            env_state: None,
            git_state: None,
            tool_results: Some(vec![tool_result]),
            tools: None,
        };
        
        let conversation = ConversationState {
            conversation_id: Some("conv_123".to_string()),
            user_input_message: UserInputMessage {
                content: "Great! What's in the file?".to_string(),
                user_input_message_context: Some(user_context),
                user_intent: None,
                images: None,
                model_id: Some("gpt-oss:20b".to_string()),
            },
            history: Some(vec![
                ChatMessage::UserInputMessage(UserInputMessage {
                    content: "Create a file called test.txt".to_string(),
                    user_input_message_context: None,
                    user_intent: None,
                    images: None,
                    model_id: None,
                }),
                ChatMessage::AssistantResponseMessage(assistant_msg),
            ]),
        };
        
        let (messages, _) = provider.convert_conversation_to_ollama(conversation).unwrap();
        
        // Should have: user → assistant (with tool_calls) → tool result → user follow-up
        assert_eq!(messages.len(), 4);
        
        // Message 1: Initial user request
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, Some("Create a file called test.txt".to_string()));
        
        // Message 2: Assistant with tool call
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1].tool_calls.is_some());
        assert_eq!(messages[1].content, None); // Empty content when tool calls present
        
        // Message 3: Tool result
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[2].content, Some("File created: test.txt".to_string()));
        assert_eq!(messages[2].tool_call_id, Some("test_tool_id".to_string()));
        
        // Message 4: User follow-up
        assert_eq!(messages[3].role, "user");
        assert_eq!(messages[3].content, Some("Great! What's in the file?".to_string()));
    }
}
