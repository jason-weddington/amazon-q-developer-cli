## Build and Test Commands
```bash
# Rust Development Commands
source ~/.cargo/env                    # Source Rust environment
cargo check                          # Fast compilation check
cargo test                           # Run all tests
cargo test module::tests             # Run specific module tests
cargo fix --bin "chat_cli" --tests --allow-dirty  # Auto-fix code issues
cargo clippy                         # Linting
cargo +nightly fmt                   # Code formatting

# Project-specific commands
./test_ollama.sh                     # Test Ollama integration
./test_ollama_connection.sh          # Test Ollama server connectivity
./validate_task3.sh                  # Validate specific task implementation
./check_progress.sh                  # Overall project progress
```

## Research and Documentation

### Local Repository Research
**IMPORTANT**: Always check `~/git/` for locally cloned repositories before researching external APIs or documentation:
- **Check first**: `ls ~/git/` to see what repositories are available
- **Common repos**: `ollama`, `ollama-python`, and other relevant projects may already be cloned
- **Official docs**: Look in `docs/` directories for authoritative API documentation
- **Source code**: Check actual implementation in source files for accurate behavior
- **Clone for research**: Use `git clone <repo>` to `~/git/` for any new research needs

**Example**: For Ollama integration, check `~/git/ollama/docs/api.md` and `~/git/ollama/types/model/capability.go` for official API specs and capability definitions.

## Development Best Practices

### Code Quality
- **Use `cargo fix`**: Automatically removes unused imports and fixes simple issues
  ```bash
  cargo fix --bin "chat_cli" --tests --allow-dirty
  ```
- **Review changes**: Always check `git diff` after running `cargo fix`
- **Expected warnings**: "Dead code" warnings are normal during development - they indicate code that's implemented but not yet connected

### Testing Strategy
- **Test-driven development**: Write tests first, then implement
- **Incremental testing**: Test each component as you build it
- **Mock external dependencies**: Use `mockito` for HTTP client testing
- **Integration tests**: Test the full pipeline with real scenarios

### Environment Variables
- **Generic naming**: Use `Q_CLI_MODEL_PROVIDER_BASE_URL` instead of provider-specific names
- **Sensible defaults**: Always provide reasonable defaults for optional environment variables
- **Validation**: Validate environment variables early in the application startup

### Error Handling
- **Comprehensive error types**: Create specific error variants for different failure modes
- **Error conversion**: Use `#[from]` attribute for automatic error conversion
- **Graceful degradation**: Log warnings for non-critical failures (like health checks)

### API Design
- **Provider abstraction**: Design APIs to work with multiple providers
- **Unified interfaces**: Use common traits/enums across different implementations
- **Future-proofing**: Consider how the design will extend to new providers

### Module Organization
```
crates/chat-cli/src/api_client/
├── mod.rs                    # Main API client with provider switching
├── ollama.rs                 # Provider-specific implementation
├── send_message_output.rs    # Unified response handling
└── error.rs                  # Comprehensive error types
```

### Testing Files Structure
```
project-root/
├── test_ollama.sh           # Main functionality testing
├── test_ollama_connection.sh # External dependency testing
├── validate_task3.sh        # Task-specific validation
├── check_progress.sh        # Overall progress tracking
└── planning/feature/*/      # Task planning and documentation
```

## Project Structure

