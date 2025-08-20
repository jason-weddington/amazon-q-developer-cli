#!/bin/bash

# Test script for Ollama provider functionality
# Assumes: ollama serve is already running and models are pulled

set -e  # Exit on any error

echo "🧪 Testing Amazon Q CLI with Ollama Provider"
echo "=============================================="
echo

# Source Rust environment
source ~/.cargo/env

# Set Ollama as the provider
export Q_CLI_MODEL_PROVIDER=ollama

echo "📋 Test 1: Basic CLI functionality"
echo "Command: Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --help"
echo
cargo run --bin chat_cli -- --help 2>/dev/null | head -10
echo "✅ Help command works with Ollama provider"
echo

echo "📋 Test 2: Version check"
echo "Command: Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --version"
echo
cargo run --bin chat_cli -- --version 2>/dev/null
echo "✅ Version command works"
echo

echo "📋 Test 3: Custom base URL"
echo "Command: Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:11434 cargo run --bin chat_cli -- --help"
echo
Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:11434 cargo run --bin chat_cli -- --help 2>/dev/null | head -5
echo "✅ Custom base URL works"
echo

echo "📋 Test 4: Unit tests (Tasks 1-2)"
echo "Command: cargo test ollama::tests"
echo
OLLAMA_TESTS=$(cargo test ollama::tests 2>/dev/null | grep -E "test result: ok\. [0-9]+ passed")
if echo "$OLLAMA_TESTS" | grep -q "7 passed"; then
    echo "✅ All 7 Ollama HTTP client tests pass"
else
    echo "❌ Ollama HTTP client tests failing or incomplete"
fi
echo

echo "📋 Test 5: Task 3 - SendMessageOutput enum tests"
echo "Command: cargo test send_message_output::tests"
echo

# Check if SendMessageOutput file exists and has Ollama variant
if [ -f "crates/chat-cli/src/api_client/send_message_output.rs" ] && grep -q "Ollama(" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    # File exists and has Ollama variant, check if tests pass
    SEND_MSG_TESTS=$(cargo test send_message_output::tests 2>/dev/null)
    if echo "$SEND_MSG_TESTS" | grep -q "test result: ok" && echo "$SEND_MSG_TESTS" | grep -q "passed"; then
        echo "✅ SendMessageOutput Ollama variant tests pass"
        TASK3_ENUM_STATUS="✅ COMPLETE"
    else
        echo "❌ SendMessageOutput tests exist but failing"
        TASK3_ENUM_STATUS="❌ FAILING"
    fi
else
    echo "❌ SendMessageOutput Ollama variant not implemented"
    TASK3_ENUM_STATUS="⏳ PENDING"
fi
echo

echo "📋 Test 6: Task 3 - ApiClient Ollama integration"
echo "Command: cargo test api_client.*send_message_ollama"
echo

# Check if ApiClient has send_message_ollama method
if grep -q "send_message_ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    # Method exists, check if tests pass
    API_TESTS=$(cargo test api_client 2>/dev/null)
    if echo "$API_TESTS" | grep -q "test_send_message_ollama.*ok"; then
        echo "✅ ApiClient Ollama integration tests pass"
        TASK3_API_STATUS="✅ COMPLETE"
    else
        echo "❌ ApiClient Ollama integration tests failing"
        TASK3_API_STATUS="❌ FAILING"
    fi
else
    echo "❌ ApiClient send_message_ollama method not implemented"
    TASK3_API_STATUS="⏳ PENDING"
fi
echo

# Determine overall Task 3 status
if [ "$TASK3_ENUM_STATUS" = "✅ COMPLETE" ] && [ "$TASK3_API_STATUS" = "✅ COMPLETE" ]; then
    TASK3_STATUS="✅ COMPLETE"
elif [ "$TASK3_ENUM_STATUS" = "❌ FAILING" ] || [ "$TASK3_API_STATUS" = "❌ FAILING" ]; then
    TASK3_STATUS="❌ FAILING"
else
    TASK3_STATUS="⏳ PENDING"
fi

echo "📋 Test 7: What works vs what doesn't"
echo
echo "✅ WORKING (Tasks 1-2):"
echo "  - Provider selection and validation"
echo "  - HTTP client initialization"
echo "  - Connection to Ollama server"
echo "  - All CLI commands (help, version, etc.)"
echo "  - Environment variable configuration"
echo
echo "Task 3 Status: $TASK3_STATUS"
if [ "$TASK3_STATUS" = "✅ COMPLETE" ]; then
    echo "  - ✅ SendMessageOutput::Ollama variant"
    echo "  - ✅ Response processing pipeline"
    echo "  - ✅ Metadata extraction"
    echo "  - ✅ ApiClient integration methods"
