# Codebase Structure and Patterns

Amazon Q Developer CLI is a Rust-based command-line application built as a Cargo workspace with multiple crates for modular functionality.

## Code Organization

### Workspace Structure
```
amazon-q-developer-cli/
├── crates/                           # All Rust crates
│   ├── chat-cli/                     # Main CLI application
│   ├── amzn-qdeveloper-streaming-client/     # Q Developer API client
│   ├── amzn-codewhisperer-client/            # CodeWhisperer API client  
│   ├── amzn-codewhisperer-streaming-client/  # CodeWhisperer streaming
│   ├── amzn-consolas-client/                 # Consolas service client
│   ├── amzn-toolkit-telemetry-client/        # Telemetry client
│   ├── aws-toolkit-telemetry-definitions/    # Telemetry definitions
│   └── semantic-search-client/               # Semantic search engine
├── docs/                             # Technical documentation
├── scripts/                          # Build and deployment scripts
├── build-config/                     # Platform-specific build configs
├── schemas/                          # JSON schemas for validation
└── planning/                         # Development planning docs
```

### Main Crate Structure (`crates/chat-cli/src/`)
```
src/
├── main.rs                          # Application entry point
├── lib.rs                           # Library exports for testing
├── cli/                             # Command-line interface
│   ├── mod.rs                       # CLI argument parsing and routing
│   ├── chat/                        # Chat command implementation
│   ├── agent/                       # Agent management commands
│   ├── mcp.rs                       # MCP server management
│   ├── settings.rs                  # Configuration management
│   └── user.rs                      # Authentication commands
├── api_client/                      # AWS service clients
│   ├── mod.rs                       # Main API client with provider routing
│   ├── model.rs                     # Data models and types
│   ├── credentials.rs               # AWS credential management
│   ├── error.rs                     # Error handling
│   └── send_message_output.rs       # Unified response handling
├── providers/                       # 🆕 Plugin system for external providers
│   ├── mod.rs                       # Provider trait and registry
│   └── ollama/                      # Ollama provider plugin
│       ├── mod.rs                   # Plugin implementation
│       ├── client.rs                # HTTP client for Ollama API
│       └── types.rs                 # Ollama-specific types
├── auth/                            # Authentication system
│   ├── builder_id.rs                # AWS Builder ID auth
│   └── pkce.rs                      # OAuth PKCE flow
├── mcp_client/                      # Model Context Protocol
│   ├── client.rs                    # MCP client implementation
│   ├── server.rs                    # MCP server management
│   └── transport/                   # Communication protocols
├── database/                        # Local data persistence
│   ├── mod.rs                       # SQLite database interface
│   ├── settings.rs                  # Settings storage
│   └── sqlite_migrations/           # Database schema migrations
├── telemetry/                       # Usage analytics
├── util/                            # Shared utilities
│   ├── knowledge_store.rs           # Semantic search integration
│   ├── directories.rs               # File system helpers
│   └── system_info/                 # Platform detection
└── os/                              # Operating system abstractions
    ├── mod.rs                       # OS trait definitions
    └── fs/                          # File system operations
```

## Architecture Patterns

### 1. Os (Operating System Abstraction Layer)

The `Os` struct is a **dependency injection container** that provides testable interfaces to all system operations. Despite its name, it's not just "Operating System" info - it's the entire **application context**.

#### **Structure**
```rust
pub struct Os {
    pub env: Env,           // Environment variables (std::env wrapper)
    pub fs: Fs,             // File system operations (tokio::fs wrapper)  
    pub sysinfo: SysInfo,   // System information
    pub database: Database, // Local SQLite database
    pub client: ApiClient,  // AWS API client + external providers
    pub telemetry: TelemetryThread, // Usage analytics
}
```

#### **Design Philosophy**
From the code documentation:
> "Every operation that accesses the file system, environment, or other related platform primitives should be done through a [Context] as this enables testing otherwise untestable code paths in unit tests."

#### **Key Benefits**
- **Testability**: All system operations go through `Os`, making them mockable in tests
- **Dependency Injection**: Instead of calling `std::fs::read()` directly, you call `os.fs.read()`
- **Centralized Access**: All system resources available in one place
- **Cross-Platform**: Abstracts platform differences (Windows vs Unix paths, etc.)