```
your-project/                  # Your project repository
├── AmazonQ.md               # Project-specific build and test commands (created by init)
├── docs/                      # General project documentation (created by init)
│   ├── api-docs.md            # API documentation
│   ├── architecture.md        # System architecture
│   ├── codebase.md            # Code style and patterns
│   ├── domain.md              # Domain concepts
│   ├── setup.md               # Environment setup instructions
│   └── testing.md             # Testing strategy
├── planning/                  # Planning directory (created by init)
│   ├── templates/             # Feature-specific templates (created by init)
│   │   ├── feature.md         # Feature description template
│   │   ├── tasks.md           # Detailed development tasks template
│   │   └── to-do.md           # Task checklist template
│   └── [branch-name]/         # Mirrors your git branch structure (created by new)
│       ├── feature.md         # Feature description
│       ├── tasks.md           # Detailed development tasks
│       └── to-do.md           # Task checklist
│
│   Examples:
│   └── feature/new-auth/      # For git branch "feature/new-auth"
│   └── fix/bug-123/           # For git branch "fix/bug-123"
│   └── refactor/db-layer/     # For git branch "refactor/db-layer"
```

Feature-specific notes are stored in the `planning` folder in a subfolder that exactly matches your git branch name. The directory structure mirrors your branch names - if your branch is called `feature/new-feature`, the docs go in `planning/feature/new-feature/`. The word "feature" is not a special directory, it's just part of common branch naming conventions.

## Creating a New Project
To bootstrap a new project with the standard planning structure:

1. Create and checkout a new feature branch (e.g., `git checkout -b feature/new-feature`)
2. Run the project bootstrap command:
   ```
   claude-workflow new
   ```
3. The command will:
   - Create the proper directory structure based on your current branch
   - Copy template files for feature-specific documents (feature.md, tasks.md, to-do.md)
4. **IMPORTANT:** Open and read the template files in the docs/ directory, particularly `codebase.md` and `domain.md`. These contain first-time setup instructions for AI assistants to analyze your codebase and document it properly.

## Development Workflow
- Check to-do.md for the next task to implement (simple checklist with 1:1 mapping to tasks.md)
- Read the detailed requirements in tasks.md for that specific task
- Implement only that single task completely, following TDD practices
- Ensure all tests pass before considering the task complete
- Update docs/codebase.md with any new structures, patterns, or concepts introduced
- Mark the task as completed in to-do.md (check the box)
- Run `cargo fix --bin "chat_cli" --tests --allow-dirty` to clean up code
- Commit changes to git with a meaningful commit message
- Stop and wait for feedback before moving to the next task

**File Relationship**: tasks.md contains detailed task descriptions with acceptance criteria and implementation notes. to-do.md contains a simple checklist that maps 1:1 to those tasks - one checkbox per task, no detailed breakdowns.

## Rust-Specific Guidelines

### Compilation and Testing
- **Fast feedback loop**: Use `cargo check` for quick compilation validation
- **Incremental testing**: Test individual modules with `cargo test module::tests`
- **Clean builds**: Occasionally run `cargo clean` if you encounter weird compilation issues

### Code Organization
- **Module privacy**: Use `pub(crate)` for internal APIs, `pub` only for external interfaces
- **Error handling**: Prefer `Result<T, E>` over panics for recoverable errors
- **Async patterns**: Use `tokio::test` for async test functions

### Dependencies
- **Workspace dependencies**: Use `dependency.workspace = true` in Cargo.toml
- **Feature flags**: Use conditional compilation `#[cfg(test)]` for test-only code
- **Mock dependencies**: Use `mockito` for HTTP mocking, avoid real network calls in tests

### Performance Considerations
- **Clone vs Reference**: Prefer references where possible, clone when necessary for ownership
- **Async efficiency**: Don't block async runtime with synchronous operations
- **Memory usage**: Be mindful of large data structures in enum variants

## Code Style Guidelines
- Follow existing patterns in the codebase
- Use descriptive variable names
- Add comprehensive error handling
- Include unit tests for all new functionality
- Document public APIs with rustdoc comments
- Use `cargo clippy` to catch common issues
- Run `cargo fix` regularly to maintain clean code

## Debugging Tips
- **Compilation errors**: Read error messages carefully - Rust errors are usually very helpful
- **Test failures**: Use `cargo test -- --nocapture` to see println! output
- **Integration issues**: Check environment variables and external dependencies
- **Performance**: Use `cargo test --release` for performance-sensitive tests

