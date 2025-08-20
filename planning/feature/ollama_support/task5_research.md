# Task 5 Research: Add Streaming Response Handling

## 🔍 **Current Architecture Analysis**

### **Existing Streaming Pipeline**
Based on codebase analysis, here's how AWS streaming currently works:

#### **1. SendMessageOutput Variants**
```rust
pub enum SendMessageOutput {
    Codewhisperer(GenerateAssistantResponseOutput),  // AWS streaming
    QDeveloper(SendMessageOutput),                   // AWS streaming  
    Mock(Vec<ChatResponseStream>),                   // Test mock
    Ollama(OllamaChatResponse),                      // Our current non-streaming Ollama
}
```

#### **2. Streaming Interface**
All streaming variants implement:
```rust
impl SendMessageOutput {
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        match self {
            Self::Codewhisperer(output) => Ok(output.generate_assistant_response_response.recv().await?.map(|s| s.into())),
            Self::QDeveloper(output) => Ok(output.send_message_response.recv().await?.map(|s| s.into())),
            Self::Mock(vec) => Ok(vec.pop()),
            Self::Ollama(_) => Ok(None), // Currently returns None (non-streaming)
        }
    }
}
```

#### **3. Response Processing Pipeline**
Located in `crates/chat-cli/src/cli/chat/parser.rs`:

```rust
struct ResponseParser {
    response: SendMessageOutput,
    // ... other fields
}

impl ResponseParser {
    async fn next(&mut self) -> Result<Option<ChatResponseStream>, RecvError> {
        self.response.recv().await // Calls SendMessageOutput::recv()
    }
}
```

#### **4. ChatResponseStream Events**
The parser expects these event types:
```rust
pub enum ChatResponseStream {
    AssistantResponseEvent { content: String },      // Text chunks
    CodeEvent { content: String },                   // Code chunks
    ToolUseEvent { tool_use_id: String, name: String, input: Option<String>, stop: Option<bool> },
    MessageMetadataEvent { conversation_id: Option<String>, utterance_id: Option<String> },
    InvalidStateEvent { reason: String, message: String },
    // ... other variants
}
```

#### **5. Response Event Flow**
The parser converts `ChatResponseStream` to `ResponseEvent`:
```rust
pub enum ResponseEvent {
    AssistantResponseChunk { content: String },      // Text to display
    ToolUseStart { name: String },                   // Tool execution start
    ToolUse(AssistantToolUse),                       // Tool execution
    EndStream {                                      // Stream completion
        message: AssistantMessage,                   // Complete message for history
        request_metadata: RequestMetadata,           // Performance metrics
    },
}
```

### **Current Problem**
Our `SendMessageOutput::Ollama(OllamaChatResponse)` variant returns `Ok(None)` from `recv()`, which immediately signals stream completion to the parser, but without any content events.

## 🌊 **Ollama Streaming API Research**

### **Ollama Streaming Request**
```json
{
  "model": "gpt-oss:120b",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": true
}
```

### **Ollama Streaming Response Format**
Each line is a separate JSON object (newline-delimited JSON):

```json
{"model":"gpt-oss:120b","created_at":"2023-12-12T14:13:43.416799Z","message":{"role":"assistant","content":"Hello"},"done":false}
{"model":"gpt-oss:120b","created_at":"2023-12-12T14:13:43.516799Z","message":{"role":"assistant","content":" there"},"done":false}
{"model":"gpt-oss:120b","created_at":"2023-12-12T14:13:43.616799Z","message":{"role":"assistant","content":"!"},"done":false}
{"model":"gpt-oss:120b","created_at":"2023-12-12T14:13:43.716799Z","message":{"role":"assistant","content":""},"done":true,"total_duration":5589157167,"load_duration":3013701500,"prompt_eval_count":26,"prompt_eval_duration":383809000,"eval_count":298,"eval_duration":2117345000}
```

### **Key Characteristics**
- **Content-Type**: `application/x-ndjson` (newline-delimited JSON)
- **Incremental content**: Each chunk contains partial content
- **Final message**: `done: true` with performance metrics
- **Empty content**: Final message has empty content string

## 🎯 **Task 5 Implementation Strategy**

### **Option 1: Streaming SendMessageOutput Variant (Recommended)**
Create a new streaming variant that matches AWS streaming behavior:

```rust
pub enum SendMessageOutput {
    Codewhisperer(GenerateAssistantResponseOutput), // AWS streaming
    QDeveloper(SendMessageOutput),                  // AWS streaming  
    Mock(Vec<ChatResponseStream>),                  // Test mock
    Ollama(OllamaChatResponse),                     // Keep for non-streaming Ollama
    OllamaStreaming(OllamaStreamReceiver),          // New streaming Ollama
}
```

### **Option 2: Modify Existing Ollama Variant (Simpler)**
Make the existing `Ollama` variant support streaming by changing its internal structure.

**Recommendation**: Go with Option 1 for cleaner separation and easier testing.

## 🔧 **Detailed Implementation Plan**

### **Step 1: Create Ollama Stream Types**
```rust
// In crates/chat-cli/src/api_client/ollama.rs

use futures::Stream;
use std::pin::Pin;
use tokio_stream::StreamExt;

pub struct OllamaStreamReceiver {
    stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send>>,
    message_id: String,
    ended: bool,
}

impl OllamaStreamReceiver {
    pub fn new(stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send>>, message_id: String) -> Self {
        Self {
            stream,
            message_id,
            ended: false,
        }
    }
    
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, crate::api_client::ApiClientError> {
        if self.ended {
            return Ok(None);
        }
        
        match self.stream.next().await {
            Some(Ok(ollama_response)) => {
                if ollama_response.done {
                    self.ended = true;
                    // Return None to signal end of stream (parser handles EndStream event)
                    Ok(None)
                } else if !ollama_response.message.content.is_empty() {
                    // Convert to ChatResponseStream::AssistantResponseEvent
                    Ok(Some(ChatResponseStream::AssistantResponseEvent {
                        content: ollama_response.message.content,
                    }))
                } else {
                    // Skip empty content chunks, get next
                    self.recv().await
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

### **Step 2: Update OllamaClient for Streaming**
```rust
impl OllamaClient {
    pub async fn chat_stream(&self, request: OllamaChatRequest) 
        -> Result<OllamaStreamReceiver, OllamaError> {
        
        let mut request = request;
        request.stream = Some(true);
        
        let response = self.client
            .post(&format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let stream = response
            .bytes_stream()
            .map(|chunk_result| {
                let chunk = chunk_result.map_err(OllamaError::HttpError)?;
                let line = String::from_utf8(chunk.to_vec())
                    .map_err(|e| OllamaError::InvalidResponse(format!("Invalid UTF-8: {}", e)))?;
                
                // Skip empty lines
                if line.trim().is_empty() {
                    return Ok(None);
                }
                
                let ollama_response: OllamaChatResponse = serde_json::from_str(&line)
                    .map_err(|e| OllamaError::InvalidResponse(format!("Invalid JSON: {}", e)))?;
                Ok(Some(ollama_response))
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(response)) => Some(Ok(response)),
                    Ok(None) => None, // Skip empty lines
                    Err(e) => Some(Err(e)),
                }
            });
            
