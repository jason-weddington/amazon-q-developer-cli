# Development Tasks

## Task 1: Environment Variable Validation and Provider Selection

### Description
Implement environment variable validation for model provider selection and add the foundation for multi-provider support. This task establishes the basic infrastructure needed for Ollama integration without breaking existing AWS functionality.

### Acceptance Criteria
- [ ] `Q_CLI_MODEL_PROVIDER` environment variable is validated at application startup
- [ ] Valid values are: `aws`, `ollama`, `openai`, `anthropic` (case-insensitive)
- [ ] Invalid values show clear error message with valid options
- [ ] When provider is `openai` or `anthropic`, `Q_CLI_MODEL_PROVIDER_API_KEY` is required
- [ ] When provider is `ollama`, no API key validation is performed
- [ ] Default behavior (no env var set) remains unchanged (uses AWS)
- [ ] All existing functionality continues to work normally
- [ ] Clear error messages guide users on proper configuration

### Implementation Details

#### 1. Add Environment Variable Validation Function
Create in `crates/chat-cli/src/util/mod.rs`:

```rust
use eyre::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProvider {
    Aws,
    Ollama,
    OpenAi,
    Anthropic,
}

impl ModelProvider {
    pub fn from_env() -> Result<Self> {
        let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
            .unwrap_or_else(|_| "aws".to_string())
            .to_lowercase();
        
        match provider.as_str() {
            "aws" => Ok(Self::Aws),
            "ollama" => Ok(Self::Ollama),
            "openai" => {
                // Check for required API key
                if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                    bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using OpenAI provider");
                }
                Ok(Self::OpenAi)
            },
            "anthropic" => {
                // Check for required API key
                if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                    bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using Anthropic provider");
                }
                Ok(Self::Anthropic)
            },
            _ => bail!(
                "Invalid Q_CLI_MODEL_PROVIDER: '{}'. Valid values are: aws, ollama, openai, anthropic", 
                provider
            ),
        }
    }
    
    pub fn requires_auth(&self) -> bool {
        matches!(self, Self::Aws)
    }
    
    pub fn requires_api_key(&self) -> bool {
        matches!(self, Self::OpenAi | Self::Anthropic)
    }
}
```

#### 2. Add Validation to Main Function
Modify `crates/chat-cli/src/main.rs`:

```rust
use crate::util::ModelProvider;

fn main() -> Result<ExitCode> {
    color_eyre::install()?;
    
    // Validate environment variables early
    ModelProvider::from_env()?;

    let parsed = match cli::Cli::try_parse() {
        // ... existing code
    };
    
    // ... rest of main function
}
```

#### 3. Update Authentication Check
Modify `crates/chat-cli/src/auth/builder_id.rs`:

```rust
use crate::util::ModelProvider;

pub async fn is_logged_in(database: &mut Database) -> bool {
    // Check if using non-AWS provider (bypass auth)
    if let Ok(provider) = ModelProvider::from_env() {
        if !provider.requires_auth() {
            debug!("bypassing auth for provider: {:?}", provider);
            return true;
        }
    }

    // Existing AWS auth logic...
    if std::env::var("AMAZON_Q_SIGV4").is_ok_and(|v| !v.is_empty()) {
        debug!("logged in using sigv4 credentials");
        return true;
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

#### 4. Add Provider Context to ApiClient
Modify `crates/chat-cli/src/api_client/mod.rs`:

```rust
use crate::util::ModelProvider;

#[derive(Clone, Debug)]
pub struct ApiClient {
    client: CodewhispererClient,
    streaming_client: Option<CodewhispererStreamingClient>,
    sigv4_streaming_client: Option<QDeveloperStreamingClient>,
    mock_client: Option<Arc<Mutex<std::vec::IntoIter<Vec<ChatResponseStream>>>>>,
    profile: Option<AuthProfile>,
    model_cache: ModelCache,
    provider: ModelProvider, // Add this field
}

impl ApiClient {
    pub async fn new(
        env: &Env,
        fs: &Fs,
        database: &mut Database,
        endpoint: Option<Endpoint>,
    ) -> Result<Self, ApiClientError> {
        let provider = ModelProvider::from_env()
            .map_err(|e| ApiClientError::InvalidConfiguration(e.to_string()))?;
        
        // For now, only AWS is implemented
        match provider {
            ModelProvider::Aws => {
                // Existing AWS client creation logic...
            },
            ModelProvider::Ollama | ModelProvider::OpenAi | ModelProvider::Anthropic => {
                // TODO: Implement in future tasks
                return Err(ApiClientError::UnsupportedProvider(format!("{:?}", provider)));
            },
        }
        
        // ... existing client creation code, but add provider to struct
        Ok(Self {
            client,
            streaming_client,
            sigv4_streaming_client,
            mock_client: None,
            profile,
            model_cache: Arc::new(RwLock::new(None)),
            provider, // Add this
        })
    }
    
    pub fn provider(&self) -> &ModelProvider {
        &self.provider
    }
}
```

#### 5. Add New Error Types
Add to `crates/chat-cli/src/api_client/error.rs`:

```rust
#[derive(Debug, Error)]
pub enum ApiClientError {
    // ... existing variants
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),
}
```

### Testing Strategy

#### Unit Tests
Create `crates/chat-cli/src/util/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_provider_from_env_default() {
        env::remove_var("Q_CLI_MODEL_PROVIDER");
        let provider = ModelProvider::from_env().unwrap();
        assert_eq!(provider, ModelProvider::Aws);
    }

    #[test]
    fn test_provider_from_env_valid_values() {
        for (env_val, expected) in [
            ("aws", ModelProvider::Aws),
            ("AWS", ModelProvider::Aws),
            ("ollama", ModelProvider::Ollama),
            ("OLLAMA", ModelProvider::Ollama),
        ] {
            env::set_var("Q_CLI_MODEL_PROVIDER", env_val);
            let provider = ModelProvider::from_env().unwrap();
            assert_eq!(provider, expected);
        }
    }

    #[test]
    fn test_provider_from_env_invalid() {
        env::set_var("Q_CLI_MODEL_PROVIDER", "invalid");
        let result = ModelProvider::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid Q_CLI_MODEL_PROVIDER"));
    }

    #[test]
    fn test_openai_requires_api_key() {
        env::remove_var("Q_CLI_MODEL_PROVIDER_API_KEY");
        env::set_var("Q_CLI_MODEL_PROVIDER", "openai");
        let result = ModelProvider::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Q_CLI_MODEL_PROVIDER_API_KEY"));
    }

    #[test]
    fn test_ollama_no_api_key_required() {
        env::remove_var("Q_CLI_MODEL_PROVIDER_API_KEY");
        env::set_var("Q_CLI_MODEL_PROVIDER", "ollama");
        let provider = ModelProvider::from_env().unwrap();
        assert_eq!(provider, ModelProvider::Ollama);
    }
}
```

#### Integration Tests
Test that the CLI properly validates environment variables:

```bash
# Test 1: Invalid provider shows helpful error
Q_CLI_MODEL_PROVIDER=invalid cargo run --bin chat_cli -- chat
# Expected output: "Invalid Q_CLI_MODEL_PROVIDER: 'invalid'. Valid values are: aws, ollama, openai, anthropic"

# Test 2: OpenAI without API key shows clear error  
Q_CLI_MODEL_PROVIDER=openai cargo run --bin chat_cli -- chat
# Expected output: "Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using OpenAI provider"

# Test 3: Anthropic without API key shows clear error
Q_CLI_MODEL_PROVIDER=anthropic cargo run --bin chat_cli -- chat  
# Expected output: "Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using Anthropic provider"

# Test 4: Ollama bypasses auth but shows unsupported error (expected for now)
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Expected: Should NOT show "You are not logged in" error
# Expected: Should show "Unsupported provider: Ollama" error instead

# Test 5: Valid OpenAI with API key bypasses auth  
Q_CLI_MODEL_PROVIDER=openai Q_CLI_MODEL_PROVIDER_API_KEY=test cargo run --bin chat_cli -- chat
# Expected: Should NOT show "You are not logged in" error
# Expected: Should show "Unsupported provider: OpenAi" error instead

# Test 6: Default behavior unchanged (no env vars)
cargo run --bin chat_cli -- chat
# Expected: "You are not logged in, please log in with q login" (existing behavior)

# Test 7: Case insensitive provider names
Q_CLI_MODEL_PROVIDER=OLLAMA cargo run --bin chat_cli -- chat
Q_CLI_MODEL_PROVIDER=Aws cargo run --bin chat_cli -- chat
# Expected: Should work the same as lowercase versions