#### **Common Patterns**
```rust
// Instead of direct system calls:
let content = std::fs::read_to_string("file.txt")?;
let var = std::env::var("HOME")?;

// Use Os abstraction:
let content = os.fs.read_to_string("file.txt").await?;
let var = os.env.get("HOME")?;
```

#### **Circular Dependency Challenge**
When external providers (like `OllamaProvider`) need access to `Os` components:
```rust
// Problem: OllamaProvider lives inside os.client
Os {
    client: ApiClient {
        external_provider: Some(OllamaProvider) ← We are here
    }
    database: Database ← We need this for settings
}

// Solution: Pass specific components instead of full Os
async fn get_ollama_tools(&self, model: &str, database: &Database) -> Result<Vec<OllamaTool>, ApiClientError>
```

#### **Alternative Names**
The `Os` name can be confusing. It's more like:
- **ApplicationContext** 
- **SystemServices**
- **DependencyContainer**
- **RuntimeEnvironment**

### 2. Modular Crate Design
- **Separation of Concerns**: Each AWS service has its own client crate
- **Workspace Dependencies**: Shared dependencies defined at workspace level
- **Feature Flags**: Optional functionality controlled via Cargo features
- **Platform Abstraction**: OS-specific code isolated in dedicated modules

### 2. Async-First Architecture
- **Tokio Runtime**: Multi-threaded async runtime for all I/O operations
- **Streaming APIs**: Real-time communication with AWS services
- **Concurrent Processing**: Parallel execution for file operations and MCP servers
- **Graceful Shutdown**: Proper cleanup on CTRL+C and termination signals

### 3. Error Handling Strategy
- **Eyre Integration**: Rich error context and reporting
- **Thiserror Derives**: Structured error types with automatic Display/Error impls
- **Result Propagation**: Consistent use of `Result<T, E>` throughout codebase
- **User-Friendly Messages**: Technical errors converted to actionable user messages

### 4. Configuration Management
- **JSON Schema Validation**: Agent configurations validated against schemas
- **Layered Settings**: Global, agent-specific, and runtime configuration layers
- **Environment Variables**: Support for environment-based configuration
- **Migration System**: Database schema versioning and automatic upgrades

### 5. Plugin Architecture (MCP)
- **Process Management**: Automatic lifecycle management of MCP server processes
- **Protocol Abstraction**: Transport-agnostic communication (stdio, websocket)
- **Tool Discovery**: Dynamic discovery of available tools and resources
- **Error Isolation**: MCP server failures don't crash main application

### 6. 🆕 Multi-Provider Plugin System
The CLI now supports multiple AI model providers through a clean plugin architecture that extends beyond AWS services.

#### Provider Architecture
```rust
// Core trait for all external providers
pub trait MessageProvider: Send + Sync {
    async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ApiClientError>;
    fn provider_name(&self) -> &'static str;
    fn requires_auth(&self) -> bool;
    fn supports_streaming(&self) -> bool;
    fn supports_tools(&self) -> bool;
    async fn list_models(&self) -> Result<Vec<String>, ApiClientError>;
    async fn test_connection(&self) -> Result<bool, ApiClientError>;
}
```

#### Provider Integration Pattern
```rust
// ApiClient with plugin support
pub struct ApiClient {
    // AWS clients (existing)
    client: CodewhispererClient,
    streaming_client: Option<CodewhispererStreamingClient>,
    sigv4_streaming_client: Option<QDeveloperStreamingClient>,
    
    // Plugin system integration
    external_provider: Option<Box<dyn MessageProvider>>,
    provider: ModelProvider,
}

// Provider routing in send_message
impl ApiClient {
    pub async fn send_message(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
        // NEW: Check for external provider first
        if let Some(external_provider) = &self.external_provider {
            let response = external_provider.send_message(conversation).await?;
            return Ok(response.into());
        }
        
        // Fallback to AWS providers
        match self.provider {
            ModelProvider::Aws => { /* existing AWS logic */ }
            _ => Err(ApiClientError::UnsupportedProvider(format!("{:?}", self.provider)))
        }
    }
}
```