        let message_id = uuid::Uuid::new_v4().to_string();
        Ok(OllamaStreamReceiver::new(Box::pin(stream), message_id))
    }
}
```

### **Step 3: Update SendMessageOutput Implementation**
```rust
impl SendMessageOutput {
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        match self {
            Self::Codewhisperer(output) => Ok(output.generate_assistant_response_response.recv().await?.map(|s| s.into())),
            Self::QDeveloper(output) => Ok(output.send_message_response.recv().await?.map(|s| s.into())),
            Self::Mock(vec) => Ok(vec.pop()),
            Self::Ollama(_) => Ok(None), // Keep existing non-streaming behavior
            Self::OllamaStreaming(receiver) => receiver.recv().await, // New streaming behavior
        }
    }
}
```

### **Step 4: Update ApiClient Integration**
```rust
impl ApiClient {
    async fn send_message_ollama_internal(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
        let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
        
        match &self.ollama_client {
            Some(client) => {
                let request = OllamaChatRequest {
                    model,
                    messages: ollama_messages,
                    stream: Some(true), // Enable streaming
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
}
```

### **Step 5: Add Required Dependencies**
Add to `crates/chat-cli/Cargo.toml`:
```toml
[dependencies]
tokio-stream = "0.1"
futures = "0.3"
uuid = { version = "1.0", features = ["v4"] }
```

## 🧪 **Testing Strategy**

### **Unit Tests**
```rust
#[tokio::test]
async fn test_ollama_streaming_response() {
    let mock_responses = vec![
        r#"{"model":"test","message":{"role":"assistant","content":"Hello"},"done":false}"#,
        r#"{"model":"test","message":{"role":"assistant","content":" world"},"done":false}"#,
        r#"{"model":"test","message":{"role":"assistant","content":"!"},"done":false}"#,
        r#"{"model":"test","message":{"role":"assistant","content":""},"done":true}"#,
    ];
    
    // Create mock stream and test OllamaStreamReceiver
    let mut receiver = create_mock_stream_receiver(mock_responses);
    
    // Should get content chunks
    let event1 = receiver.recv().await.unwrap().unwrap();
    assert!(matches!(event1, ChatResponseStream::AssistantResponseEvent { content } if content == "Hello"));
    
    let event2 = receiver.recv().await.unwrap().unwrap();
    assert!(matches!(event2, ChatResponseStream::AssistantResponseEvent { content } if content == " world"));
    
    let event3 = receiver.recv().await.unwrap().unwrap();
    assert!(matches!(event3, ChatResponseStream::AssistantResponseEvent { content } if content == "!"));
    
    // Final message should return None (end of stream)
    let event4 = receiver.recv().await.unwrap();
    assert!(event4.is_none());
}

#[tokio::test]
async fn test_ollama_stream_error_handling() {
    // Test network errors, malformed JSON, etc.
}
```

### **Integration Tests**
```bash
# Test with real Ollama server
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Should see streaming responses character by character
```

## 🚧 **Challenges and Considerations**

### **1. Error Handling**
- Network interruptions during streaming
- Malformed JSON in stream (skip bad lines, continue stream)
- Ollama server errors mid-stream
- Graceful degradation when streaming fails

### **2. Performance**
- Buffering strategy for network chunks
- Memory usage for long responses
- Backpressure handling if UI can't keep up

### **3. Compatibility**
- Maintain existing non-streaming Ollama support for testing
- Ensure AWS streaming behavior unchanged
- Mock testing infrastructure needs updating

### **4. Stream Parsing**
- Handle newline-delimited JSON correctly
- Skip empty lines gracefully
- Parse partial JSON chunks (buffer incomplete lines)

## 🎯 **Success Criteria**

After Task 5 implementation:
- [ ] `Q_CLI_MODEL_PROVIDER=ollama q chat` shows streaming responses
- [ ] Characters appear progressively as Ollama generates them
- [ ] Stream ends gracefully when `done: true` received
- [ ] Error handling works for stream interruptions
- [ ] All existing AWS streaming functionality unchanged
- [ ] Unit tests pass for streaming conversion
- [ ] Integration tests demonstrate real-time streaming
- [ ] Performance is acceptable (no significant lag)

## 🔄 **Dependencies and Prerequisites**

### **Required Rust Crates**
- `tokio-stream = "0.1"` - for async stream handling
- `futures = "0.3"` - for stream utilities  
- `uuid = { version = "1.0", features = ["v4"] }` - for message ID generation

### **Ollama Server Requirements**
- Ollama server running with streaming support
- Model pulled and available
- Network connectivity stable

## 📋 **Implementation Phases**

### **Phase 1: Basic Streaming Infrastructure**
1. Create `OllamaStreamReceiver` struct
2. Implement basic newline-delimited JSON parsing
3. Add `OllamaStreaming` variant to `SendMessageOutput`

### **Phase 2: HTTP Streaming Client**
1. Update `OllamaClient::chat_stream()` method
2. Handle HTTP streaming response parsing
3. Implement proper error handling for stream failures

### **Phase 3: Integration and Testing**
1. Update `ApiClient` to use streaming by default
2. Add comprehensive unit tests
3. Test with real Ollama server

### **Phase 4: Polish and Error Handling**
1. Robust error handling for stream failures
2. Performance optimization and buffering
3. Documentation and examples

## 🔍 **Key Architectural Insights**

1. **No Parser Changes Needed**: The existing `ResponseParser` will work unchanged - it just calls `response.recv().await` in a loop until it gets `None`.

2. **Stream Completion**: When Ollama sends `done: true`, we return `None` from `recv()`, which signals the parser to emit `ResponseEvent::EndStream`.

3. **Error Propagation**: Stream errors are converted to `ApiClientError` and propagate up through the parser to the UI.

4. **Message ID Generation**: Since Ollama doesn't provide message IDs, we generate UUIDs for consistency with AWS behavior.

5. **Backward Compatibility**: Keep the existing `Ollama` variant for non-streaming use cases and testing.

This research provides a clear roadmap for implementing proper Ollama streaming support that integrates seamlessly with the existing AWS streaming infrastructure.
