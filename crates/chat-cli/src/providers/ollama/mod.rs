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
            for chat_message in history {
                match chat_message {
                    crate::api_client::model::ChatMessage::UserInputMessage(user_msg) => {
                        ollama_messages.push(OllamaMessage {
                            role: "user".to_string(),
                            content: Some(user_msg.content),
                            images: self.convert_images_to_ollama(user_msg.images)?,
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    },
                    crate::api_client::model::ChatMessage::AssistantResponseMessage(assistant_msg) => {
                        // TODO: Handle tool calls in assistant messages
                        // For now, just convert as regular assistant message
                        ollama_messages.push(OllamaMessage {
                            role: "assistant".to_string(),
                            content: Some(assistant_msg.content),
                            images: None, // Assistants don't send images in Ollama
                            tool_calls: None, // TODO: Convert tool uses to tool calls
                            tool_call_id: None,
                        });
                    },
                }
            }
        }
        
        // Add current user message
        let current_message = OllamaMessage {
            role: "user".to_string(),
            content: Some(conversation.user_input_message.content),
            images: self.convert_images_to_ollama(conversation.user_input_message.images)?,
            tool_calls: None,
            tool_call_id: None,
        };
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
                description: "Create and edit files".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string", 
                            "enum": ["create", "str_replace", "insert", "append"],
                            "description": "The command to run"
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
            stream: Some(true), // Enable streaming
            format: None,
            options: None,
            keep_alive: None,
        };
        
        let stream_receiver = self.client.chat_stream(request).await?;
        Ok(crate::providers::ProviderResponse::OllamaStreaming(stream_receiver))
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