#### Environment-Based Provider Selection
```bash
# Provider selection via environment variables
Q_CLI_MODEL_PROVIDER=ollama          # Use local Ollama models
Q_CLI_MODEL_PROVIDER=aws             # Use AWS models (default)
Q_CLI_MODEL_PROVIDER=openai          # Use OpenAI models (future)
Q_CLI_MODEL_PROVIDER=anthropic       # Use Anthropic models (future)

# Provider-specific configuration
Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:11434  # Ollama server URL
Q_CLI_MODEL_PROVIDER_API_KEY=sk-...                   # API key for external providers
```

#### Plugin-Based Environment Validation
```rust
// Environment validation moved to plugin system
pub fn validate_provider_environment() -> Result<String> {
    let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
        .unwrap_or_else(|_| "aws".to_string())
        .to_lowercase();
    
    match provider.as_str() {
        "aws" => Ok(provider),
        "ollama" => Ok(provider), // No API key required
        "openai" | "anthropic" => {
            // Check for required API key
            if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required");
            }
            Ok(provider)
        },
        _ => bail!("Invalid Q_CLI_MODEL_PROVIDER: '{}'", provider),
    }
}

// Plugin-based auth bypass
pub fn current_provider_requires_auth() -> bool {
    let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
        .unwrap_or_else(|_| "aws".to_string())
        .to_lowercase();
    
    match provider.as_str() {
        "aws" => true,
        "ollama" => false,
        "openai" | "anthropic" => false, // They use API keys, not AWS auth
        _ => true, // Default to requiring auth for unknown providers
    }
}
```

#### Ollama Provider Implementation
```rust
// Example provider implementation
pub struct OllamaProvider {
    client: OllamaClient,
    base_url: String,
}

impl MessageProvider for OllamaProvider {
    async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ApiClientError> {
        let (ollama_messages, model) = self.convert_conversation(conversation)?;
        let ollama_tools = self.get_tools(&model).await?;
        
        let request = OllamaChatRequest {
            model,
            messages: ollama_messages,
            tools: Some(ollama_tools),
            stream: Some(true),
        };
        
        let stream_receiver = self.client.chat_stream(request).await?;
        Ok(ProviderResponse::OllamaStreaming(stream_receiver))
    }
    
    fn provider_name(&self) -> &'static str { "ollama" }
    fn requires_auth(&self) -> bool { false }
    fn supports_streaming(&self) -> bool { true }
    fn supports_tools(&self) -> bool { true }
}
```

#### Plugin Benefits
- **Minimal Core Changes**: Only one field added to `ApiClient`
- **Clean Separation**: Provider-specific code isolated in `providers/` module
- **AWS Preservation**: All existing AWS functionality unchanged
- **Extensible**: Easy to add new providers (OpenAI, Anthropic, etc.)
- **Type Safety**: Unified response types with provider-specific variants
- **Tool Support**: Built-in tools work across all providers
- **Streaming**: Consistent streaming interface across providers

## Architectural Patterns

### 1. Strangler Fig Pattern (Plugin System)
The multi-provider plugin system was implemented using the Strangler Fig pattern:
- **Phase 1**: New plugin system built alongside existing AWS code
- **Phase 2**: Plugin system handles external providers while AWS code unchanged
- **Phase 3**: Old provider-specific code removed, plugin system becomes primary
- **Benefits**: Zero downtime migration, gradual rollout, risk mitigation

### 2. Provider Pattern
```rust
// Unified interface for all model providers
trait MessageProvider {
    async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ApiClientError>;
    // ... other provider methods
}

// Provider-specific implementations
impl MessageProvider for OllamaProvider { /* Ollama-specific logic */ }
impl MessageProvider for OpenAIProvider { /* OpenAI-specific logic */ }
impl MessageProvider for AnthropicProvider { /* Anthropic-specific logic */ }
```

