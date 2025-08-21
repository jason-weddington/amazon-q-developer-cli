# Task 9 Research: Error Handling for Unsupported Models

## 🔍 **Problem Analysis**

### **Current Issue**
When using models that don't support tools (like `deepseek-r1:8b`), the CLI fails with:
```
Amazon Q is having trouble responding right now:
   0: Failed to send the request: Invalid configuration: Ollama server error 400: {"error":"registry.ollama.ai/library/deepseek-r1:8b does not support tools"}
```

### **Root Cause**
In `crates/chat-cli/src/providers/ollama/mod.rs`, the `send_message` method always includes tools:

```rust
async fn send_message(&self, conversation: ConversationState) -> Result<crate::providers::ProviderResponse, ApiClientError> {
    let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
    let ollama_tools = self.get_ollama_tools(&model).await?;  // ← Always gets tools
    
    let request = OllamaChatRequest {
        model,
        messages: ollama_messages,
        tools: Some(ollama_tools), // ← Always includes tools
        // ...
    };
}
```

## 🔬 **Technical Investigation**

### **Model Capabilities Research**
From Ollama API testing:

**Models WITH tool support:**
- `gpt-oss:120b` capabilities: `["completion", "tools", "thinking"]` ✅
- `llama3.2` capabilities: `["completion", "tools"]` ✅

**Models WITHOUT tool support:**
- `deepseek-r1:8b` capabilities: `["completion", "thinking"]` ❌ (no "tools")
- Many older/specialized models lack tool support

### **Existing Infrastructure**
We already have the capability detection infrastructure:

```rust
// In crates/chat-cli/src/providers/ollama/client.rs
pub async fn get_model_capabilities(&self, model: &str) -> Result<Vec<String>, OllamaError> {
    // Calls /api/show endpoint
}

pub async fn supports_capability(&self, model: &str, capability: &str) -> Result<bool, OllamaError> {
    let capabilities = self.get_model_capabilities(model).await?;
    Ok(capabilities.contains(&capability.to_string()))
}
```

### **Ollama API Behavior**
- **With tools**: Models that support tools work normally
- **Without tools**: Models return HTTP 400 with clear error message
- **Detection**: `/api/show` endpoint provides `capabilities` array

## 🎯 **Implementation Strategy**

### **Approach: Conditional Tool Inclusion**
Modify `send_message` to check model capabilities before including tools:

```rust
async fn send_message(&self, conversation: ConversationState) -> Result<crate::providers::ProviderResponse, ApiClientError> {
    let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
    
    // NEW: Check if model supports tools before including them
    let ollama_tools = if self.client.supports_capability(&model, "tools").await.unwrap_or(false) {
        Some(self.get_ollama_tools(&model).await?)
    } else {
        tracing::info!("Model {} does not support tools, proceeding with chat-only mode", model);
        None
    };
    
    let request = OllamaChatRequest {
        model,
        messages: ollama_messages,
        tools: ollama_tools, // ← Conditional based on capability
        // ...
    };
}
```

### **Error Handling Strategy**
1. **Graceful Degradation**: Models without tools work for basic chat
2. **User Notification**: Log info message about tool limitation
3. **Fallback Behavior**: Continue with chat-only functionality
4. **No Breaking Changes**: Existing tool-capable models unaffected

### **Edge Cases to Handle**
1. **Network Errors**: If capability check fails, assume no tools (safe default)
2. **Unknown Models**: If model not found, let Ollama handle the error
3. **Capability Check Timeout**: Use reasonable timeout, fallback to no tools
4. **Mixed Conversations**: Handle conversations that started with tools but switch to non-tool model

## 📋 **Implementation Plan**

### **Phase 1: Core Capability Check**
1. Modify `send_message` to check model capabilities
2. Conditionally include tools based on "tools" capability
3. Add appropriate logging for user awareness

### **Phase 2: Error Handling Enhancement**
1. Handle capability check failures gracefully
2. Add timeout for capability detection
3. Provide clear user feedback about tool limitations

### **Phase 3: User Experience Improvements**
1. Show model capabilities in `/model` command
2. Warn users when selecting non-tool models
3. Consider caching capability results for performance

## 🧪 **Testing Strategy**

### **Test Cases**
1. **Tool-capable model**: `gpt-oss:120b` - should include tools
2. **Non-tool model**: `deepseek-r1:8b` - should exclude tools, work for chat
3. **Network failure**: Capability check fails - should default to no tools
4. **Unknown model**: Model doesn't exist - let Ollama handle error
5. **Mixed conversation**: Switch between tool/non-tool models

### **Manual Testing**
```bash
# Test 1: Non-tool model should work for basic chat
Q_CLI_MODEL_PROVIDER=ollama q chat
/model
# Select deepseek-r1:8b
hello!  # Should work without tools

# Test 2: Tool-capable model should include tools
Q_CLI_MODEL_PROVIDER=ollama q chat
/model  
# Select gpt-oss:120b
create a file test.txt  # Should work with tools
```

## 🎯 **Success Criteria**

### **Functional Requirements**
- [ ] Models without tool support work for basic chat
- [ ] Models with tool support continue to work with tools
- [ ] No HTTP 400 errors from Ollama server
- [ ] Graceful degradation when capability check fails

### **User Experience Requirements**
- [ ] Clear logging about tool availability
- [ ] No breaking changes to existing workflows
- [ ] Reasonable performance (capability check shouldn't slow down chat)

### **Technical Requirements**
- [ ] Proper error handling for all edge cases
- [ ] Maintainable code with clear separation of concerns
- [ ] Comprehensive test coverage

## 🔄 **Future Enhancements**

### **Capability Caching**
Cache model capabilities to avoid repeated API calls:
```rust
struct CapabilityCache {
    cache: HashMap<String, Vec<String>>,
    ttl: Duration,
}
```

### **Enhanced Model Selection**
Show capabilities in model selection:
```
Available models:
1. gpt-oss:120b (tools, thinking, completion)
2. deepseek-r1:8b (thinking, completion) - no tools
3. llama3.2 (tools, completion)
```

### **Smart Tool Filtering**
Instead of all-or-nothing, filter tools based on model capabilities:
- Basic models: Only essential tools (fs_read, execute_bash)
- Advanced models: All tools including complex ones

## 📝 **Implementation Notes**

### **Key Files to Modify**
- `crates/chat-cli/src/providers/ollama/mod.rs` - Main capability check logic
- `crates/chat-cli/src/providers/ollama/client.rs` - Enhance error handling
- Add comprehensive tests for capability detection

### **Backward Compatibility**
- All existing functionality preserved
- No changes to AWS provider behavior
- Plugin architecture remains intact

### **Performance Considerations**
- Capability check adds one HTTP request per conversation
- Consider caching for frequently used models
- Timeout capability checks to avoid hanging

This implementation will solve the immediate problem while laying groundwork for more sophisticated capability-aware tool management in the future.