## Lessons Learned

### Cargo Fix Best Practices
- **Run regularly**: `cargo fix --bin "chat_cli" --tests --allow-dirty` cleans up unused imports automatically
- **Review changes**: Always run `git diff` after `cargo fix` to see what changed
- **Safe transformations**: `cargo fix` only makes changes guaranteed to be safe
- **What it fixes**: Unused imports, deprecated syntax, simple style issues, some clippy suggestions
- **What it doesn't fix**: Logic errors, dead code warnings (intentionally), complex refactoring

### Test Script Design
- **Fail fast**: Test scripts should fail when features aren't implemented (avoid false positives)
- **Specific validation**: Check for actual implementation, not just file existence
- **Progressive feedback**: Show what works, what doesn't, and what's next
- **Pattern matching**: Use specific patterns to detect test success (e.g., `grep -q "test_name.*ok"`)

### Enum Design Patterns
- **Provider abstraction**: Use enums to handle multiple providers uniformly
- **Helper methods**: Add methods to extract common data regardless of variant
- **Metadata extraction**: Separate enum for provider-specific metadata
- **Conversion traits**: Implement `From` trait for easy conversion between types

### HTTP Client Integration
- **Mock testing**: Use `mockito::Server::new_async()` for reliable HTTP testing
- **Error handling**: Create comprehensive error enums with `#[from]` conversions
- **Health checks**: Non-blocking health checks with warning logs, not failures
- **Environment configuration**: Generic environment variables for multi-provider support

### API Client Architecture
- **Unified interface**: Same methods work regardless of provider
- **Optional clients**: Use `Option<Client>` for provider-specific clients
- **Graceful degradation**: Methods return appropriate errors when provider not configured
- **Future-proofing**: Design for easy addition of new providers

### Testing Strategy Insights
- **Unit test coverage**: Test each component in isolation with mocks
- **Integration testing**: Test actual provider integration separately
- **Test organization**: Group tests by functionality, not by file structure
- **Async testing**: Use `tokio::test` for async functions, handle futures properly

### Development Workflow Improvements
- **Task validation scripts**: Create specific validation for each major task
- **Progress tracking**: Automated scripts to show overall project progress
- **Incremental development**: Complete one task fully before moving to next
- **Test-driven approach**: Write tests first, then implement to make them pass

### Common Pitfalls Avoided
- **False positive tests**: Ensure tests actually validate implementation
- **Unused import accumulation**: Regular `cargo fix` prevents import bloat
- **Provider coupling**: Keep provider-specific code isolated in modules
- **Error propagation**: Use `?` operator and proper error conversion
- **Clone vs reference**: Be mindful of ownership in enum variants

### Multi-Provider Architecture Patterns
- **Generic environment variables**: Use `Q_CLI_MODEL_PROVIDER_BASE_URL` instead of provider-specific names
- **Unified response types**: Single enum with provider-specific variants (e.g., `SendMessageOutput::Ollama`)
- **Provider detection**: Early validation of `Q_CLI_MODEL_PROVIDER` with clear error messages
- **Graceful fallbacks**: Health checks that warn but don't fail the application
- **Consistent error handling**: Map provider-specific errors to common error types
- **Future-proof design**: Structure code to easily add new providers without major refactoring

### Task-Based Development Insights
- **Incremental validation**: Create validation scripts for each task to track progress
- **Test-first approach**: Write failing tests, then implement to make them pass
- **Single responsibility**: Each task should have one clear, testable outcome
- **Documentation as code**: Keep task definitions and progress tracking in version control
- **Automated progress tracking**: Scripts that show overall project status at a glance

### Testing Patterns That Work
- **Layered testing approach**:
  - Unit tests with mocks for individual components
  - Integration tests for provider-specific functionality
  - End-to-end tests for complete workflows