# Test 8: Help and other commands work normally
cargo run --bin chat_cli -- --help
cargo run --bin chat_cli -- version
# Expected: Normal output, no validation errors
```

#### Manual Testing Checklist
- [ ] Invalid provider shows error before any other processing
- [ ] Missing API key for OpenAI/Anthropic shows clear error message  
- [ ] Ollama and valid API key providers bypass authentication
- [ ] Default behavior (no env vars) works exactly as before
- [ ] Case insensitive provider names work
- [ ] Non-chat commands (help, version) are unaffected
- [ ] Error messages are user-friendly and actionable

### Files to Modify
- `crates/chat-cli/src/main.rs` - Add validation call
- `crates/chat-cli/src/util/mod.rs` - Add ModelProvider enum and validation
- `crates/chat-cli/src/auth/builder_id.rs` - Update is_logged_in function
- `crates/chat-cli/src/api_client/mod.rs` - Add provider field and basic handling
- `crates/chat-cli/src/api_client/error.rs` - Add new error types

### Definition of Done
- [ ] All unit tests pass
- [ ] Integration tests demonstrate proper validation
- [ ] Invalid environment variables show helpful error messages
- [ ] Existing AWS functionality works unchanged
- [ ] Code is properly documented with rustdoc comments
- [ ] No clippy warnings introduced
- [ ] Changes are covered by tests

---

## Task 2: Implement Basic Ollama HTTP Client

### Description
Create a basic HTTP client for communicating with Ollama's REST API. This task implements the core HTTP communication layer without streaming support, providing the foundation for Ollama integration.

### Acceptance Criteria
- [ ] `OllamaClient` struct with HTTP client functionality
- [ ] Basic chat method that sends requests to Ollama `/api/chat` endpoint
- [ ] Request/response types matching Ollama API specification
- [ ] Environment variable support for `Q_CLI_MODEL_PROVIDER_BASE_URL` (default: "http://localhost:11434" for Ollama)
- [ ] Integration with existing `ApiClient` structure
- [ ] Proper error handling for HTTP and Ollama-specific errors
- [ ] Unit tests with mocked HTTP responses
- [ ] Non-streaming responses only (streaming in Task 5)
- [ ] Manual testing with local Ollama server works

### Implementation Details

#### 1. Create Ollama Client Module
Create `crates/chat-cli/src/api_client/ollama.rs`:

```rust
use reqwest;
use serde::{Deserialize, Serialize};
use eyre::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("Ollama server error: {status} - {message}")]
    ServerError { status: u16, message: String },
    
    #[error("Model not found: {model}")]
    ModelNotFound { model: String },
    
    #[error("Connection failed to {url}")]
    ConnectionFailed { url: String },
    
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

// Ollama API types based on official documentation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,    // "system", "user", "assistant", "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>, // base64 encoded images for multimodal
}

#[derive(Serialize, Debug)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>, // Default true, we'll set to false for Task 2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>, // "json" for JSON mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>, // Model parameters like temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>, // e.g. "5m"
}

#[derive(Deserialize, Debug)]
pub struct OllamaChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaMessage,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_reason: Option<String>, // "stop", "length", etc.
    
    // Performance metrics (only in final response when done=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModel {
    pub name: String,
    pub model: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModel>,
}

#[derive(Clone, Debug)]
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
    
    /// Send a chat request to Ollama (non-streaming)
    pub async fn chat(&self, mut request: OllamaChatRequest) -> Result<OllamaChatResponse, OllamaError> {
        // Ensure non-streaming for Task 2
        request.stream = Some(false);
        
        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(OllamaError::HttpError)?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let chat_response: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| OllamaError::InvalidResponse(e.to_string()))?;
            
        Ok(chat_response)
    }
    
    /// List available models
    pub async fn list_models(&self) -> Result<OllamaModelsResponse, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(OllamaError::HttpError)?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let models_response: OllamaModelsResponse = response
            .json()
            .await
            .map_err(|e| OllamaError::InvalidResponse(e.to_string()))?;
            
        Ok(models_response)
    }
    
    /// Health check - verify Ollama server is accessible
    pub async fn health_check(&self) -> Result<bool, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Get the base URL for this client
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}
```

#### 2. Add Ollama Module to api_client
Modify `crates/chat-cli/src/api_client/mod.rs`:

```rust
mod ollama;
pub use ollama::{
    OllamaClient, 
    OllamaError, 
    OllamaMessage, 
    OllamaChatRequest, 
    OllamaChatResponse,
    OllamaModel,
    OllamaModelsResponse,
};
```

#### 3. Update ApiClient Constructor
Modify the `ApiClient::new` method to handle Ollama provider:

```rust
match provider {
    ModelProvider::Aws => {
        // Existing AWS client creation logic...
        Ok(Self {
            client,
            streaming_client,
            sigv4_streaming_client,
            ollama_client: None,
            mock_client: None,
            profile,
            model_cache: Arc::new(RwLock::new(None)),
            provider,
        })
    },
    ModelProvider::Ollama => {
        let base_url = env.get("Q_CLI_MODEL_PROVIDER_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        
        let ollama_client = OllamaClient::new(base_url);
        
        // Test connection during initialization
        if let Err(e) = ollama_client.health_check().await {
            warn!("Ollama health check failed: {}", e);
        }
        
        // Create minimal AWS client for compatibility
        let credentials = Credentials::new("dummy", "dummy", None, None, "dummy");
        let bearer_sdk_config = aws_config::defaults(behavior_version())
            .region(Region::new("us-east-1"))
            .credentials_provider(credentials)
            .load()
            .await;
        
        let client = CodewhispererClient::from_conf(
            amzn_codewhisperer_client::config::Builder::from(&bearer_sdk_config)
                .build(),
        );
        
        Ok(Self {
            client,
            streaming_client: None,
            sigv4_streaming_client: None,
            ollama_client: Some(ollama_client),
            mock_client: None,
            profile: None,
            model_cache: Arc::new(RwLock::new(None)),
            provider,
        })
    },
    ModelProvider::OpenAi | ModelProvider::Anthropic => {
        return Err(ApiClientError::UnsupportedProvider(format!("{}", provider)));
    },
}
```

#### 4. Add Helper Methods to ApiClient
Add methods to interact with Ollama:

```rust
impl ApiClient {
    /// Test Ollama connection
    pub async fn test_ollama_connection(&self) -> Result<bool, ApiClientError> {
        match &self.ollama_client {
            Some(client) => Ok(client.health_check().await?),
            None => Ok(false),
        }
    }
    
    /// Get Ollama client reference
    pub fn ollama_client(&self) -> Option<&OllamaClient> {
        self.ollama_client.as_ref()
    }
    
    /// List available Ollama models
    pub async fn list_ollama_models(&self) -> Result<Vec<String>, ApiClientError> {
        match &self.ollama_client {
            Some(client) => {
                let response = client.list_models().await?;
                Ok(response.models.into_iter().map(|m| m.name).collect())
            },
            None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
        }
    }
}
```

### Testing Strategy

#### Unit Tests
Create comprehensive tests in `crates/chat-cli/src/api_client/ollama.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, server_url};
    
    #[tokio::test]
    async fn test_ollama_chat_success() {
        let _m = mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "model": "llama3.2",
                "created_at": "2023-12-12T14:13:43.416799Z",
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "done": true,
                "total_duration": 5191566416,
                "load_duration": 2154458,
                "prompt_eval_count": 26,
                "prompt_eval_duration": 383809000,
                "eval_count": 298,
                "eval_duration": 4799921000
            }"#)
            .create();
            
        let client = OllamaClient::new(server_url());
        let request = OllamaChatRequest {
            model: "llama3.2".to_string(),
            messages: vec![OllamaMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            stream: Some(false),
            format: None,
            options: None,
            keep_alive: None,
        };
        
        let response = client.chat(request).await.unwrap();
        assert_eq!(response.message.content, "Hello! How can I help you today?");
        assert!(response.done);
        assert_eq!(response.model, "llama3.2");
    }
    
    #[tokio::test]
    async fn test_ollama_list_models() {
        let _m = mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "models": [
                    {
                        "name": "llama3.2:latest",
                        "model": "llama3.2:latest",
                        "modified_at": "2023-12-07T09:32:18.757212583-08:00",
                        "size": 2019393189,
                        "digest": "sha256:a80c4f17acd55265feec403c7aef86be0c25983ab279d83f3bcd3abbcb5b8b72"
                    }
                ]
            }"#)
            .create();
            
        let client = OllamaClient::new(server_url());
        let response = client.list_models().await.unwrap();
        assert_eq!(response.models.len(), 1);
        assert_eq!(response.models[0].name, "llama3.2:latest");
    }
    
    #[tokio::test]
    async fn test_ollama_server_error() {
        let _m = mock("POST", "/api/chat")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();
            
        let client = OllamaClient::new(server_url());
        let request = OllamaChatRequest {
            model: "nonexistent".to_string(),
            messages: vec![],
            stream: Some(false),
            format: None,
            options: None,
            keep_alive: None,
        };
        
        let result = client.chat(request).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            OllamaError::ServerError { status, .. } => assert_eq!(status, 500),
            _ => panic!("Expected ServerError"),
        }
    }
    
    #[tokio::test]
    async fn test_health_check_success() {
        let _m = mock("GET", "/api/tags")
            .with_status(200)
            .with_body(r#"{"models": []}"#)
            .create();
            
        let client = OllamaClient::new(server_url());
        let healthy = client.health_check().await.unwrap();
        assert!(healthy);
    }
    
    #[tokio::test]
    async fn test_health_check_failure() {
        let _m = mock("GET", "/api/tags")
            .with_status(500)
            .create();
            
        let client = OllamaClient::new(server_url());
        let healthy = client.health_check().await.unwrap();
        assert!(!healthy);
    }
}
```

#### Integration Tests
Manual test commands for real Ollama server:

```bash
# Prerequisites: Start Ollama and pull a model
ollama serve &
ollama pull llama3.2

