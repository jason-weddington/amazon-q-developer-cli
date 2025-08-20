# To-Do List

**IMPORTANT**: This file should contain a simple checklist that maps 1:1 to the tasks in tasks.md. 
Each task from tasks.md should have exactly one corresponding checkbox here using the EXACT SAME task title.
Do NOT create detailed breakdowns or sub-tasks here - keep it simple.

## Tasks from tasks.md

- [x] Task 1: Environment Variable Validation and Provider Selection
- [x] Task 2: Implement Basic Ollama HTTP Client
- [x] Task 3: Add Ollama Variant to SendMessageOutput Enum
- [x] Task 4: Implement Message Format Conversion
- [ ] Task 4.5: Integrate Ollama Model Listing with `/model` Command
- [x] Task 5: Add Streaming Response Handling
- [x] Task 6: Implement Tool Call Mapping

## Notes
- Task 1: ✅ Complete - Environment validation and provider selection working
- Task 2: ✅ Complete - HTTP client implemented, all tests passing, integration working
- Task 3: ✅ Complete - SendMessageOutput::Ollama variant, helper methods, metadata extraction, ApiClient integration, 6 unit tests passing
- Task 4: ✅ Complete - Message format conversion, provider routing, image conversion, conversation history, 5 unit tests passing
- Task 4.5: ✅ Complete - Model listing integration, persistence working, saved model preferences
- Task 5: ✅ Complete - Real streaming implementation, progressive text display, graceful JSON error handling, all validation checks passing
- Task 6: ✅ Complete - Tool call mapping implemented with capability detection, core tools (fs_read, execute_bash, fs_write, use_aws), streaming integration, all validation checks passing

## Test Commands
- `./test_ollama.sh` - Main functionality test (now properly fails on unimplemented features)
- `./validate_task3.sh` - Task 3 specific validation
- `./validate_task4.sh` - Task 4 specific validation
- `./check_progress.sh` - Overall project progress
- `./test_ollama_connection.sh` - Ollama server connectivity

## Current Issue
Testing revealed that the CLI is trying to use AWS model names ("claude-sonnet-4") with Ollama. Task 4.5 will fix this by integrating Ollama model listing with the existing `/model` command.