- **Test script hierarchy**:
  - `./test_ollama.sh` - Main functionality test
  - `./validate_task3.sh` - Task-specific validation
  - `./check_progress.sh` - Overall project progress
  - `./test_ollama_connection.sh` - External dependency testing
- **Mockito patterns**:
  ```rust
  let mut server = mockito::Server::new_async().await;
  let mock = server.mock("POST", "/api/chat")
      .with_status(200)
      .with_body(r#"{"response": "data"}"#)
      .create_async()
      .await;
  
  // Test code here
  mock.assert_async().await;
  ```
- **Async test patterns**:
  ```rust
  #[tokio::test]
  async fn test_async_function() {
      let result = async_function().await;
      assert!(result.is_ok());
  }
  ```

### Code Organization Patterns
- **Provider isolation**: Each provider in its own module
- **Unified interfaces**: Common enums and traits across providers
- **Error mapping**: Provider-specific errors map to common error types
- **Optional clients**: Use `Option<ProviderClient>` for conditional functionality
- **Helper methods**: Extract common functionality regardless of provider variant

## Project Structure

```
your-project/                  # Your project repository
├── AmazonQ.md               # Project-specific build and test commands (created by init)
├── docs/                      # General project documentation (created by init)
│   ├── api-docs.md            # API documentation
│   ├── architecture.md        # System architecture
│   ├── codebase.md            # Code style and patterns
│   ├── domain.md              # Domain concepts
│   ├── setup.md               # Environment setup instructions
│   └── testing.md             # Testing strategy
├── planning/                  # Planning directory (created by init)
│   ├── templates/             # Feature-specific templates (created by init)
│   │   ├── feature.md         # Feature description template
│   │   ├── tasks.md           # Detailed development tasks template
│   │   └── to-do.md           # Task checklist template
│   └── [branch-name]/         # Mirrors your git branch structure (created by new)
│       ├── feature.md         # Feature description
│       ├── tasks.md           # Detailed development tasks
│       └── to-do.md           # Task checklist
│
│   Examples:
│   └── feature/new-auth/      # For git branch "feature/new-auth"
│   └── fix/bug-123/           # For git branch "fix/bug-123"
│   └── refactor/db-layer/     # For git branch "refactor/db-layer"
```

Feature-specific notes are stored in the `planning` folder in a subfolder that exactly matches your git branch name. The directory structure mirrors your branch names - if your branch is called `feature/new-feature`, the docs go in `planning/feature/new-feature/`. The word "feature" is not a special directory, it's just part of common branch naming conventions.

## Creating a New Project
To bootstrap a new project with the standard planning structure:

1. Create and checkout a new feature branch (e.g., `git checkout -b feature/new-feature`)
2. Run the project bootstrap command:
   ```
   claude-workflow new
   ```
3. The command will:
   - Create the proper directory structure based on your current branch
   - Copy template files for feature-specific documents (feature.md, tasks.md, to-do.md)
4. **IMPORTANT:** Open and read the template files in the docs/ directory, particularly `codebase.md` and `domain.md`. These contain first-time setup instructions for AI assistants to analyze your codebase and document it properly.

## Development Workflow
- Check to-do.md for the next task to implement (simple checklist with 1:1 mapping to tasks.md)
- Read the detailed requirements in tasks.md for that specific task
- Implement only that single task completely, following TDD practices
- Ensure all tests pass before considering the task complete
- Update docs/codebase.md with any new structures, patterns, or concepts introduced
- Mark the task as completed in to-do.md (check the box)
- Commit changes to git with a meaningful commit message
- Stop and wait for feedback before moving to the next task

**File Relationship**: tasks.md contains detailed task descriptions with acceptance criteria and implementation notes. to-do.md contains a simple checklist that maps 1:1 to those tasks - one checkbox per task, no detailed breakdowns.

## Code Style Guidelines
- Add your project's code style guidelines here

## Project Structure
- Add your project's structure information here