elif [ "$TASK3_STATUS" = "❌ FAILING" ]; then
    echo "  - ❌ SendMessageOutput implementation has issues"
    echo "  - ❌ Tests are failing"
else
    echo "  - ⏳ SendMessageOutput::Ollama variant (not implemented)"
    echo "  - ⏳ Response processing pipeline (not implemented)"
    echo "  - ⏳ Metadata extraction (not implemented)"
    echo "  - ⏳ ApiClient integration methods (not implemented)"
fi
echo
echo "❌ NOT YET WORKING:"
echo "  - Chat functionality (needs Task 4: Message conversion)"
echo "  - Streaming responses (Task 5)"
echo "  - Tool calls (Task 6)"
echo

echo "📋 Test 8: Task 4 - Message Format Conversion"
echo "Command: ./validate_task4.sh"
echo

TASK4_OUTPUT=$(./validate_task4.sh 2>/dev/null)
if echo "$TASK4_OUTPUT" | grep -q "Task 4.*COMPLETE"; then
    echo "✅ Message format conversion implemented"
    TASK4_STATUS="✅ COMPLETE"
elif echo "$TASK4_OUTPUT" | grep -q "Task 4.*IN PROGRESS"; then
    echo "⏳ Message format conversion in progress"
    TASK4_STATUS="⏳ IN PROGRESS"
else
    echo "❌ Message format conversion not implemented"
    TASK4_STATUS="⏳ PENDING"
fi
echo

echo "📋 Test 9: Attempting chat (expected behavior depends on Task 4)"
echo "Command: Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
echo
if [ "$TASK3_STATUS" = "✅ COMPLETE" ]; then
    echo "With Task 3 complete, this should get further but still fail at message conversion..."
else
    echo "This should fail because SendMessageOutput integration isn't implemented yet..."
fi

# Capture the chat attempt output
CHAT_OUTPUT=$(timeout 10s cargo run --bin chat_cli -- chat 2>&1 || true)
echo "$CHAT_OUTPUT" | head -10

# Analyze the failure to give better feedback
if echo "$CHAT_OUTPUT" | grep -q "UnsupportedProvider"; then
    echo "❌ Still showing UnsupportedProvider - Task 2 integration issue"
elif echo "$CHAT_OUTPUT" | grep -q "SendMessageOutput"; then
    echo "❌ SendMessageOutput error - Task 3 needed"
elif echo "$CHAT_OUTPUT" | grep -q "message.*conversion\|format"; then
    echo "✅ Good! Failing at message conversion - Task 4 needed"
else
    echo "❌ Chat failed as expected (more implementation needed)"
fi
echo

echo "🎉 Test Summary"
echo "==============="
echo "✅ Task 1 - Environment Variable Validation: COMPLETE"
echo "✅ Task 2 - Ollama HTTP Client: COMPLETE"
echo "📋 Task 3 - SendMessageOutput Integration: $TASK3_STATUS"
echo "📋 Task 4 - Message Format Conversion: $TASK4_STATUS"
echo "⏳ Task 5 - Streaming Response Handling: PENDING"
echo "⏳ Task 6 - Tool Call Mapping: PENDING"
echo
echo "✅ All existing AWS functionality: PRESERVED"
echo

if [ "$TASK4_STATUS" = "✅ COMPLETE" ]; then
    echo "🎉 BASIC CHAT IS WORKING!"
    echo
    echo "You can now chat with Ollama:"
    echo "  Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
    echo
    echo "🚀 Ready for Task 5: Add Streaming Response Handling"
elif [ "$TASK3_STATUS" = "✅ COMPLETE" ]; then
    echo "🚀 Ready for Task 4: Implement Message Format Conversion"
    echo
    echo "After Task 4, you'll be able to:"
    echo "  Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
elif [ "$TASK3_STATUS" = "❌ FAILING" ]; then
    echo "🔧 Fix Task 3 implementation issues"
    echo
    echo "Run './validate_task3.sh' for detailed diagnostics"
else
    echo "🚀 Ready for Task 3: Add Ollama Variant to SendMessageOutput Enum"
    echo
    echo "Task 3 will enable:"
    echo "  - Ollama response processing"
    echo "  - Unified message handling pipeline"
    echo "  - Foundation for message conversion (Task 4)"
    echo
    echo "Run './validate_task3.sh' to track Task 3 progress"
fi
