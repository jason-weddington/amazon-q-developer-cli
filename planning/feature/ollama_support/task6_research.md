# Task 6 Research: Implement Tool Call Mapping

## 🔍 **Current Tool System Analysis**

### **Built-in Tools Available**
Based on codebase analysis, Amazon Q CLI has these built-in tools:

1. **`fs_read`** - Read files and directories (auto-approved)
2. **`fs_write`** - Write/modify files
3. **`execute_bash`** (Unix) / **`execute_cmd`** (Windows) - Execute shell commands  
4. **`use_aws`** - Make AWS CLI calls
5. **`gh_issue`** - Report GitHub issues
6. **`knowledge`** - Knowledge base queries
7. **`thinking`** - Internal reasoning (prerelease)

### **Current AWS Tool Flow**

#### **1. Tool Definitions**
Tools are defined as Rust structs with serde serialization:
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct FsRead {
    pub operations: Vec<FsReadOperation>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode")]
pub enum FsReadOperation {
    Line(FsLine),
    Directory(FsDirectory), 
    Search(FsSearch),
    Image(FsImage),
}
```

#### **2. Tool Call Detection**
AWS responses include tool call events in the streaming response:
```rust
ChatResponseStream::ToolUseEvent {
    tool_use_id: String,
    name: String,
    input: Option<String>,
    stop: Option<bool>,
}
```

#### **3. Tool Execution Pipeline**
1. **Detection**: `ToolUseEvent` triggers `ResponseEvent::ToolUseStart`
2. **Parsing**: Multiple `ToolUseEvent`s build up the tool arguments JSON
3. **Deserialization**: JSON arguments are parsed into tool structs
4. **Execution**: Tool's `invoke()` method is called
5. **Response**: Tool output is sent back to continue conversation

#### **4. Tool Registration**
```rust
pub const NATIVE_TOOLS: [&str; 7] = [
    "fs_read",
    "fs_write", 
    "execute_bash", // or "execute_cmd" on Windows
    "use_aws",
    "gh_issue",
    "knowledge",
    "thinking",
];

pub enum Tool {
    FsRead(FsRead),
    FsWrite(FsWrite),
    ExecuteCommand(ExecuteCommand),
    UseAws(UseAws),
    GhIssue(GhIssue),
    Knowledge(Knowledge),
    Thinking(Thinking),
    Custom(CustomTool), // MCP server tools
}
```

## 🌊 **Ollama Tool Support Research**

### **Ollama Tool Call Format**
Based on Ollama documentation, tool calls work similarly to OpenAI:

#### **Request with Tools**
```json
{
  "model": "llama3.1",
  "messages": [
    {"role": "user", "content": "Read the file README.md"}
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "fs_read",
        "description": "Read files, directories and images",
        "parameters": {
          "type": "object",
          "properties": {
            "operations": {
              "type": "array",
              "items": {
                "type": "object",
                "properties": {
                  "mode": {"type": "string", "enum": ["Line", "Directory", "Search", "Image"]},
                  "path": {"type": "string"}
                },
                "required": ["mode", "path"]
              }
            }
          },
          "required": ["operations"]
        }
      }
    }
  ]
}
```

#### **Response with Tool Call**
```json
{
  "model": "llama3.1",
  "message": {
    "role": "assistant",
    "content": "",
    "tool_calls": [
      {
        "id": "call_123",
        "type": "function",
        "function": {
          "name": "fs_read",
          "arguments": "{\"operations\":[{\"mode\":\"Line\",\"path\":\"README.md\"}]}"
        }
      }
    ]
  },
  "done": false
}
```

#### **Follow-up with Tool Result**
```json
{
  "model": "llama3.1", 
  "messages": [
    {"role": "user", "content": "Read the file README.md"},
    {
      "role": "assistant",
      "tool_calls": [
        {
          "id": "call_123",
          "type": "function", 
          "function": {
            "name": "fs_read",
            "arguments": "{\"operations\":[{\"mode\":\"Line\",\"path\":\"README.md\"}]}"
          }
        }
      ]
    },
    {
      "role": "tool",
      "content": "# Amazon Q CLI\n\nThis is the README content...",
      "tool_call_id": "call_123"
    }
  ]
}
```

## 📋 **Task 6 Implementation Plan**

### **Phase 1: Add Tool Types to Ollama Structures**

#### **1.1 Update OllamaMessage for Tool Calls**
```rust
// In crates/chat-cli/src/api_client/ollama.rs

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,    // "system", "user", "assistant", "tool"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>, // For tool result messages
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaFunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaTool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: OllamaFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}
```

#### **1.2 Update OllamaChatRequest**
```rust
#[derive(Serialize, Debug)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OllamaTool>>, // Add tools support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}
```

### **Phase 2: Tool Definition Mapping**

#### **2.1 Create Tool Definition Generator**
```rust
impl ApiClient {
    fn get_ollama_tools(&self) -> Result<Vec<OllamaTool>, ApiClientError> {
        let mut tools = Vec::new();
        
        // fs_read tool
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
        
        Ok(tools)
    }
}
```

### **Phase 3: Tool Call Detection in Streaming**

#### **3.1 Update OllamaStreamReceiver**
```rust
impl OllamaStreamReceiver {
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        if self.ended {
            return Ok(None);
        }
        
