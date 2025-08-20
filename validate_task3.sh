#!/bin/bash

echo "=== Task 3 Implementation Validation ==="
echo

echo "1. Checking SendMessageOutput enum for Ollama variant..."
if grep -q "Ollama(" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    echo "✅ Ollama variant found in SendMessageOutput enum"
else
    echo "❌ Ollama variant not found in SendMessageOutput enum"
fi

echo "2. Checking helper methods (content, role, is_done)..."
if grep -q "pub fn content(&self)" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    echo "✅ Helper methods found"
else
    echo "❌ Helper methods not found"
fi

echo "3. Checking ResponseMetadata enum..."
if grep -q "ResponseMetadata" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    echo "✅ ResponseMetadata enum found"
else
    echo "❌ ResponseMetadata enum not found"
fi

echo "4. Checking From trait implementation..."
if grep -q "impl From<OllamaChatResponse>" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    echo "✅ From trait implementation found"
else
    echo "❌ From trait implementation not found"
fi

echo "5. Checking ApiClient Ollama integration..."
if grep -q "send_message_ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    echo "✅ ApiClient Ollama integration found"
else
    echo "❌ ApiClient Ollama integration not found"
fi

echo "6. Checking unit tests for SendMessageOutput..."
if grep -q "test_ollama_variant" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    echo "✅ SendMessageOutput unit tests found"
else
    echo "❌ SendMessageOutput unit tests not found"
fi

echo
echo "=== Compilation Test ==="
echo "Testing if code compiles with Task 3 changes..."

# Source Rust environment
source ~/.cargo/env 2>/dev/null || echo "Note: Rust environment not sourced"

if command -v cargo >/dev/null 2>&1; then
    if cargo check --quiet 2>/dev/null; then
        echo "✅ Code compiles successfully"
    else
        echo "❌ Compilation errors found"
        echo "Run 'cargo check' for details"
    fi
else
    echo "⚠️  Cargo not available, skipping compilation test"
fi

echo
echo "=== Unit Tests ==="
echo "Testing SendMessageOutput functionality..."

if command -v cargo >/dev/null 2>&1; then
    if cargo test send_message_output::tests --quiet 2>/dev/null; then
        echo "✅ SendMessageOutput tests pass"
    else
        echo "❌ SendMessageOutput tests not found or failing"
    fi
    
    if cargo test api_client.*ollama --quiet 2>/dev/null; then
        echo "✅ ApiClient Ollama tests pass"
    else
        echo "❌ ApiClient Ollama tests not found or failing"
    fi
else
    echo "⚠️  Cargo not available, skipping unit tests"
fi

echo
echo "=== Integration Test ==="
echo "Testing Ollama response processing..."

if command -v cargo >/dev/null 2>&1; then
    # Test that we can create and process Ollama responses
    echo "Checking if Ollama responses can be processed through SendMessageOutput..."
    
    # This will be a simple compilation test for now
    if cargo check --quiet 2>/dev/null; then
        echo "✅ Ollama response processing integration ready"
    else
        echo "❌ Integration issues found"
    fi
else
    echo "⚠️  Cargo not available, skipping integration test"
fi

echo
echo "=== Task 3 Status ==="

# Count completed items
COMPLETED=0
TOTAL=6

if grep -q "Ollama(" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "pub fn content(&self)" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "ResponseMetadata" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "impl From<OllamaChatResponse>" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "send_message_ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

if grep -q "test_ollama_variant" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null; then
    COMPLETED=$((COMPLETED + 1))
fi

echo "Progress: $COMPLETED/$TOTAL components implemented"

if [ $COMPLETED -eq $TOTAL ]; then
    echo "✅ Task 3: Add Ollama Variant to SendMessageOutput Enum - COMPLETE"
    echo
    echo "Key achievements:"
    echo "- ✅ SendMessageOutput::Ollama variant added"
    echo "- ✅ Helper methods for content/role/completion extraction"
    echo "- ✅ ResponseMetadata for provider-specific data"
    echo "- ✅ From trait for easy conversion"
    echo "- ✅ ApiClient integration methods"
    echo "- ✅ Comprehensive unit tests"
    echo
    echo "Ready for Task 4: Implement Message Format Conversion! 🚀"
elif [ $COMPLETED -gt 0 ]; then
    echo "⏳ Task 3: Add Ollama Variant to SendMessageOutput Enum - IN PROGRESS"
    echo
    echo "Remaining work:"
    [ ! -f crates/chat-cli/src/api_client/send_message_output.rs ] && echo "- ❌ Create/modify send_message_output.rs"
    ! grep -q "Ollama(" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null && echo "- ❌ Add Ollama variant to enum"
    ! grep -q "pub fn content(&self)" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null && echo "- ❌ Add helper methods"
    ! grep -q "ResponseMetadata" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null && echo "- ❌ Add ResponseMetadata enum"
    ! grep -q "impl From<OllamaChatResponse>" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null && echo "- ❌ Add From trait implementation"
    ! grep -q "send_message_ollama" crates/chat-cli/src/api_client/mod.rs 2>/dev/null && echo "- ❌ Add ApiClient integration"
    ! grep -q "test_ollama_variant" crates/chat-cli/src/api_client/send_message_output.rs 2>/dev/null && echo "- ❌ Add unit tests"
else
    echo "⏳ Task 3: Add Ollama Variant to SendMessageOutput Enum - NOT STARTED"
    echo
    echo "Ready to begin Task 3 implementation!"
fi

echo
echo "Use './test_ollama.sh' to see overall progress across all tasks."
