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
│   ├── mod.rs                       # Main API client
│   ├── model.rs                     # Data models and types
│   ├── credentials.rs               # AWS credential management
│   └── error.rs                     # Error handling
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

### 1. Modular Crate Design
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
3. **`api_client/mod.rs`**: AWS service integration
4. **`mcp_client/mod.rs`**: MCP protocol implementation
5. **`database/mod.rs`**: Local data persistence
6. **Agent configs in user directory**: Real-world configuration examples