        match StreamExt::next(&mut self.stream).await {
            Some(Ok(ollama_response)) => {
                if ollama_response.done {
                    self.ended = true;
                    Ok(None)
                } else if let Some(tool_calls) = &ollama_response.message.tool_calls {
                    // Convert tool calls to ToolUseEvent
                    if let Some(tool_call) = tool_calls.first() {
                        Ok(Some(ChatResponseStream::ToolUseEvent {
                            tool_use_id: tool_call.id.clone(),
                            name: tool_call.function.name.clone(),
                            input: Some(tool_call.function.arguments.clone()),
                            stop: Some(false),
                        }))
                    } else {
                        Box::pin(self.recv()).await
                    }
                } else if let Some(content) = &ollama_response.message.content {
                    if !content.is_empty() {
                        Ok(Some(ChatResponseStream::AssistantResponseEvent {
                            content: content.clone(),
                        }))
                    } else {
                        Box::pin(self.recv()).await
                    }
                } else {
                    Box::pin(self.recv()).await
                }
            },
            Some(Err(e)) => Err(e.into()),
            None => {
                self.ended = true;
                Ok(None)
            }
        }
    }
}
```

### **Phase 4: Tool Integration with Requests**

#### **4.1 Update send_message_ollama_internal**
```rust
async fn send_message_ollama_internal(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
    let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
    let ollama_tools = self.get_ollama_tools()?;
    
    match &self.ollama_client {
        Some(client) => {
            let request = OllamaChatRequest {
                model,
                messages: ollama_messages,
                tools: Some(ollama_tools), // Include tools in request
                stream: Some(true),
                format: None,
                options: None,
                keep_alive: None,
            };
            
            let stream_receiver = client.chat_stream(request).await?;
            Ok(SendMessageOutput::OllamaStreaming(stream_receiver))
        },
        None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
    }
}
```

### **Phase 5: Tool Result Handling**

#### **5.1 Update Message Conversion for Tool Results**
```rust
fn convert_conversation_to_ollama(&self, conversation: ConversationState) -> Result<(Vec<OllamaMessage>, String), ApiClientError> {
    let mut ollama_messages = Vec::new();
    
    // Convert conversation history (including tool results)
    if let Some(history) = conversation.history {
        for chat_message in history {
            match chat_message {
                ChatMessage::UserInputMessage(user_msg) => {
                    ollama_messages.push(OllamaMessage {
                        role: "user".to_string(),
                        content: Some(user_msg.content),
                        images: self.convert_images_to_ollama(user_msg.images)?,
                        tool_calls: None,
                        tool_call_id: None,
                    });
                },
                ChatMessage::AssistantResponseMessage(assistant_msg) => {
                    // Check if this message contains tool calls
                    if let Some(tool_uses) = &assistant_msg.tool_uses {
                        // Convert tool uses to Ollama tool calls
                        let tool_calls: Vec<OllamaToolCall> = tool_uses.iter().map(|tool_use| {
                            OllamaToolCall {
                                id: tool_use.tool_use_id.clone(),
                                tool_type: "function".to_string(),
                                function: OllamaFunctionCall {
                                    name: tool_use.name.clone(),
                                    arguments: tool_use.input.clone(),
                                },
                            }
                        }).collect();
                        
                        ollama_messages.push(OllamaMessage {
                            role: "assistant".to_string(),
                            content: if assistant_msg.content.is_empty() { None } else { Some(assistant_msg.content.clone()) },
                            images: None,
                            tool_calls: Some(tool_calls),
                            tool_call_id: None,
                        });
                        
                        // Add tool results as separate messages
                        if let Some(tool_results) = &conversation.user_input_message.user_input_message_context
                            .as_ref().and_then(|ctx| ctx.tool_results.as_ref()) {
                            for tool_result in tool_results {
                                let content = match &tool_result.content[0] {
                                    ToolResultContentBlock::Text(text) => text.clone(),
                                    ToolResultContentBlock::Json(json) => json.to_string(),
                                };
                                
                                ollama_messages.push(OllamaMessage {
                                    role: "tool".to_string(),
                                    content: Some(content),
                                    images: None,
                                    tool_calls: None,
                                    tool_call_id: Some(tool_result.tool_use_id.clone()),
                                });
                            }
                        }
                    } else {
                        // Regular assistant message
                        ollama_messages.push(OllamaMessage {
                            role: "assistant".to_string(),
                            content: Some(assistant_msg.content),
                            images: None,
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                },
            }
        }
    }
    
    // Add current user message
    ollama_messages.push(OllamaMessage {
        role: "user".to_string(),
        content: Some(conversation.user_input_message.content),
        images: self.convert_images_to_ollama(conversation.user_input_message.images)?,
        tool_calls: None,
        tool_call_id: None,
    });
    
    let model = conversation.user_input_message.model_id
        .unwrap_or_else(|| "llama3.1".to_string()); // Use tool-capable model by default
    
    Ok((ollama_messages, model))
}
```

## 🧪 **Testing Strategy**

### **Unit Tests**
```rust
#[tokio::test]
async fn test_tool_definition_generation() {
    let api_client = create_test_api_client();
    let tools = api_client.get_ollama_tools().unwrap();
    
    assert!(tools.iter().any(|t| t.function.name == "fs_read"));
    assert!(tools.iter().any(|t| t.function.name == "execute_bash" || t.function.name == "execute_cmd"));
    assert!(tools.iter().any(|t| t.function.name == "fs_write"));
    assert!(tools.iter().any(|t| t.function.name == "use_aws"));
    
    // Verify tool schemas are valid JSON
    for tool in tools {
        assert!(tool.function.parameters.is_object());
        assert!(tool.function.parameters["type"] == "object");
        assert!(tool.function.parameters["properties"].is_object());
    }
}

#[tokio::test]
async fn test_tool_call_detection() {
    let ollama_response = OllamaChatResponse {
        model: "llama3.1".to_string(),
        message: OllamaMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![OllamaToolCall {
                id: "call_123".to_string(),
                tool_type: "function".to_string(),
                function: OllamaFunctionCall {
                    name: "fs_read".to_string(),
                    arguments: r#"{"operations":[{"mode":"Line","path":"README.md"}]}"#.to_string(),
                },
            }]),
            images: None,
            tool_call_id: None,
        },
        done: false,
        // ... other fields
    };
    
    // Test conversion to ChatResponseStream::ToolUseEvent
    // This would be tested in the streaming receiver
}

#[test]
fn test_conversation_with_tool_results() {
    // Test that tool results are properly converted to Ollama format
    let conversation = create_conversation_with_tool_results();
    let api_client = create_test_api_client();
    
    let (messages, _model) = api_client.convert_conversation_to_ollama(conversation).unwrap();
    
    // Should have user message, assistant with tool calls, tool result, and new user message
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    assert!(messages[1].tool_calls.is_some());
    assert_eq!(messages[2].role, "tool");
    assert!(messages[2].tool_call_id.is_some());
    assert_eq!(messages[3].role, "user");
}
```

### **Integration Tests**
```bash
# Test tool usage with Ollama
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Then ask: "Read the README.md file"
# Should see:
# 1. Tool call detection
# 2. Tool execution (fs_read)
# 3. File content displayed
# 4. Continued conversation with context

# Test command execution
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Then ask: "List the files in the current directory"
# Should see:
# 1. execute_bash tool call
# 2. Command execution (ls or dir)
# 3. Directory listing
# 4. Follow-up questions work
```

## 🎯 **Success Criteria**

After Task 6 implementation:
- [ ] `Q_CLI_MODEL_PROVIDER=ollama q chat` supports all built-in tools
- [ ] Tool calls are detected in streaming responses
- [ ] Tools execute correctly and return results
- [ ] Tool results are included in conversation history
- [ ] Multi-turn tool conversations work
- [ ] Error handling for unsupported models
- [ ] All existing functionality unchanged
- [ ] Unit tests pass for tool mapping
- [ ] Integration tests demonstrate working tools

## 🚧 **Implementation Challenges**

### **1. Model Compatibility**
- Not all Ollama models support tool calls (need llama3.1+, mistral, etc.)
- Graceful degradation for models without tool support
- Clear error messages when tools aren't available

### **2. Tool Schema Mapping**
- AWS tool schemas → OpenAI-compatible JSON schemas
- Handle complex nested structures (like FsReadOperation enum)
- Maintain parameter validation and requirements

### **3. Conversation Flow**
- Tool calls interrupt normal message flow
- Multiple tool calls in sequence
- Tool results must be properly threaded back into conversation

### **4. Error Handling**
- Tool execution failures
- Invalid tool arguments from model
- Network issues during tool execution

### **5. Thinking Tool Compatibility**
The `thinking` tool presents a special challenge for Ollama integration:

#### **What is the Thinking Tool?**
- **Purpose**: Internal reasoning mechanism for complex multi-step problems
- **Behavior**: Model uses it to "think out loud" and show reasoning process
- **Implementation**: Displays thought content to user, returns empty result
- **Conditional**: Only enabled when `chat.enableThinking` setting is true
- **Schema**: Simple `{"thought": "string"}` parameter

#### **Ollama Model Compatibility Issues**
```rust
// Current thinking tool definition
"thinking": {
    "description": "Thinking is an internal reasoning mechanism improving the quality of complex tasks by breaking their atomic actions down; use it specifically for multi-step problems requiring step-by-step dependencies, reasoning through multiple constraints, synthesizing results from previous tool calls, planning intricate sequences of actions, troubleshooting complex errors, or making decisions involving multiple trade-offs.",
    "input_schema": {
        "type": "object",
        "properties": {
            "thought": {
                "type": "string",
                "description": "A reflective note or intermediate reasoning step..."
            }
        },
        "required": ["thought"]
    }
}
```

**Problem**: Most Ollama models are **not reasoning models** and don't understand the concept of "thinking" as a tool. They may:
- Use the thinking tool inappropriately for simple tasks
- Not understand when to use it for complex reasoning
- Generate confusing or unnecessary "thoughts"
- Expect the tool to return meaningful results (but it returns empty)

#### **Perfect Solution: Ollama's Built-in Capability Detection**

Based on the official Ollama API documentation and source code, Ollama provides **built-in capability detection** via the `/api/show` endpoint:

```rust
// Ollama defines these capabilities in types/model/capability.go
pub enum OllamaCapability {
    Completion,  // "completion" - text completion
    Tools,       // "tools" - tool calling support  
    Insert,      // "insert" - text insertion
    Vision,      // "vision" - multimodal/image support
    Embedding,   // "embedding" - embedding generation
    Thinking,    // "thinking" - reasoning models
}

#[derive(Deserialize, Debug)]
pub struct OllamaModelInfo {
    pub modelfile: String,
    pub parameters: String,
    pub template: String,
    pub details: OllamaModelDetails,
    pub model_info: serde_json::Value,
    pub capabilities: Vec<String>, // ✨ This is the key field!
    pub modified_at: String,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModelDetails {
    pub parent_model: String,
    pub format: String,
    pub family: String,
    pub families: Vec<String>,
    pub parameter_size: String,
    pub quantization_level: String,
}
```

#### **Implementation: Capability-Based Tool Selection**

```rust
impl OllamaClient {
    /// Get model capabilities from Ollama
    pub async fn get_model_capabilities(&self, model: &str) -> Result<Vec<String>, OllamaError> {
        let url = format!("{}/api/show", self.base_url);
        let request = serde_json::json!({
            "name": model
        });
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
            
        let model_info: OllamaModelInfo = response.json().await?;
        Ok(model_info.capabilities)
    }
    
    /// Check if model supports specific capability
    pub async fn supports_capability(&self, model: &str, capability: &str) -> Result<bool, OllamaError> {
        let capabilities = self.get_model_capabilities(model).await?;
        Ok(capabilities.contains(&capability.to_string()))
    }
}

impl ApiClient {
    /// Get tools based on model capabilities
    async fn get_ollama_tools(&self, model: &str) -> Result<Vec<OllamaTool>, ApiClientError> {
        let mut tools = Vec::new();
        
        // Always include core functional tools
        tools.push(create_fs_read_tool());
        tools.push(create_execute_bash_tool());
        tools.push(create_fs_write_tool());
        tools.push(create_use_aws_tool());
        
        // Check if model supports tools at all
        if let Some(ollama_client) = &self.ollama_client {
            if !ollama_client.supports_capability(model, "tools").await? {
                // Model doesn't support tools - return empty list or basic tools only
                return Ok(vec![]);
            }
            
            // Add thinking tool only for reasoning-capable models
            if ollama_client.supports_capability(model, "thinking").await? {
                tools.push(create_thinking_tool());
            }
        }
        
        Ok(tools)
    }
}
```

#### **Benefits of This Approach**

✅ **Authoritative**: Uses Ollama's own capability detection  
✅ **Accurate**: No guessing based on model names  
✅ **Future-proof**: Works with new models automatically  
✅ **Granular**: Can detect both "tools" and "thinking" separately  
✅ **Reliable**: Official API, not heuristics  

#### **Example Usage**

```bash
# Your GPT OSS 120b would return:
curl http://localhost:11434/api/show -d '{"name": "gpt-oss"}'
# Response includes:
# "capabilities": ["completion", "tools", "thinking"]

# A basic model might return:
# "capabilities": ["completion"]

# A vision model might return:  
# "capabilities": ["completion", "vision"]
```

#### **Implementation Strategy**

1. **Query capabilities** when model is first used
2. **Cache results** to avoid repeated API calls
3. **Include appropriate tools** based on capabilities
4. **Graceful fallback** if capability detection fails

```rust
async fn should_include_thinking_tool(&self, model: &str) -> Result<bool, ApiClientError> {
    // 1. Check if thinking is globally enabled
    if !crate::cli::chat::tools::thinking::Thinking::is_enabled(os) {
        return Ok(false);
    }
    
    // 2. Check model capabilities via Ollama API
    if let Some(ollama_client) = &self.ollama_client {
        match ollama_client.supports_capability(model, "thinking").await {
            Ok(supports_thinking) => return Ok(supports_thinking),
            Err(e) => {
                warn!("Failed to check thinking capability: {}", e);
                // Fall back to pattern matching
            }
        }
    }
    
    // 3. Fallback: pattern-based detection
    Ok(self.is_likely_reasoning_model(model))
}
```

This approach gives us the **best of both worlds**: authoritative capability detection from Ollama with fallback to pattern matching for reliability.

#### **Implementation Impact**
```rust
// Updated tool list for Ollama
pub const OLLAMA_NATIVE_TOOLS: [&str; 4] = [
    "fs_read",
    "fs_write", 
    "execute_bash", // or "execute_cmd" on Windows
    "use_aws",
    // Excluded: "thinking", "knowledge", "gh_issue"
];
```

This approach ensures that Ollama integration focuses on universally useful tools while avoiding model-specific reasoning capabilities that most Ollama models don't handle well.

This comprehensive plan provides a clear roadmap for implementing full tool call support with Ollama while maintaining compatibility with the existing Amazon Q CLI tool infrastructure.

## 🧪 **Testing Strategy**

### **Unit Tests**
```rust
#[tokio::test]
async fn test_tool_call_detection() {
    let ollama_response = OllamaChatResponse {
        model: "llama3.1".to_string(),
        message: OllamaMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![OllamaToolCall {
                id: "call_123".to_string(),
                tool_type: "function".to_string(),
                function: OllamaFunctionCall {
                    name: "fs_read".to_string(),
                    arguments: r#"{"operations":[{"mode":"Line","path":"README.md"}]}"#.to_string(),
                },
            }]),
            images: None,
            tool_call_id: None,
        },
        done: false,
        // ... other fields
    };
    
    // Test that this converts to ChatResponseStream::ToolUseEvent
}

