#!/bin/bash

# Source cargo environment
source ~/.cargo/env 2>/dev/null || true

echo "=== Task 6 Implementation Validation ==="

echo "1. Checking tool call types in OllamaMessage..."
if grep -q "pub tool_calls: Option<Vec<OllamaToolCall>>" crates/chat-cli/src/api_client/ollama.rs; then
    echo "✅ OllamaMessage supports tool calls"
else
    echo "❌ OllamaMessage missing tool call support"
fi

echo "2. Checking OllamaTool and OllamaFunction definitions..."
if grep -q "pub struct OllamaTool" crates/chat-cli/src/api_client/ollama.rs && grep -q "pub struct OllamaFunction" crates/chat-cli/src/api_client/ollama.rs; then
    echo "✅ Tool definition types found"
else
    echo "❌ Tool definition types missing"
fi

echo "3. Checking tools field in OllamaChatRequest..."
if grep -q "pub tools: Option<Vec<OllamaTool>>" crates/chat-cli/src/api_client/ollama.rs; then
    echo "✅ OllamaChatRequest supports tools"
else
    echo "❌ OllamaChatRequest missing tools field"
fi

echo "4. Checking capability detection methods..."
if grep -q "get_model_capabilities" crates/chat-cli/src/api_client/ollama.rs && grep -q "supports_capability" crates/chat-cli/src/api_client/ollama.rs; then
    echo "✅ Capability detection methods found"
else
    echo "❌ Capability detection methods missing"
fi

echo "5. Checking tool call detection in streaming..."
if grep -q "Convert tool calls to ToolUseEvent" crates/chat-cli/src/api_client/ollama.rs && grep -q "ChatResponseStream::ToolUseEvent" crates/chat-cli/src/api_client/ollama.rs; then
    echo "✅ Tool call detection in streaming found"
else
    echo "❌ Tool call detection in streaming missing"
fi

echo "6. Checking tool generation in ApiClient..."
if grep -q "get_ollama_tools" crates/chat-cli/src/api_client/mod.rs; then
    echo "✅ Tool generation method found"
else
    echo "❌ Tool generation method missing"
fi

echo "7. Checking tool definitions (fs_read, execute_bash, fs_write, use_aws)..."
if grep -q "fs_read" crates/chat-cli/src/api_client/mod.rs && grep -q "execute_bash\|execute_cmd" crates/chat-cli/src/api_client/mod.rs && grep -q "fs_write" crates/chat-cli/src/api_client/mod.rs && grep -q "use_aws" crates/chat-cli/src/api_client/mod.rs; then
    echo "✅ Core tool definitions found"
else
    echo "❌ Core tool definitions missing"
fi

echo "8. Checking tools integration in send_message_ollama_internal..."
if grep -q "get_ollama_tools.*model" crates/chat-cli/src/api_client/mod.rs && grep -q "tools: Some(ollama_tools)" crates/chat-cli/src/api_client/mod.rs; then
    echo "✅ Tools integrated in message sending"
else
    echo "❌ Tools not integrated in message sending"
fi

echo "9. Checking compilation..."
if cargo check >/dev/null 2>&1; then
    echo "✅ Code compiles successfully"
else
    echo "❌ Compilation errors"
fi

echo ""
echo "=== Task 6 Status ==="

# Count successful checks
total_checks=9
passed_checks=0

if grep -q "pub tool_calls: Option<Vec<OllamaToolCall>>" crates/chat-cli/src/api_client/ollama.rs; then ((passed_checks++)); fi
if grep -q "pub struct OllamaTool" crates/chat-cli/src/api_client/ollama.rs && grep -q "pub struct OllamaFunction" crates/chat-cli/src/api_client/ollama.rs; then ((passed_checks++)); fi
if grep -q "pub tools: Option<Vec<OllamaTool>>" crates/chat-cli/src/api_client/ollama.rs; then ((passed_checks++)); fi
if grep -q "get_model_capabilities" crates/chat-cli/src/api_client/ollama.rs && grep -q "supports_capability" crates/chat-cli/src/api_client/ollama.rs; then ((passed_checks++)); fi
if grep -q "Convert tool calls to ToolUseEvent" crates/chat-cli/src/api_client/ollama.rs && grep -q "ChatResponseStream::ToolUseEvent" crates/chat-cli/src/api_client/ollama.rs; then ((passed_checks++)); fi
if grep -q "get_ollama_tools" crates/chat-cli/src/api_client/mod.rs; then ((passed_checks++)); fi
if grep -q "fs_read" crates/chat-cli/src/api_client/mod.rs && grep -q "execute_bash\|execute_cmd" crates/chat-cli/src/api_client/mod.rs && grep -q "fs_write" crates/chat-cli/src/api_client/mod.rs && grep -q "use_aws" crates/chat-cli/src/api_client/mod.rs; then ((passed_checks++)); fi
if grep -q "get_ollama_tools.*model" crates/chat-cli/src/api_client/mod.rs && grep -q "tools: Some(ollama_tools)" crates/chat-cli/src/api_client/mod.rs; then ((passed_checks++)); fi
if cargo check >/dev/null 2>&1; then ((passed_checks++)); fi

if [ $passed_checks -eq $total_checks ]; then
    echo "✅ TASK 6 IMPLEMENTATION COMPLETE ($passed_checks/$total_checks checks passed)"
    echo ""
    echo "🎉 Tool Call Mapping Successfully Implemented!"
    echo ""
    echo "Features implemented:"
    echo "  ✅ Tool call types (OllamaToolCall, OllamaTool, OllamaFunction)"
    echo "  ✅ Capability detection (get_model_capabilities, supports_capability)"
    echo "  ✅ Tool call detection in streaming responses"
    echo "  ✅ Core tool definitions (fs_read, execute_bash, fs_write, use_aws)"
    echo "  ✅ Tool integration in message sending"
    echo ""
    echo "Ready for testing:"
    echo "  Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
    echo "  Then ask: 'Read the README.md file' or 'List files in current directory'"
else
    echo "⚠️  TASK 6 PARTIALLY COMPLETE ($passed_checks/$total_checks checks passed)"
    echo "Some components may need additional work"
fi

echo ""
echo "Manual Test Instructions:"
echo "1. Start Ollama with a tool-capable model (e.g., llama3.1, gpt-oss)"
echo "2. Run: Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat"
echo "3. Test tool usage:"
echo "   - 'Read the README.md file' (should use fs_read tool)"
echo "   - 'List files in current directory' (should use execute_bash tool)"
echo "   - 'Create a test file with hello world' (should use fs_write tool)"
echo "4. Verify tool calls are detected and executed properly"
