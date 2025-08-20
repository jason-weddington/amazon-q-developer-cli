# Task 4 Research: Message Format Conversion

## 🔍 **Current Architecture Analysis**

### **Message Flow (AWS)**
1. **User Input** → `UserInputMessage` struct
2. **Conversation State** → `ConversationState` struct  
3. **API Client** → `client.send_message(conversation_state)` 
4. **Provider Routing** → AWS streaming client
5. **Response** → `SendMessageOutput::Codewhisperer` or `SendMessageOutput::QDeveloper`

### **Key Data Structures**

#### **ConversationState**
```rust
pub struct ConversationState {
    pub conversation_id: Option<String>,
    pub user_input_message: UserInputMessage,
    pub history: Option<Vec<ChatMessage>>,
}
```

#### **UserInputMessage**
```rust
pub struct UserInputMessage {
    pub content: String,
    pub user_input_message_context: Option<UserInputMessageContext>,
    pub user_intent: Option<UserIntent>,
    pub images: Option<Vec<ImageBlock>>,
    pub model_id: Option<String>,
}
```

#### **ChatMessage (History)**
```rust
pub enum ChatMessage {
    AssistantResponseMessage(AssistantResponseMessage),
    UserInputMessage(UserInputMessage),
}
```

### **Current send_message Method**
Located in `crates/chat-cli/src/api_client/mod.rs:478`

```rust
pub async fn send_message(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
    // Routes to streaming_client (AWS) or sigv4_streaming_client (AWS)
    // NO OLLAMA ROUTING YET
}
```

## 🎯 **Task 4 Implementation Plan**

### **Goal**
Enable the CLI to convert between AWS message formats and Ollama message formats, allowing seamless chat functionality with Ollama providers.

### **Key Components to Implement**

#### **1. Add Ollama Routing to send_message**
Modify `ApiClient::send_message` to detect Ollama provider and route accordingly:

```rust
pub async fn send_message(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
    // NEW: Check provider and route to Ollama
    match self.provider {
        ModelProvider::Ollama => {
            return self.send_message_ollama_internal(conversation).await;
        },
        ModelProvider::Aws => {
            // Existing AWS logic
        },
        // ... other providers
    }
}
```

#### **2. Message Format Conversion Functions**

**AWS → Ollama Conversion:**
```rust
fn convert_conversation_to_ollama(conversation: ConversationState) -> (Vec<OllamaMessage>, String) {
    let mut ollama_messages = Vec::new();
    
    // Convert history
    if let Some(history) = conversation.history {
        for chat_message in history {
            match chat_message {
                ChatMessage::UserInputMessage(user_msg) => {
                    ollama_messages.push(OllamaMessage {
                        role: "user".to_string(),
                        content: user_msg.content,
                        images: convert_images(user_msg.images),
                    });
                },
                ChatMessage::AssistantResponseMessage(assistant_msg) => {
                    ollama_messages.push(OllamaMessage {
                        role: "assistant".to_string(),
                        content: assistant_msg.content,
                        images: None,
                    });
                },
            }
        }
    }
    
    // Add current user message
    ollama_messages.push(OllamaMessage {
        role: "user".to_string(),
        content: conversation.user_input_message.content,
        images: convert_images(conversation.user_input_message.images),
    });
    
    let model = conversation.user_input_message.model_id
        .unwrap_or_else(|| "llama3.2".to_string());
    
    (ollama_messages, model)
}
```

**Image Conversion:**
```rust
fn convert_images(aws_images: Option<Vec<ImageBlock>>) -> Option<Vec<String>> {
    aws_images.map(|images| {
        images.into_iter()
            .filter_map(|img| {
                // Convert AWS ImageBlock to Ollama base64 format
                match img {
                    ImageBlock { data, format } => {
                        Some(format!("data:{};base64,{}", format, data))
                    }
                }
            })
            .collect()
    })
}
```