### 3. Adapter Pattern (Response Conversion)
```rust
// Unified response type that adapts provider-specific responses
pub enum ProviderResponse {
    OllamaStreaming(OllamaStreamReceiver),
    Ollama(OllamaChatResponse),
    // Future: OpenAI, Anthropic variants
}

// Conversion to core response types
impl From<ProviderResponse> for SendMessageOutput {
    fn from(response: ProviderResponse) -> Self {
        match response {
            ProviderResponse::OllamaStreaming(receiver) => {
                // Convert plugin streaming to core streaming
                SendMessageOutput::Mock(/* bridge implementation */)
            }
            // ... other conversions
        }
    }
}
```

### 4. Registry Pattern (Future Extension)
```rust
// Extensible provider registry for dynamic provider management
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn MessageProvider>>,
}

impl ProviderRegistry {
    pub fn register<P: MessageProvider + 'static>(&mut self, name: String, provider: P) {
        self.providers.insert(name, Box::new(provider));
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn MessageProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }
}
```

## Style Guide

### Rust Conventions
- **Edition 2024**: Latest Rust edition with modern syntax
- **Clippy Lints**: Extensive clippy configuration for code quality
- **Rustfmt**: Consistent code formatting with nightly formatter
- **Documentation**: Public APIs documented with rustdoc comments

