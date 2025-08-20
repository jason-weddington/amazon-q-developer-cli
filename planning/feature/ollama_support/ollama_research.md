# Ollama Integration Research

## Overview
Research findings from analyzing the [ollama-python](https://github.com/ollama/ollama-python) repository to understand how to integrate Ollama support into Amazon Q CLI.

## Key Findings

### Default Configuration
- **Default Host**: `http://127.0.0.1:11434` (localhost on port 11434)
- **Environment Variable**: `OLLAMA_HOST` can override the default host
- **Protocol**: HTTP/HTTPS with JSON API
- **No Authentication**: Local Ollama server doesn't require API keys or authentication

### API Endpoints
Based on the Python client, Ollama exposes these key endpoints:
- `POST /api/chat` - Chat completion (our primary need)
- `POST /api/generate` - Text generation (alternative approach)
- `GET /api/tags` - List available models
- `POST /api/show` - Get model information
- `POST /api/pull` - Download models
- `POST /api/embed` - Generate embeddings

### Chat API Structure

#### Request Format (`POST /api/chat`)
```json
{
  "model": "llama2",
  "messages": [
    {
      "role": "user",
      "content": "Hello, how are you?"
    }
  ],
  "stream": true,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "function_name",
        "description": "Function description",
        "parameters": {
          "type": "object",
          "properties": {
            "param1": {"type": "string", "description": "Parameter description"}
          },
          "required": ["param1"]
        }
      }
    }
  ],
  "options": {
    "temperature": 0.7,
    "top_p": 0.9
  },
  "keep_alive": "5m"
}
```

#### Response Format (Streaming)
Each line is a JSON object:
```json
{
  "model": "llama2",
  "created_at": "2023-12-12T14:13:43.416799Z",
  "message": {
    "role": "assistant",
    "content": "Hello! I'm doing well, thank you for asking."
  },
  "done": false
}
```

Final message:
```json
{
  "model": "llama2",
  "created_at": "2023-12-12T14:13:43.416799Z",
  "message": {
    "role": "assistant",
    "content": ""
  },
  "done": true,
  "total_duration": 5589157167,
  "load_duration": 3013701500,
  "prompt_eval_count": 26,
  "prompt_eval_duration": 383809000,
  "eval_count": 298,
  "eval_duration": 2117345000
}
```

#### Non-Streaming Response
```json
{
  "model": "llama2",
  "created_at": "2023-12-12T14:13:43.416799Z",
  "message": {
    "role": "assistant",
    "content": "Hello! I'm doing well, thank you for asking. How can I help you today?"
  },
  "done": true,
  "total_duration": 5589157167,
  "load_duration": 3013701500,
  "prompt_eval_count": 26,
  "prompt_eval_duration": 383809000,
  "eval_count": 298,
  "eval_duration": 2117345000
}
```

### Tool Support
Ollama supports function calling similar to OpenAI:

#### Tool Call Request
```json
{
  "model": "llama3.1",
  "messages": [{"role": "user", "content": "What is 3 + 5?"}],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "add_numbers",
        "description": "Add two numbers together",
        "parameters": {
          "type": "object",
          "properties": {
            "a": {"type": "integer", "description": "First number"},
            "b": {"type": "integer", "description": "Second number"}
          },
          "required": ["a", "b"]
        }
      }
    }
  ]
}
```

#### Tool Call Response
```json
{
  "model": "llama3.1",
  "message": {
    "role": "assistant",
    "content": "",
    "tool_calls": [
      {
        "function": {
          "name": "add_numbers",
          "arguments": {"a": 3, "b": 5}
        }
      }
    ]
  },
  "done": true
}
```

#### Tool Result Follow-up
```json
{
  "model": "llama3.1",
  "messages": [
    {"role": "user", "content": "What is 3 + 5?"},
    {
      "role": "assistant",
      "tool_calls": [
        {
          "function": {
            "name": "add_numbers",
            "arguments": {"a": 3, "b": 5}
          }
        }
      ]
    },
    {
      "role": "tool",
      "content": "8",
      "tool_name": "add_numbers"
    }
  ]
}
```

### Error Handling
- **HTTP Status Codes**: Standard HTTP status codes (404 for model not found, 500 for server errors)
- **Error Response Format**:
  ```json
  {
    "error": "model 'nonexistent' not found, try pulling it first"
  }
  ```
- **Connection Errors**: Handle cases where Ollama server is not running

### Model Management
- **List Models**: `GET /api/tags` returns available models
- **Model Format**: Models are referenced by name (e.g., `llama2`, `codellama`, `gemma3`)
- **Model Information**: `POST /api/show` with `{"name": "model_name"}` returns model details

### Streaming Implementation
- **Content-Type**: `application/x-ndjson` (newline-delimited JSON)
- **Streaming Pattern**: Each line is a complete JSON object
- **End Detection**: `"done": true` indicates stream completion
- **Partial Content**: `message.content` contains incremental text chunks

## Implementation Requirements for Rust

### HTTP Client
- Use existing `reqwest` crate (already in dependencies)
- Support for streaming responses with `reqwest::Response::bytes_stream()`
- JSON serialization/deserialization with `serde_json`

### Request/Response Types
Need to create Rust structs for:

```rust
#[derive(Serialize, Deserialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
    #[serde(default = "default_stream")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OllamaChatResponse {
    model: String,
    created_at: String,
    message: OllamaMessage,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_duration: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
}
```

### Streaming Implementation
```rust
async fn stream_chat_response(
    client: &reqwest::Client,
    request: OllamaChatRequest,
) -> Result<impl Stream<Item = Result<OllamaChatResponse, Error>>, Error> {
    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&request)
        .send()
        .await?;
    
    let stream = response
        .bytes_stream()
        .map(|chunk| {
            let chunk = chunk?;
            let line = String::from_utf8(chunk.to_vec())?;
            let response: OllamaChatResponse = serde_json::from_str(&line)?;
            Ok(response)
        });
    
    Ok(stream)
}
```

### Integration Points

#### 1. Provider Selection
Add to `ApiClient::new()`:
```rust
let provider = env.get("Q_CLI_MODEL_PROVIDER").unwrap_or("aws".to_string());
match provider.as_str() {
    "aws" => {
        // Existing AWS client logic
    },
    "ollama" => {
        // Create Ollama client
        let ollama_client = OllamaClient::new(
            env.get("Q_CLI_OLLAMA_BASE_URL")
                .unwrap_or("http://localhost:11434".to_string())
        )?;
    },
    _ => return Err(ApiClientError::InvalidProvider(provider)),
}
```

#### 2. SendMessageOutput Extension
```rust
pub enum SendMessageOutput {
    Codewhisperer(/* existing */),
    QDeveloper(/* existing */),
    Ollama(OllamaStreamResponse),
    Mock(/* existing */),
}
```

#### 3. Message Format Conversion
Need to convert between Amazon Q message format and Ollama format:
- Map `UserInputMessage` → `OllamaMessage`
- Map tool calls between formats
- Handle conversation history

#### 4. Tool Integration
Ollama's tool format is similar to OpenAI's, need to:
- Convert Amazon Q tool definitions to Ollama format
- Map tool call responses back to Amazon Q format
- Handle tool execution results

### Configuration
Environment variables to support:
- `Q_CLI_MODEL_PROVIDER=ollama`
- `Q_CLI_OLLAMA_BASE_URL=http://localhost:11434` (optional, defaults to localhost)
- `Q_CLI_OLLAMA_MODEL=llama2` (optional, can be set per conversation)

### Error Scenarios to Handle
1. **Ollama server not running**: Connection refused errors
2. **Model not found**: 404 responses with helpful error messages
3. **Invalid model format**: Malformed responses
4. **Network timeouts**: Long-running model inference
5. **Tool call failures**: Invalid tool definitions or execution errors

### Testing Strategy
1. **Unit Tests**: Mock Ollama HTTP responses
2. **Integration Tests**: Require running Ollama server with test models
3. **Error Handling Tests**: Simulate various failure scenarios
4. **Tool Integration Tests**: Verify tool calls work end-to-end

## Compatibility Considerations

### Message Format Differences
- **Amazon Q**: Uses specific message structures with metadata
- **Ollama**: Uses OpenAI-compatible message format
- **Solution**: Create conversion layer between formats

### Tool Call Differences
- **Amazon Q**: Has specific tool call format and execution model
- **Ollama**: Uses OpenAI-style function calling
- **Solution**: Map between tool formats, maintain execution compatibility

### Streaming Differences
- **Amazon Q**: Uses AWS-specific streaming format
- **Ollama**: Uses newline-delimited JSON streaming
- **Solution**: Adapt streaming parser to handle both formats

### Model Selection
- **Amazon Q**: Uses AWS model identifiers
- **Ollama**: Uses local model names (llama2, codellama, etc.)
- **Solution**: Allow model specification via environment variable or agent config

## Authentication Bypass Research

### Current Authentication Flow
Based on codebase analysis, here's how authentication currently works:

#### 1. Command-Level Auth Check
In `crates/chat-cli/src/cli/mod.rs`:
```rust
pub fn requires_auth(&self) -> bool {
    matches!(self, Self::Chat(_) | Self::Profile)
}

pub async fn execute(self, os: &mut Os) -> Result<ExitCode> {
    // Check for auth on subcommands that require it.
    if self.requires_auth() && !crate::auth::is_logged_in(&mut os.database).await {
        bail!(
            "You are not logged in, please log in with {}",
            format!("{CLI_BINARY_NAME} login").bold()
        );
    }
    // ... rest of execution
}
```

#### 2. Auth Status Check
In `crates/chat-cli/src/auth/builder_id.rs`:
```rust
pub async fn is_logged_in(database: &mut Database) -> bool {
    // Check for BuilderId if not using Sigv4
    if std::env::var("AMAZON_Q_SIGV4").is_ok_and(|v| !v.is_empty()) {
        debug!("logged in using sigv4 credentials");
        return true;  // SIGV4 bypasses token check
    }

    match BuilderIdToken::load(database).await {
        Ok(Some(_)) => true,
        Ok(None) => {
            info!("not logged in - no valid token found");
            false
        },
        Err(err) => {
            warn!(?err, "failed to try to load a builder id token");
            false
        },
    }
}
```

#### 3. Client Creation
In `crates/chat-cli/src/api_client/mod.rs`, the client is created based on environment variables:
```rust
// If SIGV4_AUTH_ENABLED is true, use Q developer client
match env.get("AMAZON_Q_SIGV4").is_ok() {
    true => {
        // Create SIGV4 client with AWS credentials
        let credentials_chain = CredentialsChain::new().await;
        // ... SIGV4 client setup
    },
    false => {
        // Create Bearer token client (Builder ID)
        // ... Bearer token client setup
    },
}
```

### Authentication Bypass Strategy

#### Option 1: Extend `is_logged_in()` Function
Modify the `is_logged_in()` function to check for `Q_CLI_MODEL_PROVIDER`:

```rust
pub async fn is_logged_in(database: &mut Database) -> bool {
    // Check if using non-AWS provider (bypass auth)
    if let Ok(provider) = std::env::var("Q_CLI_MODEL_PROVIDER") {
        if provider != "aws" {
            debug!("bypassing auth for non-AWS provider: {}", provider);
            return true;
        }
    }

    // Existing AWS auth logic...
    if std::env::var("AMAZON_Q_SIGV4").is_ok_and(|v| !v.is_empty()) {
        debug!("logged in using sigv4 credentials");
        return true;
    }

    match BuilderIdToken::load(database).await {
        // ... existing logic
    }
}
```

#### Option 2: Modify `requires_auth()` Function
Alternative approach - modify the command-level auth requirement:

```rust
impl RootSubcommand {
    pub fn requires_auth(&self) -> bool {
        // Check if using non-AWS provider
        if let Ok(provider) = std::env::var("Q_CLI_MODEL_PROVIDER") {
            if provider != "aws" {
                return false; // Non-AWS providers don't require auth
            }
        }
        
        matches!(self, Self::Chat(_) | Self::Profile)
    }
}
```

#### Option 3: Provider-Aware Client Creation
Extend the client creation logic to handle multiple providers:

```rust
pub async fn new(
    env: &Env,
    fs: &Fs,
    database: &mut Database,
    endpoint: Option<Endpoint>,
) -> Result<Self, ApiClientError> {
    let provider = env.get("Q_CLI_MODEL_PROVIDER").unwrap_or("aws".to_string());
    
    match provider.as_str() {
        "aws" => {
            // Existing AWS client creation logic
            // Requires authentication
        },
        "ollama" => {
            // Create Ollama client
            // No authentication required
            let base_url = env.get("Q_CLI_OLLAMA_BASE_URL")
                .unwrap_or("http://localhost:11434".to_string());
            
            Ok(Self {
                client: CodewhispererClient::from_conf(/* minimal config */),
                streaming_client: None,
                sigv4_streaming_client: None,
                ollama_client: Some(OllamaClient::new(base_url)?),
                mock_client: None,
                profile: None, // No AWS profile needed
                model_cache: Arc::new(RwLock::new(None)),
            })
        },
        "openai" | "anthropic" => {
            // Future: API key-based authentication
            let api_key = env.get("Q_CLI_MODEL_PROVIDER_API_KEY")
                .ok_or(ApiClientError::MissingApiKey(provider.clone()))?;
            // ... create respective clients
        },
        _ => Err(ApiClientError::InvalidProvider(provider)),
    }
}
```

### Recommended Approach

**Option 1 (Extend `is_logged_in()`)** is the cleanest approach because:

1. **Minimal Changes**: Only requires modifying one function
2. **Consistent Flow**: Maintains existing command execution flow
3. **Clear Logic**: Auth bypass is explicit and well-documented
4. **Future-Proof**: Easy to extend for other providers

### Environment Variable Validation

Add validation at application startup in `main.rs`:

```rust
fn validate_environment() -> Result<()> {
    if let Ok(provider) = std::env::var("Q_CLI_MODEL_PROVIDER") {
        match provider.as_str() {
            "aws" | "ollama" | "openai" | "anthropic" => {
                // Valid provider
                if provider == "openai" || provider == "anthropic" {
                    // Check for required API key
                    if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                        bail!("Q_CLI_MODEL_PROVIDER_API_KEY is required when using provider: {}", provider);
                    }
                }
            },
            _ => bail!("Invalid Q_CLI_MODEL_PROVIDER: {}. Valid values: aws, ollama, openai, anthropic", provider),
        }
    }
    Ok(())
}

fn main() -> Result<ExitCode> {
    color_eyre::install()?;
    
    // Validate environment variables early
    validate_environment()?;
    
    // ... rest of main function
}
```

### Implementation Points

1. **Auth Bypass**: Modify `is_logged_in()` to return `true` for non-AWS providers
2. **Client Creation**: Extend `ApiClient::new()` to handle provider selection
3. **Validation**: Add environment variable validation at startup
4. **Error Handling**: Provide clear error messages for configuration issues
5. **Documentation**: Update help text and error messages to mention provider options

This approach ensures that:
- ✅ Ollama works without `q login`
- ✅ AWS functionality remains unchanged
- ✅ Future providers can be added easily
- ✅ Clear error messages guide users
- ✅ Validation happens early to catch configuration issues

## Next Steps
1. Implement environment variable validation at startup
2. Modify `is_logged_in()` function for auth bypass
3. Extend `ApiClient::new()` for provider selection
4. Implement basic Ollama HTTP client
5. Create message format conversion layer
6. Integrate with existing `SendMessageOutput` enum
7. Add streaming response handling
8. Implement tool call mapping
9. Add comprehensive error handling
10. Create integration tests