# Test 1: Basic connection test
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --help
# Should not show "UnsupportedProvider" error

# Test 2: Custom Ollama URL
Q_CLI_MODEL_PROVIDER=ollama Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:11434 cargo run --bin chat_cli -- --help

# Test 3: Connection to non-existent server (should show connection error)
Q_CLI_MODEL_PROVIDER=ollama Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:99999 cargo run --bin chat_cli -- --help
```

### Dependencies
Ensure these are in `crates/chat-cli/Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[dev-dependencies]
mockito = "1.0"
tokio-test = "0.4"
```

### Files to Create/Modify
- **Create**: `crates/chat-cli/src/api_client/ollama.rs`
- **Modify**: `crates/chat-cli/src/api_client/mod.rs`
- **Modify**: `crates/chat-cli/src/api_client/error.rs`
- **Modify**: `crates/chat-cli/Cargo.toml` (if dependencies missing)

### Definition of Done
- [ ] All unit tests pass
- [ ] Integration tests work with local Ollama server
- [ ] `Q_CLI_MODEL_PROVIDER=ollama` no longer shows "UnsupportedProvider" error
- [ ] Basic HTTP communication with Ollama API works
- [ ] Error handling covers common failure scenarios (connection, server errors, invalid responses)
- [ ] Code follows existing patterns and style in the codebase
- [ ] No clippy warnings introduced
- [ ] Documentation comments added to public APIs
- [ ] Health check functionality works for connection validation

---

## Task 3: Add Ollama Variant to SendMessageOutput Enum

### Description
Add Ollama support to the `SendMessageOutput` enum to enable the CLI to handle Ollama chat responses. This task creates the bridge between our Ollama HTTP client (Task 2) and the existing message handling system, allowing Ollama responses to flow through the same pipeline as AWS responses.

### Acceptance Criteria
- [ ] Add `Ollama(OllamaChatResponse)` variant to `SendMessageOutput` enum
- [ ] Implement required traits for the new variant (`Debug`, `Clone`, etc.)
- [ ] Update pattern matching in message handling code to include Ollama case
- [ ] Add helper methods to extract common fields (content, role, etc.)
- [ ] Update error handling to support Ollama-specific errors
- [ ] Ensure backward compatibility with existing AWS functionality
- [ ] Unit tests for new enum variant and methods
- [ ] Integration test showing Ollama responses can be processed

### Implementation Details

#### 1. Update SendMessageOutput Enum
Modify `crates/chat-cli/src/api_client/send_message_output.rs`:

```rust
use crate::api_client::ollama::OllamaChatResponse;

#[derive(Debug, Clone)]
pub enum SendMessageOutput {
    // Existing AWS variants
    Streaming(ChatResponseStream),
    NonStreaming(SendMessageOutput),
    
    // New Ollama variant
    Ollama(OllamaChatResponse),
}
```

#### 2. Add Helper Methods
Add methods to extract common information regardless of provider:

```rust
impl SendMessageOutput {
    /// Get the message content from any provider response
    pub fn content(&self) -> Option<&str> {
        match self {
            Self::Streaming(stream) => {
                // Existing AWS streaming logic
                stream.message_content()
            },
            Self::NonStreaming(output) => {
                // Existing AWS non-streaming logic  
                output.message().and_then(|m| m.content())
            },
            Self::Ollama(response) => {
                Some(&response.message.content)
            },
        }
    }
    
    /// Get the message role from any provider response
    pub fn role(&self) -> Option<&str> {
        match self {
            Self::Streaming(stream) => {
                // Existing AWS logic
                stream.message_role()
            },
            Self::NonStreaming(output) => {
                // Existing AWS logic
                output.message().and_then(|m| m.role())
            },
            Self::Ollama(response) => {
                Some(&response.message.role)
            },
        }
    }
    
    /// Check if the response is complete
    pub fn is_done(&self) -> bool {
        match self {
            Self::Streaming(stream) => {
                // Existing AWS logic
                stream.is_complete()
            },
            Self::NonStreaming(_) => {
                true // Non-streaming is always complete
            },
            Self::Ollama(response) => {
                response.done
            },
        }
    }
    
