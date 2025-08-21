# To-Do List

**IMPORTANT**: This file should contain a simple checklist that maps 1:1 to the tasks in tasks.md. 
Each task from tasks.md should have exactly one corresponding checkbox here using the EXACT SAME task title.
Do NOT create detailed breakdowns or sub-tasks here - keep it simple.

## Tasks from tasks.md

- [x] Task 1: Environment Variable Validation and Provider Selection
- [x] Task 2: Implement Basic Ollama HTTP Client
- [x] Task 3: Add Ollama Variant to SendMessageOutput Enum
- [x] Task 4: Implement Message Format Conversion
- [x] Task 4.5: Integrate Ollama Model Listing with `/model` Command
- [x] Task 7: Tool Result Handling in Conversation History
- [x] Task 8: Multi-turn Tool Conversations
- [x] Task 9: Error Handling for Unsupported Models
- [x] Task 10: Enhanced Thinking Tool Capability Detection
- [x] Task 11: MCP Tools Integration with Ollama
- [ ] Task 12: Configurable Reasoning Effort for Thinking Models
- [x] Task 13: Accurate Context Window Reporting for Ollama Models

## Notes
- Task 1: ✅ Complete - Environment validation and provider selection working
- Task 2: ✅ Complete - HTTP client implemented, all tests passing, integration working
- Task 3: ✅ Complete - SendMessageOutput::Ollama variant, helper methods, metadata extraction, ApiClient integration, 6 unit tests passing
- Task 4: ✅ Complete - Message format conversion, provider routing, image conversion, conversation history, 5 unit tests passing
- Task 4.5: ✅ Complete - Model listing integration, persistence working, saved model preferences
- **Missing Tasks 5 & 6**: These were completed during plugin refactor but are missing from tasks.md
  - Task 5: ✅ Complete - Streaming Response Handling (infrastructure in place)
  - Task 6: ✅ Complete - Tool Call Mapping (built-in tools working with Ollama)
- Task 7: ✅ Complete - Tool calls working
- Task 8: ✅ Complete - Multi-turn Tool Conversations
- Task 9: ✅ **COMPLETE** - Error handling for unsupported models implemented
  - Problem: Models like deepseek-r1:8b don't support tools, causing HTTP 400 errors
  - Solution: Check model capabilities before including tools in requests
  - Implementation: Added `check_model_supports_tools()` method with graceful error handling
  - Result: Non-tool models work for basic chat, tool-capable models continue working with tools
- Task 10: ✅ **COMPLETE** - Enhanced Thinking Tool Capability Detection
  - Problem: Thinking tool exists in core CLI but not exposed to Ollama models
  - Solution: Added thinking tool to Ollama with capability detection and settings integration
  - Implementation: Added `database: Database` field to `OllamaProvider`, minimal upstream change in `ApiClient::new()`
  - Result: Models with "thinking" capability get thinking tool when `chat.enableThinking` is enabled
  - Hardcoded "medium" reasoning effort (Task 12 will add user configuration)
- Task 11: ✅ **COMPLETE** - MCP Tools Integration with Ollama
  - Problem: MCP tools (like `convert_to_markdown` from fetch server) were not visible to Ollama models
  - Solution: Fixed through proper streaming implementation in earlier tasks
  - Result: MCP tools now work with Ollama models as demonstrated in testing

- Task 13: ✅ **COMPLETE** - Accurate Context Window Reporting for Ollama Models
  - Problem: gpt-oss models have 128K token context window but Q CLI reported 200K tokens
  - Root Cause: Three sources of hardcoded 200K limits in model creation code
  - Solution: Added dynamic querying via Ollama `/api/show` endpoint for real model metadata
  - Key Fix: Field name detection (`gptoss.context_length` not `gpt-oss.context_length`)
  - Implementation: Enhanced `OllamaClient` with `get_model_context_window()`, updated model creation to query real context windows
  - Result: `/usage` command now shows accurate context window sizes (128K for gpt-oss, etc.)
  - Fallback: Graceful error handling with 200K default when queries fail

## Test Commands
- `./test_ollama.sh` - Main functionality test (now properly fails on unimplemented features)
- `./validate_task3.sh` - Task 3 specific validation
- `./validate_task4.sh` - Task 4 specific validation
- `./check_progress.sh` - Overall project progress
- `./test_ollama_connection.sh` - Ollama server connectivity