#### **3. Internal Ollama Send Method**
```rust
async fn send_message_ollama_internal(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
    let (ollama_messages, model) = convert_conversation_to_ollama(conversation);
    
    match &self.ollama_client {
        Some(client) => {
            let request = OllamaChatRequest {
                model,
                messages: ollama_messages,
                stream: Some(false), // Non-streaming for Task 4
                format: None,
                options: None,
                keep_alive: None,
            };
            
            let response = client.chat(request).await?;
            Ok(SendMessageOutput::from_ollama(response))
        },
        None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
    }
}
```

#### **4. Model Selection Integration**
Ensure model selection works with Ollama:

```rust
// In model selection logic
match provider {
    ModelProvider::Ollama => {
        // Get available Ollama models
        let models = api_client.list_ollama_models().await?;
        // Present to user for selection
    },
    ModelProvider::Aws => {
        // Existing AWS model logic
    }
}
```

### **Files to Modify**

1. **`crates/chat-cli/src/api_client/mod.rs`**
   - Add Ollama routing to `send_message` method
   - Add `send_message_ollama_internal` method
   - Add message conversion functions

2. **`crates/chat-cli/src/api_client/model.rs`** (if needed)
   - Add any Ollama-specific message types

3. **`crates/chat-cli/src/cli/chat/cli/model.rs`** (if exists)
   - Add Ollama model selection support

### **Testing Strategy**

#### **Unit Tests**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_convert_conversation_to_ollama() {
        let conversation = ConversationState {
            conversation_id: Some("test".to_string()),
            user_input_message: UserInputMessage {
                content: "Hello".to_string(),
                model_id: Some("llama3.2".to_string()),
                ..Default::default()
            },
            history: Some(vec![
                ChatMessage::UserInputMessage(UserInputMessage {
                    content: "Previous question".to_string(),
                    ..Default::default()
                }),
                ChatMessage::AssistantResponseMessage(AssistantResponseMessage {
                    content: "Previous answer".to_string(),
                    ..Default::default()
                }),
            ]),
        };
        
        let (messages, model) = convert_conversation_to_ollama(conversation);
        
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Previous question");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Previous answer");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, "Hello");
        assert_eq!(model, "llama3.2");
    }
    
    #[tokio::test]
    async fn test_send_message_ollama_integration() {
        // Test with mock Ollama server
        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("POST", "/api/chat")
            .with_status(200)
            .with_body(r#"{"model":"llama3.2","message":{"role":"assistant","content":"Hello!"},"done":true}"#)
            .create_async()
            .await;
            
        let api_client = create_test_api_client_with_ollama(server.url()).await;
        let conversation = create_test_conversation();
        
        let result = api_client.send_message(conversation).await;
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(matches!(output, SendMessageOutput::Ollama(_)));
        
        mock.assert_async().await;
    }
}
```

#### **Integration Tests**
Update test scripts to validate Task 4:

```bash
# In validate_task4.sh
echo "Testing message conversion..."
if Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat <<< "Hello" 2>&1 | grep -q "Hello"; then
    echo "✅ Basic chat working"
else
    echo "❌ Basic chat failing"
fi
```

### **Expected Behavior After Task 4**

```bash
# This should work!
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
> Hello, how are you?
# Should get response from Ollama model

# With specific model
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --model llama3.2 chat
> What's the weather like?
# Should use llama3.2 model specifically
```

### **Edge Cases to Handle**

1. **Empty History**: Handle conversations with no previous messages
2. **Image Messages**: Convert AWS image format to Ollama base64 format
3. **Model Selection**: Default to available model if none specified
4. **Error Handling**: Proper error conversion from Ollama to AWS error types
5. **Context Length**: Handle Ollama context limits differently from AWS

### **Success Criteria**

- [ ] `Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat` works
- [ ] Conversation history is preserved across messages
- [ ] Model selection works with Ollama models
- [ ] Error handling provides clear feedback
- [ ] All existing AWS functionality remains unchanged
- [ ] Unit tests pass for message conversion
- [ ] Integration tests demonstrate end-to-end functionality

## 🚀 **Next Steps**

1. Implement message conversion functions
2. Add Ollama routing to `send_message`
3. Add comprehensive unit tests
4. Update integration test scripts
5. Test with real Ollama server
6. Validate conversation flow works end-to-end

This research provides a clear roadmap for implementing Task 4 with confidence that we understand the existing architecture and can integrate Ollama seamlessly.
