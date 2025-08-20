# Amazon Q CLI - Ollama Support for Offline Development

## Overview
Add support for using local Ollama models as an alternative to AWS-hosted models, enabling developers to work offline or in environments where AWS connectivity is limited. This feature introduces a pluggable model provider architecture that can be extended to support additional providers (OpenAI, Anthropic) in the future.

## Business Goals
- Enable offline development workflows for developers without reliable internet connectivity
- Reduce dependency on AWS services for basic AI assistance functionality
- Provide cost-effective alternative for developers who want to avoid AWS usage charges
- Create extensible architecture for future multi-provider support

## User Stories
- As a developer working offline, I want to use local Ollama models so that I can continue using Amazon Q CLI without internet connectivity
- As a developer in a restricted network environment, I want to use local models so that I don't need to configure AWS access or deal with corporate firewall restrictions
- As a developer experimenting with different models, I want to easily switch between AWS and local Ollama models so that I can compare their performance for my use cases
- As a developer in a cost-conscious environment, I want to use free local models so that I can avoid AWS usage charges during development

## Acceptance Criteria
- [ ] Environment variable `Q_CLI_MODEL_PROVIDER` accepts values: `aws`, `ollama`, `openai`, `anthropic`
- [ ] Invalid `Q_CLI_MODEL_PROVIDER` values are validated at application launch with clear error messages
- [ ] When `Q_CLI_MODEL_PROVIDER=ollama`, authentication flow is bypassed (no `q login` required)
- [ ] When `Q_CLI_MODEL_PROVIDER=openai|anthropic`, `Q_CLI_MODEL_PROVIDER_API_KEY` environment variable is required
- [ ] When `Q_CLI_MODEL_PROVIDER=ollama`, no API key is required (local models)
- [ ] Ollama integration works with locally running Ollama server (default: http://localhost:11434)
- [ ] Chat conversations work seamlessly with Ollama models
- [ ] Tool usage (fs_read, execute_bash, etc.) works with Ollama models
- [ ] Conversation history is preserved when using Ollama models
- [ ] Error handling provides clear messages when Ollama server is unavailable
- [ ] Model selection works with available Ollama models
- [ ] Streaming responses work with Ollama models

## Technical Requirements

### Architecture Changes
- **Model Provider Abstraction**: Create a trait-based system for different model providers
- **Configuration System**: Environment variable validation and provider selection logic
- **Client Abstraction**: Abstract the API client to support multiple backends
- **Response Streaming**: Ensure streaming works consistently across providers
- **Error Handling**: Provider-specific error handling and user-friendly messages

### Key Integration Points (Research Findings)
Based on codebase analysis, the main integration points are:

1. **`ApiClient::send_message()`** in `crates/chat-cli/src/api_client/mod.rs` (lines 353+)
   - Currently handles AWS CodeWhisperer and Q Developer streaming clients
   - Returns `SendMessageOutput` enum with provider-specific variants

2. **`SendMessageOutput`** in `crates/chat-cli/src/api_client/send_message_output.rs`
   - Enum with `Codewhisperer`, `QDeveloper`, and `Mock` variants
   - Needs new `Ollama` variant for local model responses

3. **`SendMessageStream`** in `crates/chat-cli/src/cli/chat/parser.rs` (lines 174+)
   - Handles streaming response parsing and events
   - Needs to support Ollama's response format

4. **Client Construction** in `ApiClient::new()` (lines 100+)
   - Currently creates AWS clients based on `AMAZON_Q_SIGV4` environment variable
   - Needs provider selection logic based on `Q_CLI_MODEL_PROVIDER`

### Environment Variables
- `Q_CLI_MODEL_PROVIDER`: Primary provider selection (`aws` | `ollama` | `openai` | `anthropic`)
- `Q_CLI_MODEL_PROVIDER_API_KEY`: API key for external providers (not needed for Ollama)
- `Q_CLI_OLLAMA_BASE_URL`: Ollama server URL (default: `http://localhost:11434`)
- `Q_CLI_OLLAMA_MODEL`: Default Ollama model to use (e.g., `llama2`, `codellama`)

### Ollama Integration
- **HTTP Client**: Use existing `reqwest` client for Ollama API calls
- **API Compatibility**: Implement Ollama's chat completion API format
- **Model Discovery**: Query available models from Ollama server
- **Streaming**: Support Ollama's streaming response format
- **Error Handling**: Handle Ollama-specific errors (server down, model not found, etc.)

## Constraints
- Must maintain backward compatibility with existing AWS-based workflows
- Cannot break existing agent configurations or conversation storage
- Must work across all supported platforms (macOS, Linux, Windows)
- Should not significantly increase binary size or startup time
- Must handle graceful degradation when Ollama server is unavailable

## Out of Scope
- Ollama installation and setup (users must install Ollama separately)
- Model downloading and management (users handle via Ollama CLI)
- OpenAI and Anthropic provider implementations (future features)
- Advanced Ollama configuration (model parameters, custom endpoints beyond base URL)
- Migration tools for existing conversations between providers

## Timeline
- Start date: Current sprint
- Target completion: 2-3 sprints (depending on complexity of streaming integration)

## Dependencies
- **Ollama Server**: Users must have Ollama installed and running locally
- **Ollama Models**: Users must have downloaded desired models (llama2, codellama, etc.)
- **HTTP Client**: Leverage existing `reqwest` dependency for Ollama API calls
- **JSON Parsing**: Use existing `serde_json` for request/response serialization

## Risks and Considerations

### Technical Risks
- **Response Format Differences**: Ollama's response format may differ from AWS, requiring careful mapping
- **Streaming Implementation**: Ensuring consistent streaming behavior across providers
- **Tool Integration**: Verifying that built-in tools work correctly with Ollama responses
- **Performance**: Local models may be slower than AWS-hosted models
- **Memory Usage**: Large local models may consume significant system resources

### User Experience Risks
- **Setup Complexity**: Users need to install and configure Ollama separately
- **Model Management**: Users need to understand Ollama model downloading and management
- **Error Messages**: Need clear guidance when Ollama server is not running or models are missing
- **Feature Parity**: Some AWS-specific features may not be available with local models

### Future Extensibility
- **Provider Interface**: Design must accommodate future OpenAI and Anthropic integrations
- **Configuration Management**: Environment variable approach should scale to multiple providers
- **Authentication**: Framework should support different auth methods (API keys, OAuth, etc.)
- **Feature Flags**: Some features may be provider-specific and need conditional availability