#[test]
fn test_tool_definition_generation() {
    let tools = get_ollama_tools().unwrap();
    assert!(tools.iter().any(|t| t.function.name == "fs_read"));
    assert!(tools.iter().any(|t| t.function.name == "execute_bash"));
}
```

### **Integration Tests**
```bash
# Test tool usage with Ollama
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Then ask: "Read the README.md file"
# Should see tool call execution and file content
```

## 🚧 **Challenges and Considerations**

### **1. Tool Schema Mapping**
- AWS tool schemas may differ from Ollama expectations
- Need to map parameter types correctly
- Handle optional vs required parameters

### **2. Tool Execution Integration**
- Reuse existing tool execution infrastructure
- Handle tool errors gracefully
- Maintain security boundaries

### **3. Conversation Flow**
- Tool calls interrupt normal message flow
- Need to handle multi-turn tool conversations
- Preserve conversation history with tool results

### **4. Model Compatibility**
- Not all Ollama models support tool calls
- Need graceful degradation for unsupported models
- Clear error messages when tools aren't available

## 🎯 **Success Criteria**

After Task 6 implementation:
- [ ] `Q_CLI_MODEL_PROVIDER=ollama q chat` supports tool calls
- [ ] Built-in tools (`fs_read`, `execute_bash`, etc.) work with Ollama
- [ ] Tool call detection works in streaming responses
- [ ] Tool execution results are properly integrated
- [ ] Error handling works for unsupported models/tools
- [ ] All existing functionality remains unchanged
- [ ] Unit tests pass for tool call mapping
- [ ] Integration tests demonstrate working tools

## 🔄 **Dependencies and Prerequisites**

### **Model Requirements**
- Ollama model that supports tool calls (e.g., `llama3.1`, `mistral`)
- Model must be pulled and available locally

### **Code Dependencies**
- Existing tool execution infrastructure
- JSON schema generation for tool parameters
- Tool call ID generation and tracking

## 📋 **Implementation Phases**

### **Phase 1: Basic Tool Types**
1. Add tool call types to Ollama message structures
2. Update streaming receiver to detect tool calls
3. Add basic tool definitions

### **Phase 2: Tool Integration**
1. Add tools to Ollama requests
2. Implement tool call detection and conversion
3. Integrate with existing tool execution

### **Phase 3: Conversation Flow**
1. Handle tool results in conversation history
2. Support multi-turn tool conversations
3. Error handling and graceful degradation

### **Phase 4: Testing and Polish**
1. Comprehensive unit tests
2. Integration tests with real Ollama models
3. Documentation and examples

This research provides a clear roadmap for implementing tool call support that integrates seamlessly with the existing Amazon Q CLI tool infrastructure while working with Ollama's tool call format.