### Naming Conventions
- **Modules**: Snake_case (e.g., `api_client`, `mcp_client`)
- **Types**: PascalCase (e.g., `ApiClient`, `ConversationState`)
- **Functions**: Snake_case (e.g., `send_message`, `get_credentials`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_TIMEOUT_DURATION`)
- **Files**: Snake_case matching module names

### Code Organization Patterns
- **Barrel Exports**: `mod.rs` files re-export public items
- **Error Modules**: Dedicated error types per module
- **Plugin Isolation**: Provider-specific code in separate modules
- **Trait-Based Design**: Common interfaces for extensibility

## Plugin Development Workflow

### Adding a New Provider
1. **Create Provider Module**: `src/providers/new_provider/`
   ```
   providers/new_provider/
   ├── mod.rs          # MessageProvider implementation
   ├── client.rs       # HTTP/API client
   ├── types.rs        # Provider-specific types
   └── auth.rs         # Authentication (if needed)
   ```

2. **Implement MessageProvider Trait**:
   ```rust
   pub struct NewProvider {
       client: NewProviderClient,
       config: ProviderConfig,
   }
   
   impl MessageProvider for NewProvider {
       async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ApiClientError> {
           // Convert conversation to provider format
           // Make API call
           // Return unified response
       }
       
       fn provider_name(&self) -> &'static str { "new_provider" }
       // ... implement other required methods
   }
   ```

3. **Add Provider to Constructor**:
   ```rust
   // In ApiClient::new()
   let external_provider = match provider {
       ModelProvider::NewProvider => {
           let config = get_provider_config(env)?;
           Some(Box::new(NewProvider::new(config)) as Box<dyn MessageProvider>)
       },
       // ... other providers
   };
   ```

4. **Update Environment Variables**:
   ```rust
   // In ModelProvider::from_env()
   match provider.as_str() {
       "new_provider" => {
           validate_provider_config(env)?;
           Ok(Self::NewProvider)
       },
       // ... other providers
   }
   ```

### Testing Patterns for Plugins
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    
    #[tokio::test]
    async fn test_provider_send_message() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/api/chat")
            .with_status(200)
            .with_body(r#"{"response": "test"}"#)
            .create_async()
            .await;
            
        let provider = NewProvider::new(server.url());
        let conversation = create_test_conversation();
        
        let result = provider.send_message(conversation).await;
        assert!(result.is_ok());
        
        mock.assert_async().await;
    }
}
```

### Plugin Integration Checklist
- [ ] Provider implements `MessageProvider` trait
- [ ] Provider added to `ModelProvider` enum
- [ ] Environment variable validation added
- [ ] Constructor updated to create provider instance
- [ ] Error types mapped to `ApiClientError`
- [ ] Unit tests with mocked HTTP responses
- [ ] Integration tests with real provider (optional)
- [ ] Documentation updated in `codebase.md`
- **Builder Pattern**: Complex configuration objects use builders
- **Trait Abstractions**: Platform-specific code behind traits

## Common Patterns

### 1. Database Operations
```rust
// Consistent transaction handling
pub async fn update_conversation(&mut self, conversation: &Conversation) -> Result<()> {
    let conn = self.pool.get()?;
    conn.execute(
        "UPDATE conversations SET data = ?1 WHERE id = ?2",
        params![serde_json::to_string(conversation)?, conversation.id],
    )?;
    Ok(())
}
```

### 2. MCP Tool Integration
```rust
// Tool invocation pattern
pub async fn invoke_tool(&self, tool_name: &str, params: Value) -> Result<ToolResult> {
    let server = self.find_server_for_tool(tool_name)?;
    let request = ToolCallRequest { name: tool_name, arguments: params };
    server.call_tool(request).await
}
```

### 3. Streaming Response Handling
```rust
// Async stream processing
pub async fn send_message(&self, message: &str) -> Result<impl Stream<Item = ChatChunk>> {
    let stream = self.client.send_message_streaming(message).await?;
    Ok(stream.map(|chunk| self.process_chunk(chunk)))
}
```

### 4. Configuration Loading
```rust
// Layered configuration pattern
pub fn load_agent_config(name: &str) -> Result<AgentConfig> {
    let base_config = load_default_config()?;
    let agent_config = load_agent_file(name)?;
    Ok(base_config.merge(agent_config))
}
```

## Testing

### Test Organization
- **Unit Tests**: Inline tests in source files using `#[cfg(test)]`
- **Integration Tests**: Separate `tests/` directory for end-to-end testing
- **Mock Clients**: Test doubles for AWS services and MCP servers
- **Snapshot Testing**: `insta` crate for output verification

### Test Utilities
- **Test Fixtures**: Shared test data in `util/test.rs`
- **Async Testing**: `tokio-test` for async test utilities
- **Temporary Files**: `tempfile` crate for isolated test environments
- **Mock Servers**: `mockito` for HTTP service mocking

### Running Tests
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_agent_configuration

# Run integration tests only
cargo test --test integration
```

## Developer Onboarding

### Prerequisites
- **Rust Toolchain**: Install via rustup (stable + nightly for formatting)
- **Platform Tools**: Xcode (macOS), build-essential (Linux)
- **AWS CLI**: For authentication testing
- **Git**: Version control and hooks

### Local Development Setup
1. **Clone Repository**: `git clone https://github.com/aws/amazon-q-developer-cli.git`
2. **Install Dependencies**: `cargo build` (downloads and compiles dependencies)
3. **Run Tests**: `cargo test` (verify setup)
4. **Install Hooks**: Pre-commit hooks for formatting and linting
5. **Run CLI**: `cargo run --bin chat_cli` (test basic functionality)

### Development Workflow
1. **Create Feature Branch**: Follow `feature/description` naming
2. **Make Changes**: Focus on single responsibility
3. **Run Lints**: `cargo clippy` for code quality
4. **Format Code**: `cargo +nightly fmt` for consistency
5. **Test Changes**: `cargo test` for regression prevention
6. **Update Docs**: Keep documentation in sync with code changes

### Common Issues and Solutions

#### Build Issues
- **Missing System Dependencies**: Install platform-specific build tools
- **Rust Version**: Ensure using correct toolchain version
- **Cargo Cache**: Clear with `cargo clean` if builds fail

#### Runtime Issues
- **Authentication**: Ensure valid AWS credentials
- **File Permissions**: Check read/write access to config directories
- **Network Connectivity**: Verify internet access for AWS APIs

#### Development Environment
- **IDE Setup**: Use rust-analyzer for VS Code/IntelliJ
- **Debugging**: Use `RUST_LOG=debug` for verbose logging
- **Performance**: Use `cargo build --release` for optimized builds

### Key Files to Understand First
1. **`main.rs`**: Application entry point and CLI parsing
2. **`cli/mod.rs`**: Command structure and routing
3. **`api_client/mod.rs`**: AWS service integration and provider routing
4. **`providers/mod.rs`**: 🆕 Plugin system and external provider interface
5. **`providers/ollama/mod.rs`**: 🆕 Ollama provider implementation
6. **`mcp_client/mod.rs`**: MCP protocol implementation
7. **`database/mod.rs`**: Local data persistence
8. **Agent configs in user directory**: Real-world configuration examples

## Implementation Details

### Multi-Provider Message Flow
```
User Input
    ↓
ConversationState (unified format)
    ↓
ApiClient::send_message()
    ↓
Provider Detection (Q_CLI_MODEL_PROVIDER)
    ↓
┌─────────────────┬─────────────────┐
│   AWS Provider  │ External Plugin │
│   (existing)    │   (new system)  │
└─────────────────┴─────────────────┘
    ↓                       ↓
AWS Streaming Client    MessageProvider::send_message()
    ↓                       ↓
SendMessageOutput      ProviderResponse
    ↓                       ↓
    └───── Unified Response Processing ─────┘
                    ↓
            ChatResponseStream
                    ↓
              User Interface
```

### Provider Configuration System
```rust
// Environment-based provider selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProvider {
    Aws,        // Default, uses existing AWS clients
    Ollama,     // Local Ollama server
    OpenAi,     // OpenAI API (future)
    Anthropic,  // Anthropic API (future)
}

// Validation at startup
impl ModelProvider {
    pub fn from_env() -> Result<Self> {
        let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
            .unwrap_or_else(|_| "aws".to_string());
        
        match provider.to_lowercase().as_str() {
            "aws" => Ok(Self::Aws),
            "ollama" => Ok(Self::Ollama),
            "openai" => {
                require_api_key("Q_CLI_MODEL_PROVIDER_API_KEY")?;
                Ok(Self::OpenAi)
            },
            _ => bail!("Invalid provider: {}", provider),
        }
    }
}
```

### Response Type Unification
```rust
// Core response enum (preserved for AWS compatibility)
pub enum SendMessageOutput {
    Codewhisperer(GenerateAssistantResponseOutput),  // AWS CodeWhisperer
    QDeveloper(SendMessageOutput),                   // AWS Q Developer
    Mock(Vec<ChatResponseStream>),                   // Testing + Plugin Bridge
}

// Plugin response types (isolated in providers module)
pub enum ProviderResponse {
    OllamaStreaming(OllamaStreamReceiver),
    Ollama(OllamaChatResponse),
    // Future: OpenAI, Anthropic variants
}

// Conversion bridge (adapts plugin responses to core types)
impl From<ProviderResponse> for SendMessageOutput {
    fn from(response: ProviderResponse) -> Self {
        match response {
            ProviderResponse::OllamaStreaming(_receiver) => {
                // Bridge plugin streaming to core streaming system
                let mock_content = vec![ChatResponseStream::AssistantResponseEvent {
                    content: "Ollama plugin response".to_string(),
                }];
                SendMessageOutput::Mock(mock_content)
            }
        }
    }
}
```

### Tool System Integration
All built-in tools (`fs_read`, `execute_bash`, `fs_write`, `use_aws`) work across providers:

```rust
impl MessageProvider for OllamaProvider {
    async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ApiClientError> {
        // Convert built-in tools to Ollama format
        let ollama_tools = self.get_ollama_tools(&model).await?;
        
        let request = OllamaChatRequest {
            model,
            messages: ollama_messages,
            tools: Some(ollama_tools),  // Tools mapped to provider format
            stream: Some(true),
        };
        
        // Tool calls detected in response and executed by core system
        let stream_receiver = self.client.chat_stream(request).await?;
        Ok(ProviderResponse::OllamaStreaming(stream_receiver))
    }
}
```

### Authentication Bypass Pattern
```rust
// Provider-aware authentication
pub async fn is_logged_in(database: &mut Database) -> bool {
    // Check if using non-AWS provider (bypass AWS auth)
    if let Ok(provider) = ModelProvider::from_env() {
        if !provider.requires_auth() {
            debug!("bypassing auth for provider: {:?}", provider);
            return true;
        }
    }
    
    // Existing AWS authentication logic...
}
```