    /// Get provider-specific metadata
    pub fn metadata(&self) -> ResponseMetadata {
        match self {
            Self::Streaming(stream) => {
                ResponseMetadata::Aws {
                    request_id: stream.request_id(),
                    // ... other AWS metadata
                }
            },
            Self::NonStreaming(output) => {
                ResponseMetadata::Aws {
                    // ... AWS metadata
                }
            },
            Self::Ollama(response) => {
                ResponseMetadata::Ollama {
                    model: response.model.clone(),
                    created_at: response.created_at.clone(),
                    total_duration: response.total_duration,
                    eval_count: response.eval_count,
                    eval_duration: response.eval_duration,
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ResponseMetadata {
    Aws {
        request_id: Option<String>,
        // ... other AWS fields
    },
    Ollama {
        model: String,
        created_at: String,
        total_duration: Option<u64>,
        eval_count: Option<u32>,
        eval_duration: Option<u64>,
    },
}
```

#### 3. Update Message Processing
Find and update code that processes `SendMessageOutput` to handle the Ollama case:

```rust
// In message processing functions
match output {
    SendMessageOutput::Streaming(stream) => {
        // Existing AWS streaming logic
    },
    SendMessageOutput::NonStreaming(response) => {
        // Existing AWS non-streaming logic
    },
    SendMessageOutput::Ollama(response) => {
        // New Ollama processing logic
        println!("{}", response.message.content);
    },
}
```

#### 4. Add Conversion Methods
Add methods to create `SendMessageOutput::Ollama` from our HTTP client:

```rust
impl From<OllamaChatResponse> for SendMessageOutput {
    fn from(response: OllamaChatResponse) -> Self {
        Self::Ollama(response)
    }
}

impl SendMessageOutput {
    /// Create from Ollama response
    pub fn from_ollama(response: OllamaChatResponse) -> Self {
        Self::Ollama(response)
    }
}
```

#### 5. Update ApiClient Integration
Modify `ApiClient` to return `SendMessageOutput::Ollama` for Ollama responses:

```rust
impl ApiClient {
    pub async fn send_message_ollama(&self, messages: Vec<OllamaMessage>, model: &str) -> Result<SendMessageOutput, ApiClientError> {
        match &self.ollama_client {
            Some(client) => {
                let request = OllamaChatRequest {
                    model: model.to_string(),
                    messages,
                    stream: Some(false), // Non-streaming for Task 3
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
}
```

### Testing Strategy

#### Unit Tests
Add tests to `crates/chat-cli/src/api_client/send_message_output.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_client::ollama::{OllamaChatResponse, OllamaMessage};
    
    #[test]
    fn test_ollama_variant_creation() {
        let ollama_response = OllamaChatResponse {
            model: "llama3.2".to_string(),
            created_at: "2023-12-12T14:13:43.416799Z".to_string(),
            message: OllamaMessage {
                role: "assistant".to_string(),
                content: "Hello! How can I help you?".to_string(),
                images: None,
            },
            done: true,
            done_reason: Some("stop".to_string()),
            total_duration: Some(5191566416),
            load_duration: Some(2154458),
            prompt_eval_count: Some(26),
            prompt_eval_duration: Some(383809000),
            eval_count: Some(298),
            eval_duration: Some(4799921000),
        };
        
        let output = SendMessageOutput::from_ollama(ollama_response);
        
        assert_eq!(output.content(), Some("Hello! How can I help you?"));
        assert_eq!(output.role(), Some("assistant"));
        assert!(output.is_done());
    }
    
    #[test]
    fn test_ollama_metadata() {
        let ollama_response = create_test_ollama_response();
        let output = SendMessageOutput::from_ollama(ollama_response);
        
        match output.metadata() {
            ResponseMetadata::Ollama { model, .. } => {
                assert_eq!(model, "llama3.2");
            },
            _ => panic!("Expected Ollama metadata"),
        }
    }
    
    #[test]
    fn test_from_trait() {
        let ollama_response = create_test_ollama_response();
        let output: SendMessageOutput = ollama_response.into();
        
        assert!(matches!(output, SendMessageOutput::Ollama(_)));
    }
}
```

#### Integration Tests
Update test scripts to verify Task 3 functionality:

```bash
# Test that Ollama responses can be created and processed
cargo test send_message_output::tests::test_ollama

# Test ApiClient Ollama integration
cargo test api_client::tests::test_send_message_ollama
```

### Files to Create/Modify
- **Modify**: `crates/chat-cli/src/api_client/send_message_output.rs`
- **Modify**: `crates/chat-cli/src/api_client/mod.rs` (add new methods)
- **Modify**: Any files that pattern match on `SendMessageOutput`
- **Update**: Test scripts to validate Task 3 functionality

### Definition of Done
- [ ] `SendMessageOutput::Ollama` variant exists and compiles
- [ ] Helper methods work for extracting content, role, completion status
- [ ] Metadata extraction works for Ollama responses
- [ ] Conversion methods (`From` trait, `from_ollama`) work correctly
- [ ] All existing AWS functionality remains unchanged
- [ ] Unit tests pass for new functionality
- [ ] Integration tests show Ollama responses can be processed
- [ ] Test scripts demonstrate Task 3 working
- [ ] No clippy warnings introduced
- [ ] Code follows existing patterns and style

### Expected Outcome
After Task 3, the CLI will be able to:
1. Create `SendMessageOutput::Ollama` from HTTP responses
2. Extract message content and metadata from Ollama responses
3. Process Ollama responses through the same pipeline as AWS responses
4. Maintain full backward compatibility with AWS functionality

This sets up the foundation for Task 4 (message conversion) where we'll connect the user input to Ollama requests and responses back to the display system.

---

## Task 4: Implement Message Format Conversion

### Description
Implement message format conversion between AWS and Ollama message formats, enabling the CLI to send user messages to Ollama and receive responses. This task connects the existing chat flow to our Ollama HTTP client (Task 2) and response processing (Task 3), making basic chat functionality work with Ollama providers.

### Acceptance Criteria
- [ ] Add Ollama provider routing to `ApiClient::send_message` method
- [ ] Implement AWS → Ollama message format conversion
- [ ] Handle conversation history conversion (user/assistant message pairs)
- [ ] Support image message conversion (AWS ImageBlock → Ollama base64)
- [ ] Add model selection support for Ollama models
- [ ] Implement proper error handling and conversion
- [ ] Maintain backward compatibility with AWS functionality
- [ ] Unit tests for message conversion functions
- [ ] Integration tests showing end-to-end chat functionality
- [ ] Basic chat command works: `Q_CLI_MODEL_PROVIDER=ollama q chat`

### Implementation Details

#### 1. Add Provider Routing to send_message
Modify `crates/chat-cli/src/api_client/mod.rs` `send_message` method:

```rust
pub async fn send_message(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
    debug!("Sending conversation: {:#?}", conversation);

    // NEW: Route based on provider
    match self.provider {
        ModelProvider::Ollama => {
            return self.send_message_ollama_internal(conversation).await;
        },
        ModelProvider::Aws => {
            // Existing AWS logic continues here...
        },
        ModelProvider::OpenAi | ModelProvider::Anthropic => {
            return Err(ApiClientError::UnsupportedProvider(format!("{}", self.provider)));
        },
    }

    // Existing AWS implementation continues...
    let ConversationState {
        conversation_id,
        user_input_message,
        history,
    } = conversation;
    // ... rest of existing AWS code
}
```

#### 2. Internal Ollama Send Method
Add new method to `ApiClient`:

```rust
async fn send_message_ollama_internal(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
    let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
    
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

#### 3. Message Conversion Functions
Add conversion methods to `ApiClient`:

```rust
fn convert_conversation_to_ollama(&self, conversation: ConversationState) -> Result<(Vec<OllamaMessage>, String), ApiClientError> {
    let mut ollama_messages = Vec::new();
    
    // Convert conversation history
    if let Some(history) = conversation.history {
        for chat_message in history {
            match chat_message {
                ChatMessage::UserInputMessage(user_msg) => {
                    ollama_messages.push(OllamaMessage {
                        role: "user".to_string(),
                        content: user_msg.content,
                        images: self.convert_images_to_ollama(user_msg.images)?,
                    });
                },
                ChatMessage::AssistantResponseMessage(assistant_msg) => {
                    ollama_messages.push(OllamaMessage {
                        role: "assistant".to_string(),
                        content: assistant_msg.content,
                        images: None, // Assistants don't send images in Ollama
                    });
                },
            }
        }
    }
    
    // Add current user message
    let current_message = OllamaMessage {
        role: "user".to_string(),
        content: conversation.user_input_message.content,
        images: self.convert_images_to_ollama(conversation.user_input_message.images)?,
    };
    ollama_messages.push(current_message);
    
    // Determine model to use
    let model = conversation.user_input_message.model_id
        .unwrap_or_else(|| "llama3.2".to_string()); // Default model
    
    Ok((ollama_messages, model))
}

fn convert_images_to_ollama(&self, aws_images: Option<Vec<ImageBlock>>) -> Result<Option<Vec<String>>, ApiClientError> {
    match aws_images {
        Some(images) => {
            let mut ollama_images = Vec::new();
            for image in images {
                // Convert AWS ImageBlock to Ollama base64 format
                let base64_image = match image.format.as_str() {
                    "png" | "jpeg" | "jpg" | "gif" | "webp" => {
                        format!("data:image/{};base64,{}", image.format, image.data)
                    },
                    _ => {
                        warn!("Unsupported image format: {}", image.format);
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
```

#### 4. Error Handling Enhancement
Add Ollama-specific error conversion:

```rust
impl From<OllamaError> for ApiClientError {
    fn from(err: OllamaError) -> Self {
        match err {
            OllamaError::HttpError(e) => ApiClientError::HttpError(e),
            OllamaError::SerializationError(e) => ApiClientError::SerializationError(e.to_string()),
            OllamaError::ServerError { status, message } => {
                if status == 429 {
                    ApiClientError::QuotaBreach { message, status_code: Some(status) }
                } else {
                    ApiClientError::ServerError { status, message }
                }
            },
            OllamaError::ModelNotFound { model } => {
                ApiClientError::ModelNotFound { model }
            },
            OllamaError::ConnectionFailed { url } => {
                ApiClientError::ConnectionFailed { url }
            },
            OllamaError::InvalidResponse(msg) => {
                ApiClientError::InvalidResponse(msg)
            },
        }
    }
}
```

#### 5. Model Selection Integration
Enhance model selection to work with Ollama (if model selection exists):

```rust
// In model selection logic (location TBD based on codebase exploration)
pub async fn get_available_models_for_provider(provider: &ModelProvider, client: &ApiClient) -> Result<Vec<ModelInfo>, ApiClientError> {
    match provider {
        ModelProvider::Ollama => {
            let model_names = client.list_ollama_models().await?;
            Ok(model_names.into_iter().map(|name| ModelInfo {
                model_id: name.clone(),
                model_name: Some(name),
                provider: ModelProvider::Ollama,
            }).collect())
        },
        ModelProvider::Aws => {
            // Existing AWS model logic
        },
        _ => Err(ApiClientError::UnsupportedProvider(format!("{}", provider))),
    }
}
```

### Testing Strategy

#### Unit Tests
Add to `crates/chat-cli/src/api_client/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_client::model::*;
    
    #[test]
    fn test_convert_simple_conversation_to_ollama() {
        let api_client = create_test_api_client();
        let conversation = ConversationState {
            conversation_id: Some("test-conv".to_string()),
            user_input_message: UserInputMessage {
                content: "Hello, world!".to_string(),
                model_id: Some("llama3.2".to_string()),
                user_input_message_context: None,
                user_intent: None,
                images: None,
            },
            history: None,
        };
        
        let (messages, model) = api_client.convert_conversation_to_ollama(conversation).unwrap();
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello, world!");
        assert_eq!(messages[0].images, None);
        assert_eq!(model, "llama3.2");
    }
    
    #[test]
    fn test_convert_conversation_with_history() {
        let api_client = create_test_api_client();
        let conversation = ConversationState {
            conversation_id: Some("test-conv".to_string()),
            user_input_message: UserInputMessage {
                content: "Follow up question".to_string(),
                model_id: Some("llama3.2".to_string()),
                user_input_message_context: None,
                user_intent: None,
                images: None,
            },
            history: Some(vec![
                ChatMessage::UserInputMessage(UserInputMessage {
                    content: "Initial question".to_string(),
                    model_id: None,
                    user_input_message_context: None,
                    user_intent: None,
                    images: None,
                }),
                ChatMessage::AssistantResponseMessage(AssistantResponseMessage {
                    content: "Initial response".to_string(),
                    // ... other fields
                }),
            ]),
        };
        
        let (messages, model) = api_client.convert_conversation_to_ollama(conversation).unwrap();
        
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Initial question");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Initial response");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, "Follow up question");
        assert_eq!(model, "llama3.2");
    }
    
    #[tokio::test]
    async fn test_send_message_ollama_integration() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "model": "llama3.2",
                "created_at": "2023-12-12T14:13:43.416799Z",
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?"
                },
                "done": true
            }"#)
            .create_async()
            .await;
            
        let api_client = create_test_api_client_with_ollama(server.url()).await;
        let conversation = ConversationState {
            conversation_id: Some("test".to_string()),
            user_input_message: UserInputMessage {
                content: "Hello".to_string(),
                model_id: Some("llama3.2".to_string()),
                user_input_message_context: None,
                user_intent: None,
                images: None,
            },
            history: None,
        };
        
        let result = api_client.send_message(conversation).await;
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(matches!(output, SendMessageOutput::Ollama(_)));
        if let SendMessageOutput::Ollama(response) = output {
            assert_eq!(response.message.content, "Hello! How can I help you?");
            assert_eq!(response.model, "llama3.2");
        }
        
        mock.assert_async().await;
    }
    
    fn create_test_api_client() -> ApiClient {
        // Create minimal ApiClient for testing
        ApiClient {
            // ... minimal required fields
            provider: ModelProvider::Ollama,
            ollama_client: None, // Will be set in integration tests
        }
    }
    
    async fn create_test_api_client_with_ollama(base_url: String) -> ApiClient {
        let ollama_client = OllamaClient::new(base_url);
        ApiClient {
            // ... other required fields
            provider: ModelProvider::Ollama,
            ollama_client: Some(ollama_client),
        }
    }
}
```

#### Integration Tests
Update test scripts to validate Task 4:

```bash
# Add to validate_task4.sh
echo "=== Task 4 Implementation Validation ==="

echo "1. Checking send_message Ollama routing..."
if grep -q "ModelProvider::Ollama" crates/chat-cli/src/api_client/mod.rs && grep -q "send_message_ollama_internal" crates/chat-cli/src/api_client/mod.rs; then
    echo "✅ Ollama routing found in send_message"
else
    echo "❌ Ollama routing not found"
fi

echo "2. Checking message conversion functions..."
if grep -q "convert_conversation_to_ollama" crates/chat-cli/src/api_client/mod.rs; then
    echo "✅ Message conversion functions found"
else
    echo "❌ Message conversion functions not found"
fi

echo "3. Testing basic chat functionality..."
if command -v timeout >/dev/null 2>&1; then
    if echo "Hello" | timeout 10s Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --no-interactive chat 2>&1 | grep -q "Hello\|assistant\|response"; then
        echo "✅ Basic chat functionality working"
    else
        echo "❌ Basic chat functionality not working"
    fi
else
    echo "⚠️  timeout command not available, skipping chat test"
fi
```

### Files to Create/Modify
- **Modify**: `crates/chat-cli/src/api_client/mod.rs` (main implementation)
- **Modify**: `crates/chat-cli/src/api_client/error.rs` (error conversion)
- **Create**: `validate_task4.sh` (validation script)
- **Update**: Test scripts to include Task 4 validation

### Definition of Done
- [ ] `Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat` works end-to-end
- [ ] Conversation history is preserved across multiple messages
- [ ] Model selection works with Ollama models (if model selection exists)
- [ ] Image messages are properly converted (if supported)
- [ ] Error handling provides clear, actionable feedback
- [ ] All existing AWS functionality remains unchanged
- [ ] Unit tests pass for all conversion functions
- [ ] Integration tests demonstrate working chat flow
- [ ] Test scripts show Task 4 as complete
- [ ] No clippy warnings introduced
- [ ] Code follows existing patterns and style

### Expected Outcome
After Task 4, users will be able to:
1. Start a chat session with Ollama: `Q_CLI_MODEL_PROVIDER=ollama q chat`
2. Have back-and-forth conversations with full history
3. Use different Ollama models via model selection
4. Get proper error messages when things go wrong
5. Experience the same chat UX as with AWS, but powered by Ollama

This completes the core chat functionality, setting up the foundation for Task 5 (streaming) and Task 6 (tool calls).

---

## Task 4.5: Integrate Ollama Model Listing with `/model` Command

### Description
Extend the existing `/model` command to work with Ollama provider, allowing users to list available local models and select them for chat sessions. This task bridges the gap between our message conversion (Task 4) and ensures users can easily discover and select Ollama models.

### Status: ✅ COMPLETE
- `/model` command works when `Q_CLI_MODEL_PROVIDER=ollama`
- Lists available Ollama models from local server
- Shows model names in user-friendly format
- Allows model selection that works with Ollama chat
- Model selection persists for the chat session

### Acceptance Criteria
- [ ] `/model` command works when `Q_CLI_MODEL_PROVIDER=ollama`
- [ ] Lists available Ollama models from local server
- [ ] Shows model names in user-friendly format
- [ ] Allows model selection that works with Ollama chat
- [ ] Provides clear error messages when Ollama server is unavailable
- [ ] Maintains backward compatibility with AWS model listing
- [ ] Default model selection works for new Ollama users
- [ ] Model selection persists for the chat session

### Implementation Details

#### 1. Extend Model Listing Logic
Find and modify the `/model` command handler to support Ollama:

```rust
// In the model listing logic (location TBD based on codebase exploration)
match api_client.provider() {
    ModelProvider::Ollama => {
        // Get available Ollama models
        match api_client.list_ollama_models().await {
            Ok(models) => {
                println!("Available Ollama models:");
                for (i, model) in models.iter().enumerate() {
                    println!("  {}. {}", i + 1, model);
                }
                // Handle user selection
            },
            Err(e) => {
                eprintln!("Failed to list Ollama models: {}", e);
                eprintln!("Make sure Ollama server is running: ollama serve");
            }
        }
    },
    ModelProvider::Aws => {
        // Existing AWS model listing logic
    },
    _ => {
        eprintln!("Model listing not supported for provider: {:?}", api_client.provider());
    }
}
```

#### 2. Enhance list_ollama_models Method
The method exists but may need enhancement:

```rust
impl ApiClient {
    pub async fn list_ollama_models(&self) -> Result<Vec<String>, ApiClientError> {
        match &self.ollama_client {
            Some(client) => {
                let response = client.list_models().await?;
                Ok(response.models.into_iter().map(|m| m.name).collect())
            },
            None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
        }
    }
}
```

#### 3. Model Selection Integration
Ensure selected models work with chat:

```rust
// In chat initialization or model selection
let selected_model = match provider {
    ModelProvider::Ollama => {
        // Use selected Ollama model or default
        user_selected_model.unwrap_or_else(|| "llama3.2".to_string())
    },
    ModelProvider::Aws => {
        // Existing AWS model logic
    }
};
```

#### 4. Default Model Handling
Provide sensible defaults for Ollama:

```rust
impl ModelProvider {
    pub fn default_model(&self) -> &str {
        match self {
            Self::Ollama => "llama3.2", // or detect first available
            Self::Aws => "claude-3-sonnet", // existing default
            Self::OpenAi => "gpt-4",
            Self::Anthropic => "claude-3-sonnet",
        }
    }
}
```

#### 5. Error Handling for Ollama Server
Provide helpful error messages:

```rust
match api_client.list_ollama_models().await {
    Ok(models) if models.is_empty() => {
        eprintln!("No Ollama models found. Pull a model first:");
        eprintln!("  ollama pull llama3.2");
        eprintln!("  ollama pull codellama");
    },
    Ok(models) => {
        // Show model list
    },
    Err(ApiClientError::OllamaError(OllamaError::ConnectionFailed { .. })) => {
        eprintln!("Cannot connect to Ollama server.");
        eprintln!("Start Ollama server: ollama serve");
    },
    Err(e) => {
        eprintln!("Failed to list models: {}", e);
    }
}
```

### Testing Strategy

#### Unit Tests
```rust
#[tokio::test]
async fn test_list_ollama_models_success() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/tags")
        .with_status(200)
        .with_body(r#"{
            "models": [
                {"name": "llama3.2:latest"},
                {"name": "codellama:latest"}
            ]
        }"#)
        .create_async()
        .await;
        
    let api_client = create_test_api_client_with_ollama(server.url()).await;
    let models = api_client.list_ollama_models().await.unwrap();
    
    assert_eq!(models.len(), 2);
    assert!(models.contains(&"llama3.2:latest".to_string()));
    assert!(models.contains(&"codellama:latest".to_string()));
    
    mock.assert_async().await;
}

#[tokio::test]
async fn test_list_ollama_models_connection_error() {
    let api_client = create_test_api_client_with_ollama("http://localhost:99999".to_string()).await;
    let result = api_client.list_ollama_models().await;
    
    assert!(result.is_err());
    // Should be a connection error
}
```

#### Integration Tests
```bash
# Test model listing with running Ollama
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Then type: /model
# Should show available Ollama models

# Test with no Ollama server
# Stop ollama server
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Then type: /model  
# Should show helpful error message
```

#### Manual Testing Checklist
- [ ] `/model` command shows Ollama models when provider is ollama
- [ ] Model selection works and persists for chat session
- [ ] Clear error when Ollama server is down
- [ ] Clear error when no models are available
- [ ] AWS model listing still works when provider is aws
- [ ] Default model selection works for new users

### Files to Investigate/Modify
Need to find where the `/model` command is implemented:
- Look for model listing/selection logic in CLI chat module
- Likely in `crates/chat-cli/src/cli/chat/` somewhere
- May need to modify model selection persistence logic

### Definition of Done
- [ ] `/model` command works with Ollama provider
- [ ] Lists available local Ollama models
- [ ] Model selection integrates with chat functionality  
- [ ] Helpful error messages for common issues
- [ ] Unit tests pass for model listing functionality
- [ ] Integration tests demonstrate working model selection
- [ ] AWS model listing functionality unchanged
- [ ] No clippy warnings introduced

### Expected Outcome
After Task 4.5, users will be able to:
1. Run `Q_CLI_MODEL_PROVIDER=ollama q chat`
2. Type `/model` to see available Ollama models
3. Select a model and have it work for the chat session
4. Get clear guidance when Ollama server is not running
5. Have a smooth model discovery and selection experience

This task bridges the gap between our technical implementation and user experience, making Ollama integration practical for daily use.

---

## Task 7: Fix Tool Call Execution with Ollama

### Description
Fix the critical bug where Ollama tool calls result in blank responses due to missing tool call detection in non-streaming responses. This is a fundamental functionality issue that prevents basic tool usage with Ollama models.

### Current Problem
- Ollama models generate tool calls correctly ✅ (confirmed by direct API testing)
- Tool calls are ignored in non-streaming response processing ❌
- Results in blank responses when tools should be executed ❌
- Tool execution pipeline is partially working but corrupted ❌
- User sees streaming text but tool calls fail silently ❌

### Research Findings
Based on comprehensive testing and code analysis (see `task7_research.md`):

1. **Root Cause**: Non-streaming response conversion only extracts `content` field, ignoring `tool_calls`
2. **Ollama Behavior**: Returns `content: ""` when tool calls are present, but includes tool calls in response
3. **Pipeline Issue**: Tool execution exists but command parsing is corrupted
4. **Streaming Paradox**: Code uses `stream: false` but user sees progressive text (needs investigation)

### Implementation Approach
**Option 2: Switch to Streaming (SELECTED)**

Based on analysis, switching to proper streaming will solve the root cause more elegantly:

```rust
// Enable streaming in Ollama provider
let request = OllamaChatRequest {
    model,
    messages: ollama_messages,
    tools: Some(ollama_tools),
    stream: Some(true), // ← Enable real streaming
    // ...
};

let stream_receiver = self.client.chat_stream(request).await?;
Ok(ProviderResponse::OllamaStreaming(stream_receiver))
```

**Why Streaming is Better:**
1. **Solves Tool Result Issue**: Tool results flow naturally through stream
2. **Authentic Experience**: Real streaming text, not simulated
3. **Cleaner Architecture**: Eliminates Mock event simulation
4. **Natural Tool Flow**: Tool calls come in proper stream order
5. **Consistent with AWS**: Same streaming patterns

**Implementation Steps:**
1. **Enable streaming** in `send_message_ollama_internal()`
2. **Implement proper stream parsing** for newline-delimited JSON
3. **Update response routing** to use `OllamaStreaming` variant
4. **Remove Mock simulation** code from `providers/mod.rs`
5. **Test tool execution** end-to-end

### Acceptance Criteria
- [ ] Tool calls in Ollama responses are detected and processed
- [ ] Tool execution works end-to-end (request → execution → result → response)
- [ ] No more blank responses when models attempt to use tools
- [ ] Tool results are properly integrated into conversation flow
- [ ] Multi-tool scenarios work correctly
- [ ] Error handling for tool execution failures
- [ ] Built-in tools (fs_read, fs_write, execute_bash, use_aws) work with Ollama
- [ ] Tool execution corruption issues resolved

### Testing Strategy
1. **Direct API Testing**: Verify Ollama generates tool calls correctly
2. **Unit Tests**: Test tool call detection and conversion
3. **Integration Tests**: End-to-end tool execution scenarios
4. **Manual Testing**: User workflow testing with various tools

### Success Criteria
```bash
User: "Create a file called test.txt"
Assistant: [Executes fs_write tool visibly]
Assistant: "I've created the file test.txt for you."
# File actually exists on filesystem

User: "List the current directory"
Assistant: [Executes execute_bash tool]
Assistant: "Here are the files in the current directory: ..."
# Shows actual directory listing
```

### Files to Modify
- `crates/chat-cli/src/providers/ollama/mod.rs` - Enable streaming in send_message
- `crates/chat-cli/src/providers/ollama/types.rs` - Implement proper stream parsing
- `crates/chat-cli/src/providers/mod.rs` - Remove Mock simulation code
- Add comprehensive error handling and logging for streaming

### Definition of Done
- [ ] Tool calls work end-to-end with Ollama models
- [ ] No blank responses when tools are invoked
- [ ] Tool execution pipeline is robust and error-free
- [ ] All built-in tools work correctly
- [ ] Comprehensive test coverage
- [ ] User can successfully use tools in conversation

### Priority: CRITICAL
This is a critical bug that breaks core functionality. Tool calls are fundamental to the Q CLI experience, and their complete failure makes Ollama integration unusable for most practical scenarios.

---

## Task 8: Multi-turn Tool Conversations

[To define]

---

## Task 9: Error Handling for Unsupported Models

### Description
Implement proper error handling and capability checking for models that don't support tools, providing clear user feedback instead of cryptic Ollama server errors.

### Current Problem
- Code always sends tools to every model regardless of capability
- Models without tool support (e.g., deepseek-r1:8b) return HTTP 400 errors
- Error: `"registry.ollama.ai/library/deepseek-r1:8b does not support tools"`
- User gets cryptic error instead of working chat functionality

### Research Findings
**Models WITH tool support:**
- `gpt-oss:120b` capabilities: `["completion", "tools", "thinking"]` ✅
- `llama3.2` capabilities: `["completion", "tools"]` ✅

**Models WITHOUT tool support:**
- `deepseek-r1:8b` capabilities: `["completion", "thinking"]` ❌ (no "tools")
- Many older/specialized models lack tool support

**Existing Infrastructure:**
- We already have `supports_capability()` method in `OllamaClient`
- `/api/show` endpoint provides capability detection
- Need to integrate into `send_message` flow

### Implementation Plan

#### **Phase 1: Core Capability Check**
Modify `send_message` in `crates/chat-cli/src/providers/ollama/mod.rs`:

```rust
async fn send_message(&self, conversation: ConversationState) -> Result<crate::providers::ProviderResponse, ApiClientError> {
    let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
    
    // NEW: Check if model supports tools before including them
    let ollama_tools = if self.client.supports_capability(&model, "tools").await.unwrap_or(false) {
        Some(self.get_ollama_tools(&model).await?)
    } else {
        tracing::info!("Model {} does not support tools, proceeding with chat-only mode", model);
        None
    };
    
    let request = OllamaChatRequest {
        model,
        messages: ollama_messages,
        tools: ollama_tools, // ← Conditional based on capability
        stream: Some(true),
        format: None,
        options: None,
        keep_alive: None,
    };
    
    let stream_receiver = self.client.chat_stream(request).await?;
    Ok(crate::providers::ProviderResponse::Streaming(Box::new(stream_receiver)))
}
```

#### **Phase 2: Enhanced Error Handling**
Add robust error handling for capability detection:

```rust
async fn check_model_supports_tools(&self, model: &str) -> bool {
    match self.client.supports_capability(model, "tools").await {
        Ok(supports) => supports,
        Err(e) => {
            tracing::warn!("Failed to check tool capability for model {}: {}. Assuming no tool support.", model, e);
            false // Safe default: assume no tools
        }
    }
}
```

#### **Phase 3: User Experience Improvements**
Add user-friendly messaging and model capability display.

### Testing Strategy

#### **Manual Test Cases**
```bash
# Test 1: Non-tool model should work for basic chat
Q_CLI_MODEL_PROVIDER=ollama q chat
/model
# Select deepseek-r1:8b
hello!  # Should work without HTTP 400 error

# Test 2: Tool-capable model should include tools
Q_CLI_MODEL_PROVIDER=ollama q chat
/model  
# Select gpt-oss:120b
create a file test.txt  # Should work with tools

# Test 3: Network failure handling
# Stop Ollama server temporarily during capability check
# Should default to no tools, not crash
```

#### **Unit Tests**
```rust
#[tokio::test]
async fn test_model_without_tools_works() {
    let provider = create_test_ollama_provider().await;
    
    // Mock model that doesn't support tools
    let conversation = create_test_conversation();
    let result = provider.send_message(conversation).await;
    
    assert!(result.is_ok());
    // Verify request was sent without tools
}

#[tokio::test]
async fn test_capability_check_failure_defaults_to_no_tools() {
    // Test network failure during capability check
    // Should not crash, should default to no tools
}
```

### Acceptance Criteria
- [ ] Models without tool support work for basic chat (no HTTP 400 errors)
- [ ] Models with tool support continue to work with tools unchanged
- [ ] Clear logging when tools are disabled for a model
- [ ] Graceful degradation when capability check fails (network issues)
- [ ] No breaking changes to existing tool-capable workflows
- [ ] Reasonable performance (capability check doesn't slow down chat significantly)

### Files to Modify
- `crates/chat-cli/src/providers/ollama/mod.rs` - Main capability check logic
- `crates/chat-cli/src/providers/ollama/client.rs` - Enhanced error handling (if needed)
- Add comprehensive tests for capability detection scenarios

### Success Criteria
**Before Fix:**
```
!> hello!
Amazon Q is having trouble responding right now:
   0: Failed to send the request: Invalid configuration: Ollama server error 400: {"error":"registry.ollama.ai/library/deepseek-r1:8b does not support tools"}
```

**After Fix:**
```
!> hello!
Hello! How can I help you today?
```

### Definition of Done
- [ ] deepseek-r1:8b and other non-tool models work for basic chat
- [ ] No HTTP 400 "does not support tools" errors
- [ ] Tool-capable models (gpt-oss:120b) continue working with tools
- [ ] Appropriate logging about tool availability
- [ ] Comprehensive test coverage for edge cases
- [ ] User can have normal conversations with any Ollama model

---

## Task 10: Enhanced Thinking Tool Capability Detection

### Description
Add support for the experimental thinking tool to Ollama providers, with proper capability detection and settings integration. The thinking tool allows models to reason through complex problems during response generation.

### Current Problem
- Thinking tool exists in core CLI but is NOT exposed to Ollama models
- TODO comment in `get_ollama_tools()` method indicates this was planned
- Models with "thinking" capability (like deepseek-r1:8b) can't use the thinking tool
- No integration with `chat.enableThinking` setting for Ollama providers

### Research Findings
**Models WITH thinking capability:**
- `deepseek-r1:8b` capabilities: `["completion", "thinking"]` ✅
- `gpt-oss:20b` capabilities: `["completion", "tools", "thinking"]` ✅

**Models WITHOUT thinking capability:**
- `llama3.2` capabilities: `["completion", "tools"]` ❌ (no "thinking")

**Existing Infrastructure:**
- ✅ Thinking tool implementation in `crates/chat-cli/src/cli/chat/tools/thinking.rs`
- ✅ Settings support via `chat.enableThinking` 
- ✅ Capability detection via `supports_capability()` method
- ✅ Tool parsing logic already handles "thinking" tool

### Implementation Plan

#### **Phase 1: Store Database in OllamaProvider**
Add database access to the provider during construction:

```rust
// In providers/ollama/mod.rs - OUR CODE
pub struct OllamaProvider {
    client: OllamaClient,
    base_url: String,
    database: Database, // ← Add this field
}

impl OllamaProvider {
    pub fn new(base_url: String, database: Database) -> Self {
        let client = OllamaClient::new(base_url.clone());
        Self { client, base_url, database } // ← Store database
    }
}
```

#### **Phase 2: Update ApiClient Constructor**
Minimal change to pass database to provider:

```rust
// In api_client/mod.rs - MINIMAL UPSTREAM TOUCH
ModelProvider::Ollama => {
    let base_url = env.get("Q_CLI_MODEL_PROVIDER_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    Some(Box::new(OllamaProvider::new(base_url, database.clone())))
    //                                           ^^^^^^^^^^^^^^^^ Add this
},
```

#### **Phase 3: Add Thinking Tool with Capability Detection**
Update `get_ollama_tools()` method to check both capability and settings:

```rust
// In providers/ollama/mod.rs - OUR CODE
async fn get_ollama_tools(&self, model: &str) -> Result<Vec<OllamaTool>, ApiClientError> {
    let mut tools = Vec::new();
    
    // ... existing tools ...
    
    // Add thinking tool if model supports it AND setting is enabled
    if self.client.supports_capability(model, "thinking").await.unwrap_or(false) 
       && self.database.settings.get_bool(Setting::EnabledThinking).unwrap_or(false) {
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "thinking".to_string(),
                description: "Allows the model to reason through complex problems during response generation".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "thought": {
                            "type": "string",
                            "description": "The thought content that the model wants to process"
                        }
                    },
                    "required": ["thought"]
                }),
            },
        });
    }
    
    Ok(tools)
}
```

### Testing Strategy

#### **Manual Test Cases**
```bash
# Test 1: Enable thinking and use model with thinking capability
q settings chat.enableThinking true
Q_CLI_MODEL_PROVIDER=ollama q chat --model deepseek-r1:8b
# Model should have access to thinking tool

# Test 2: Disable thinking setting
q settings chat.enableThinking false  
Q_CLI_MODEL_PROVIDER=ollama q chat --model deepseek-r1:8b
# Model should NOT have thinking tool

# Test 3: Model without thinking capability
q settings chat.enableThinking true
Q_CLI_MODEL_PROVIDER=ollama q chat --model llama3.2
# Model should NOT have thinking tool (no capability)

# Test 4: Model with both tools and thinking
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b  
# Model should have both regular tools AND thinking tool
```

#### **Unit Tests**
```rust
#[tokio::test]
async fn test_thinking_tool_capability_detection() {
    // Test that thinking tool is included when:
    // 1. Model supports "thinking" capability
    // 2. Settings enable thinking
    
    // Test that thinking tool is excluded when:
    // 1. Model doesn't support "thinking" capability
    // 2. Settings disable thinking
}
```

### Acceptance Criteria
- [ ] Models with "thinking" capability get thinking tool when `chat.enableThinking` is true
- [ ] Models without "thinking" capability never get thinking tool
- [ ] Setting `chat.enableThinking false` disables thinking tool for all Ollama models
- [ ] Existing tool functionality unchanged
- [ ] Graceful error handling for capability check failures
- [ ] Clear logging about thinking tool availability

### Files to Modify
- `crates/chat-cli/src/providers/ollama/mod.rs` - Add database field and thinking tool logic
- `crates/chat-cli/src/api_client/mod.rs` - Pass database.clone() to OllamaProvider constructor (minimal change)
- Add comprehensive tests for thinking tool integration

### Success Criteria
**Before Fix:**
```
# deepseek-r1:8b model can't use thinking tool
# No thinking capability exposed to Ollama models
```

**After Fix:**
```
# deepseek-r1:8b can use thinking tool when enabled
# Proper capability detection and settings integration
# Models without thinking capability work normally
```

### Definition of Done
- [ ] Thinking tool available to capable Ollama models when enabled
- [ ] Proper integration with `chat.enableThinking` setting
- [ ] Capability detection prevents thinking tool on unsupported models
- [ ] All existing functionality preserved
- [ ] Comprehensive test coverage
- [ ] Clear user feedback about thinking tool availability

## Task 12: Configurable Reasoning Effort for Thinking Models

### Description
Add user-configurable reasoning effort settings for thinking-capable Ollama models (like gpt-oss). This allows users to control the quality vs. speed trade-off when using thinking models.

### Current Problem
- Task 10 implements basic thinking tool with hardcoded "medium" reasoning effort
- gpt-oss models support configurable reasoning effort ("low", "medium", "high") 
- No user configuration available for reasoning effort levels
- Users can't optimize for their specific use case (speed vs. quality)

### Research Findings
**Ollama API Support:**
- `think` parameter accepts boolean or string ("high", "medium", "low")
- Can be set via top-level `think` field or `options.reasoning`
- OpenAI compatibility via `reasoning_effort` parameter

**Current Q CLI:**
- ✅ `chat.enableThinking` setting exists
- ❌ No `chat.reasoningEffort` setting exists
- Would require minimal upstream change to add new setting

### Implementation Plan

#### **Phase 1: Add Setting Support**
Add new setting with minimal upstream touch:

```rust
// In settings.rs - MINIMAL UPSTREAM CHANGE
pub enum Setting {
    // ... existing settings
    ChatReasoningEffort,  // ← Add this
}

impl AsRef<str> for Setting {
    fn as_ref(&self) -> &'static str {
        match self {
            // ... existing mappings  
            Self::ChatReasoningEffort => "chat.reasoningEffort", // ← Add this
        }
    }
}
```

#### **Phase 2: Update Ollama Provider**
Use setting in our provider:

```rust
// In providers/ollama/mod.rs - OUR CODE
let reasoning_effort = self.database.settings
    .get_string(Setting::ChatReasoningEffort)
    .unwrap_or_else(|_| "medium".to_string());

let request = OllamaChatRequest {
    think: Some(reasoning_effort), // ← Use setting value
    // ... rest of request
};
```

#### **Phase 3: Add Validation**
Ensure only valid values are accepted:

```rust
// Validation for "low", "medium", "high" values
// Clear error messages for invalid values
// Default to "medium" if unset
```

### Testing Strategy

#### **Manual Test Cases**
```bash
# Test 1: Set high reasoning effort
q settings chat.reasoningEffort high
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
# Should use high reasoning effort (better quality, slower)

# Test 2: Set low reasoning effort  
q settings chat.reasoningEffort low
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
# Should use low reasoning effort (faster, lower quality)

# Test 3: Invalid value
q settings chat.reasoningEffort invalid
# Should show error: "Valid values: low, medium, high"

# Test 4: Default behavior
q settings chat.reasoningEffort --unset
# Should default to "medium"
```

### Acceptance Criteria
- [ ] New `chat.reasoningEffort` setting available via `q settings`
- [ ] Valid values: "low", "medium", "high" 
- [ ] Default value: "medium"
- [ ] Invalid values show helpful error message with valid options
- [ ] Setting applies to all thinking-capable Ollama models
- [ ] Non-thinking models ignore the setting gracefully
- [ ] Existing thinking tool functionality unchanged

### Files to Modify
- `crates/chat-cli/src/database/settings.rs` - Add new setting (minimal upstream touch)
- `crates/chat-cli/src/providers/ollama/mod.rs` - Use setting in requests
- Add validation and comprehensive tests

### Success Criteria
**Before Task 12:**
```bash
# Hardcoded "medium" reasoning effort
# No user control over reasoning quality vs. speed
```

**After Task 12:**
```bash
q settings chat.reasoningEffort high  # Better quality, slower
q settings chat.reasoningEffort low   # Faster responses, lower quality
q settings chat.reasoningEffort medium # Balanced (default)
```

### Dependencies
- **Requires**: Task 10 (basic thinking tool support) completed
- **Research**: See `task12_research.md` for detailed technical analysis

### Definition of Done
- [ ] Users can configure reasoning effort via settings
- [ ] Setting validation prevents invalid values
- [ ] Default behavior is reasonable ("medium")
- [ ] All existing functionality preserved
- [ ] Comprehensive test coverage
- [ ] Clear documentation of reasoning effort trade-offs

---

## Task 13: Accurate Context Window Reporting for Ollama Models

### Description
Fix context window reporting for Ollama models by dynamically querying the Ollama API for real model metadata instead of using hardcoded 200K defaults. This ensures `/usage` command shows accurate token limits and usage percentages.

### Current Problem
- gpt-oss models have 128K token context window but Q CLI reports 200K tokens
- Usage percentage calculations are wrong (9.70% vs actual ~15.1%)
- Users get misleading information about available context space
- All Ollama models default to hardcoded 200K context window

### Root Cause Analysis
**Three sources of hardcoded 200K limits:**
1. `default_context_window()` function returns 200K
2. Ollama model creation hardcodes `context_window_tokens: 200_000` (line 186)
3. `ModelInfo::from_id()` fallback uses 200K

**AWS vs Ollama difference:**
- AWS models query `model.token_limits().max_input_tokens()` for real limits
- Ollama models use hardcoded 200K with no API querying

### Implementation Plan

#### **Phase 1: Add Dynamic Querying**
Enhance `OllamaClient` to query model metadata:

```rust
impl OllamaClient {
    pub async fn get_model_context_window(&self, model: &str) -> Result<Option<usize>, OllamaError> {
        let model_info = self.get_model_info(model).await?;
        
        // Extract context window from model_info.model_info fields
        // Try: "gpt-oss.context_length", "context_length", "max_position_embeddings", "n_ctx"
        // Return actual context window size or None if not found
    }
}
```

#### **Phase 2: Update Model Creation**
Replace hardcoded 200K in Ollama model creation (line 186):

```rust
// Instead of: context_window_tokens: 200_000
let context_window_tokens = if let Some(provider) = os.client.ollama_client() {
    provider.get_model_context_window(&name).await
        .unwrap_or(None)
        .unwrap_or(200_000) // Fallback if query fails
} else {
    200_000 // Fallback if no Ollama client
};
```

#### **Phase 3: Add Caching & Error Handling**
- Cache model metadata to avoid repeated API calls
- Graceful error handling for network failures (fallback to 200K)
- Retry logic for temporary failures

### Testing Strategy

#### **Manual Test Cases**
```bash
# Test 1: gpt-oss shows correct context window
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
/usage
# Should show "128k tokens" not "200k tokens"

# Test 2: Usage percentage accuracy
# With 19390 tokens used:
# Before: 19390/200000 = 9.70%
# After:  19390/128000 = 15.1%

# Test 3: Network failure graceful fallback
# Stop Ollama server, should fall back to 200K without crashing
```

### Acceptance Criteria
- [ ] `/usage` shows correct context window for all Ollama models
- [ ] Usage percentages calculated accurately based on real model limits
- [ ] Dynamic querying via Ollama `/api/show` endpoint
- [ ] Graceful fallback to 200K when query fails or model unknown
- [ ] Caching to avoid repeated API calls
- [ ] Works for gpt-oss (128K), llama3.2 (32K), and other models
- [ ] No performance degradation or blocking behavior

### Files to Modify
- `crates/chat-cli/src/providers/ollama/client.rs` - Add context window querying
- `crates/chat-cli/src/providers/ollama/types.rs` - Add model info response types
- `crates/chat-cli/src/cli/chat/cli/model.rs` - Update Ollama model creation (line 186)
- Add comprehensive tests for dynamic querying and fallbacks

### Success Criteria
**Before Fix:**
```bash
Current context window (19390 of 200k tokens used) 9.70%
```

**After Fix:**
```bash
Current context window (19390 of 128k tokens used) 15.1%
```

### Dependencies
- **Requires**: Existing Ollama client and `/api/show` endpoint
- **Research**: See `task13_research.md` for detailed technical analysis
- **Enhances**: User understanding of actual context limits

### Definition of Done
- [ ] Dynamic querying of Ollama model metadata implemented
- [ ] Context window reporting accurate for all supported models
- [ ] Graceful error handling and fallbacks
- [ ] Performance optimized with caching
- [ ] Comprehensive test coverage
- [ ] Clear logging for debugging context window detection

---
## Task 11: MCP Tools Integration with Ollama

### Description
Integrate MCP (Model Context Protocol) tools with Ollama provider so that tools from MCP servers (like `convert_to_markdown` from fetch server) are visible and usable by Ollama models, not just built-in tools.

### Current Problem
- Built-in tools (fs_read, execute_bash, fs_write, use_aws) work with Ollama ✅
- MCP tools (convert_to_markdown, etc.) are visible in `/tools` command ✅
- But MCP tools are NOT exposed to Ollama models in the system prompt ❌
- Ollama models can only see and use built-in tools, missing MCP functionality

### Research Findings
- MCP tools are managed separately from built-in tools
- `/tools` command shows both built-in and MCP tools correctly
- Ollama tool mapping in `get_ollama_tools()` only includes built-in tools
- Need to discover and include MCP tools in Ollama tool definitions

### Implementation Approach
```rust
// In get_ollama_tools() method
async fn get_ollama_tools(&self, model: &str) -> Result<Vec<OllamaTool>, ApiClientError> {
    let mut tools = Vec::new();
    
    // Add built-in tools (existing)
    tools.extend(self.get_builtin_ollama_tools());
    
    // NEW: Add MCP tools
    if let Some(mcp_tools) = self.get_mcp_ollama_tools().await? {
        tools.extend(mcp_tools);
    }
    
    Ok(tools)
}

async fn get_mcp_ollama_tools(&self) -> Result<Option<Vec<OllamaTool>>, ApiClientError> {
    // Discover MCP servers and their tools
    // Convert MCP tool definitions to Ollama format
    // Return tools that Ollama can invoke
}
```

### Acceptance Criteria
- [ ] MCP tools are discoverable by Ollama provider
- [ ] MCP tools are included in Ollama tool definitions sent to models
- [ ] Ollama models can see and invoke MCP tools (e.g., convert_to_markdown)
- [ ] MCP tool calls work end-to-end (request → MCP server → response)
- [ ] Tool results are properly integrated into conversation
- [ ] Error handling for MCP server failures
- [ ] All existing built-in tool functionality preserved

### Implementation Details

#### 1. MCP Tool Discovery
Need to integrate with existing MCP client system to discover available tools from running MCP servers.

#### 2. Tool Definition Conversion
Convert MCP tool schemas to Ollama-compatible tool definitions:
```rust
// MCP tool schema → Ollama tool schema
fn convert_mcp_tool_to_ollama(mcp_tool: McpTool) -> OllamaTool {
    OllamaTool {
        tool_type: "function".to_string(),
        function: OllamaFunction {
            name: mcp_tool.name,
            description: mcp_tool.description,
            parameters: convert_mcp_schema_to_json_schema(mcp_tool.input_schema),
        },
    }
}
```

#### 3. Tool Execution Integration
Ensure MCP tool calls from Ollama are properly routed to MCP servers and results returned.

### Files to Investigate/Modify
- `crates/chat-cli/src/providers/ollama/mod.rs` - Add MCP tool discovery
- `crates/chat-cli/src/mcp_client/` - Integration with MCP system
- Tool execution pipeline - Ensure MCP tools work with Ollama

### Definition of Done
- [ ] `/tools` command shows MCP tools available to Ollama
- [ ] Ollama models can invoke MCP tools in conversation
- [ ] MCP tool execution works end-to-end
- [ ] Error handling for MCP failures
- [ ] All existing functionality preserved
- [ ] Integration tests demonstrate MCP tools working with Ollama

### Expected Outcome
After Task 11, Ollama models will have access to the full ecosystem of MCP tools, making them as capable as AWS models in terms of available functionality.
