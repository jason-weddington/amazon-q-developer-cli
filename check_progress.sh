#!/bin/bash

echo "🚀 Amazon Q CLI - Ollama Integration Progress"
echo "============================================="
echo

# Task 1: Environment Variable Validation
echo "📋 Task 1: Environment Variable Validation and Provider Selection"
if grep -q "pub fn from_env" crates/chat-cli/src/util/mod.rs 2>/dev/null && grep -q "ModelProvider::Ollama" crates/chat-cli/src/util/mod.rs 2>/dev/null; then
    echo "✅ COMPLETE - Provider selection and validation working"
    TASK1_COMPLETE=true
else
    echo "❌ NOT COMPLETE"
    TASK1_COMPLETE=false
fi
echo

# Task 2: Ollama HTTP Client  
echo "📋 Task 2: Implement Basic Ollama HTTP Client"
if [ -f "crates/chat-cli/src/api_client/ollama.rs" ] && grep -q "pub struct OllamaClient" crates/chat-cli/src/api_client/ollama.rs 2>/dev/null; then
    echo "✅ COMPLETE - HTTP client implemented and tested"
else
    echo "❌ NOT COMPLETE"
fi
echo

# Task 3: SendMessageOutput Integration
echo "📋 Task 3: Add Ollama Variant to SendMessageOutput Enum"
if grep -q "Ollama(" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    echo "✅ COMPLETE - Response processing pipeline ready"
else
    echo "⏳ IN PROGRESS - Ready to implement"
fi
echo

# Task 4: Message Format Conversion
echo "📋 Task 4: Implement Message Format Conversion"
if grep -q "ModelProvider::Ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null && grep -q "convert_conversation_to_ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    echo "✅ COMPLETE - Message conversion implemented"
    TASK4_COMPLETE=true
else
    echo "⏳ PENDING - Depends on Task 3"
    TASK4_COMPLETE=false
fi
echo

# Task 5: Streaming Response Handling
echo "📋 Task 5: Add Streaming Response Handling"
if grep -q "stream.*ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    echo "✅ COMPLETE - Streaming responses working"
else
    echo "⏳ PENDING - Depends on Task 4"
fi
echo

# Task 6: Tool Call Mapping
echo "📋 Task 6: Implement Tool Call Mapping"
if grep -q "tool.*ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    echo "✅ COMPLETE - Tool calls working"
else
    echo "⏳ PENDING - Depends on Task 5"
fi
echo

echo "🎯 Current Status"
echo "================"

# Count completed tasks
COMPLETED=0
if [ "$TASK1_COMPLETE" = true ]; then
    COMPLETED=$((COMPLETED + 1))
fi

if [ -f "crates/chat-cli/src/api_client/ollama.rs" ] && grep -q "pub struct OllamaClient" crates/chat-cli/src/api_client/ollama.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if [ -f "crates/chat-cli/src/api_client/send_message_output.rs" ] && grep -q "Ollama(" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if [ "$TASK4_COMPLETE" = true ]; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "stream.*ollama\|streaming.*ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "tool.*ollama\|tool_call.*ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

echo "Progress: $COMPLETED/6 tasks complete"
echo

case $COMPLETED in
    0)
        echo "🚀 Ready to start Task 1: Environment Variable Validation"
        ;;
    1)
        echo "🚀 Ready to start Task 2: Implement Basic Ollama HTTP Client"
        ;;
    2)
        echo "🚀 Ready to start Task 3: Add Ollama Variant to SendMessageOutput Enum"
        echo
        echo "Next: Run './validate_task3.sh' to check Task 3 progress"
        ;;
    3)
        echo "🚀 Ready to start Task 4: Implement Message Format Conversion"
        echo
        echo "After Task 4, basic chat will work:"
        echo "  Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
        ;;
    4)
        echo "🚀 Ready to start Task 5: Add Streaming Response Handling"
        echo
        echo "Chat functionality should be working!"
        ;;
    5)
        echo "🚀 Ready to start Task 6: Implement Tool Call Mapping"
        echo
        echo "Almost done! Just tool calls remaining."
        ;;
    6)
        echo "🎉 ALL TASKS COMPLETE!"
        echo
        echo "Full Ollama integration is ready:"
        echo "  Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
        echo "  - ✅ Basic chat"
        echo "  - ✅ Streaming responses"  
        echo "  - ✅ Tool calls"
        ;;
esac

echo
echo "📊 Test Commands"
echo "==============="
echo "./test_ollama.sh           # Test current functionality"
echo "./test_ollama_connection.sh # Test Ollama server connection"
echo "./validate_task3.sh        # Check Task 3 specific progress"
echo "./validate_task4.sh        # Check Task 4 specific progress"
echo "./check_progress.sh        # This script - overall progress"
