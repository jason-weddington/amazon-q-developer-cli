# Ollama API Research for Task 2

## Ollama REST API Overview

Ollama provides a REST API that we can integrate with using Rust's HTTP client libraries. Based on the Ollama documentation and common usage patterns:

### Key API Endpoints

1. **Chat Completion** (Primary endpoint we need)
   ```
   POST /api/chat
   ```

2. **Generate** (Alternative endpoint)
   ```
   POST /api/generate
   ```

3. **List Models**
   ```
   GET /api/tags
   ```

4. **Model Info**
   ```
   POST /api/show
   ```

### Chat API Request Format
```json
{
  "model": "llama3.2",
  "messages": [
    {
      "role": "user", 
      "content": "Hello, how are you?"
    }
  ],
  "stream": true
}
```

### Chat API Response Format (Streaming)
```json
{
  "model": "llama3.2",
  "created_at": "2023-12-12T14:13:43.416799Z",
  "message": {
    "role": "assistant",
    "content": "Hello! I'm doing well, thank you for asking."
  },
  "done": true
}
```

## Rust HTTP Client Options

### Option 1: reqwest (Recommended)
- Already used in the codebase (seen in Cargo.toml dependencies)
- Excellent async support
- Built-in JSON serialization/deserialization
- Streaming support
- Well-maintained and popular

### Option 2: hyper
- Lower-level, more control
- Already used indirectly in the codebase
- More complex to use directly

### Option 3: ureq
- Synchronous, simpler
- Smaller dependency footprint
- Less suitable for streaming

## Implementation Strategy for Task 2

### 1. Create Ollama Client Structure
```rust
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}
```

### 2. Define Request/Response Types
```rust
#[derive(Serialize, Deserialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    pub stream: bool,
}

#[derive(Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
pub struct OllamaChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaMessage,
    pub done: bool,
}
```

### 3. Implement Chat Method
```rust
impl OllamaClient {
    pub async fn chat(&self, request: OllamaChatRequest) -> Result<OllamaChatResponse> {
        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
            
        let chat_response: OllamaChatResponse = response.json().await?;
        Ok(chat_response)
    }
}
```

### 4. Integration Points with Existing Code

#### Add to ApiClient
```rust
pub struct ApiClient {
    // ... existing fields
    ollama_client: Option<OllamaClient>,
}
```

#### Modify Constructor
```rust
match provider {
    ModelProvider::Ollama => {
        let base_url = env.get("Q_CLI_OLLAMA_BASE_URL")
            .unwrap_or("http://localhost:11434".to_string());
        
        Ok(Self {
            client: /* minimal AWS client for compatibility */,
            streaming_client: None,
            sigv4_streaming_client: None,
            ollama_client: Some(OllamaClient::new(base_url)),
            // ... other fields
        })
    }
}
```

## Environment Variables

### Required
- `Q_CLI_MODEL_PROVIDER=ollama` (already implemented in Task 1)

### Optional
- `Q_CLI_MODEL_PROVIDER_BASE_URL` (default: "http://localhost:11434" for Ollama)
- `Q_CLI_MODEL_PROVIDER_API_KEY` (for OpenAI/Anthropic, not needed for Ollama)

### Future Provider Examples
```bash
# Ollama (local)
Q_CLI_MODEL_PROVIDER=ollama
Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:11434  # optional, this is default

# OpenAI (future)
Q_CLI_MODEL_PROVIDER=openai
Q_CLI_MODEL_PROVIDER_BASE_URL=https://api.openai.com/v1  # optional, would be default
Q_CLI_MODEL_PROVIDER_API_KEY=sk-...

# Anthropic (future)  
Q_CLI_MODEL_PROVIDER=anthropic
Q_CLI_MODEL_PROVIDER_BASE_URL=https://api.anthropic.com  # optional, would be default
Q_CLI_MODEL_PROVIDER_API_KEY=sk-ant-...

# Custom OpenAI-compatible endpoint
Q_CLI_MODEL_PROVIDER=openai
Q_CLI_MODEL_PROVIDER_BASE_URL=https://my-custom-llm-api.com/v1
Q_CLI_MODEL_PROVIDER_API_KEY=custom-key
```

## Error Handling

### New Error Types Needed
```rust
#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("Ollama server error: {status} - {message}")]
    ServerError { status: u16, message: String },
    
    #[error("Model not found: {model}")]
    ModelNotFound { model: String },
    
    #[error("Connection failed: {url}")]
    ConnectionFailed { url: String },
}
```

## Testing Strategy

### Unit Tests
- Mock HTTP responses using `mockito` or similar
- Test request/response serialization
- Test error handling scenarios

### Integration Tests
- Require running Ollama server locally
- Test actual API communication
- Test streaming responses

### Manual Testing
```bash
# Start Ollama server
ollama serve

# Pull a model
ollama pull llama3.2

# Test our implementation
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
```

## Dependencies to Add

Add to `Cargo.toml`:
```toml
[dependencies]
# reqwest is likely already present, but ensure it has json feature
reqwest = { version = "0.11", features = ["json", "stream"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio-stream = "0.1" # for streaming support
```

## Advantages of This Approach

1. **No External SDK Dependency**: We control the implementation
2. **Lightweight**: Only the HTTP calls we need
3. **Flexible**: Easy to extend and modify
4. **Consistent**: Uses same patterns as existing AWS clients
5. **Testable**: Easy to mock and test
6. **Future-Proof**: Can easily add new Ollama features

## Next Steps for Task 2

1. Create basic `OllamaClient` struct and HTTP client
2. Implement simple chat method (non-streaming first)
3. Add to `ApiClient` enum/structure
4. Create basic error types
5. Add environment variable support for base URL
6. Write unit tests
7. Test with local Ollama server

This approach gives us a solid foundation that we can build on for streaming (Task 5) and other advanced features.
