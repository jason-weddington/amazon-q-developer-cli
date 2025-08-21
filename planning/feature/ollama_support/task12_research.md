# Task 12 Research: Configurable Reasoning Effort for Thinking Models

## 🔍 **Problem Analysis**

### **Current State**
- Task 10 implements basic thinking tool support
- gpt-oss models support configurable reasoning effort ("low", "medium", "high")
- Currently hardcoded to "medium" reasoning effort
- No user configuration available

### **gpt-oss Model Capabilities**
From https://ollama.com/library/gpt-oss:
> **Configurable reasoning effort:** Easily adjust the reasoning effort (low, medium, high) based on your specific use case and latency needs.

## 🔬 **Technical Investigation**

### **Ollama API Support**
From `/Users/jason/git/ollama/docs/api.md` and source code analysis:

#### **Three Ways to Control Reasoning:**

1. **`think` Parameter** (Top-level in request):
   ```json
   {
     "model": "gpt-oss:20b",
     "messages": [...],
     "think": "high"  // ← Can be true/false or "high"/"medium"/"low"
   }
   ```

2. **`reasoning` in Options**:
   ```json
   {
     "model": "gpt-oss:20b", 
     "messages": [...],
     "options": {
       "reasoning": "medium"  // ← Also works here
     }
   }
   ```

3. **OpenAI Compatibility** (`reasoning_effort`):
   ```json
   {
     "model": "gpt-oss:20b",
     "messages": [...], 
     "reasoning_effort": "low"  // ← OpenAI-style parameter
   }
   ```

### **Ollama Source Code Analysis**

#### **ThinkValue Type** (from `/Users/jason/git/ollama/api/types.go`):
```go
// ThinkValue represents a value that can be a boolean or a string ("high", "medium", "low")
type ThinkValue struct {
    // Value can be a bool or string
    Value interface{}
}

// IsValid checks if the ThinkValue is valid
func (t *ThinkValue) IsValid() bool {
    switch v := t.Value.(type) {
    case bool:
        return true
    case string:
        return v == "high" || v == "medium" || v == "low"
    default:
        return false
    }
}
```

#### **API Integration**:
- Both `/api/generate` and `/api/chat` endpoints support `think` parameter
- Can be passed via `options.reasoning` as well
- OpenAI compatibility layer maps `reasoning_effort` to internal `think`

### **Current Q CLI Settings**
From `crates/chat-cli/src/database/settings.rs`:

#### **Existing Settings:**
- ✅ `EnabledThinking` → `"chat.enableThinking"` (already exists)
- ❌ **No reasoning effort setting exists**

#### **Would Need to Add:**
```rust
pub enum Setting {
    // ... existing settings
    ChatReasoningEffort,  // ← NEW: Would need to add this
}

impl AsRef<str> for Setting {
    fn as_ref(&self) -> &'static str {
        match self {
            // ... existing mappings
            Self::ChatReasoningEffort => "chat.reasoningEffort", // ← NEW
        }
    }
}
```

## 🎯 **Implementation Plan**

### **Phase 1: Add Setting Support**
Add new setting to upstream code:

```rust
// In settings.rs - MINIMAL UPSTREAM TOUCH
pub enum Setting {
    // ... existing settings
    ChatReasoningEffort,
}

impl AsRef<str> for Setting {
    fn as_ref(&self) -> &'static str {
        match self {
            // ... existing mappings
            Self::ChatReasoningEffort => "chat.reasoningEffort",
        }
    }
}
```

### **Phase 2: Update Ollama Provider**
Modify our provider to use the setting:

```rust
// In providers/ollama/mod.rs - OUR CODE
let reasoning_effort = self.database.settings
    .get_string(Setting::ChatReasoningEffort)
    .unwrap_or_else(|_| "medium".to_string());

let request = OllamaChatRequest {
    model,
    messages: ollama_messages,
    tools: Some(ollama_tools),
    think: Some(reasoning_effort), // ← Use setting value
    stream: Some(true),
    // ...
};
```

### **Phase 3: Add Validation**
Ensure only valid values are accepted:

```rust
impl Settings {
    pub fn set_reasoning_effort(&mut self, effort: &str) -> Result<(), DatabaseError> {
        match effort {
            "low" | "medium" | "high" => {
                self.set(Setting::ChatReasoningEffort, effort).await
            },
            _ => Err(DatabaseError::InvalidValue(format!(
                "Invalid reasoning effort: '{}'. Valid values: low, medium, high", 
                effort
            ))),
        }
    }
}
```

## 🧪 **Testing Strategy**

### **Manual Test Cases**
```bash
# Test 1: Set reasoning effort
q settings chat.reasoningEffort high
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
# Should use high reasoning effort

# Test 2: Invalid value
q settings chat.reasoningEffort invalid
# Should show error with valid options

# Test 3: Default behavior
q settings chat.reasoningEffort --unset
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b  
# Should default to "medium"

# Test 4: Non-thinking models
q settings chat.reasoningEffort high
Q_CLI_MODEL_PROVIDER=ollama q chat --model llama3.2
# Should work normally (reasoning effort ignored for non-thinking models)
```

### **Unit Tests**
```rust
#[tokio::test]
async fn test_reasoning_effort_setting() {
    // Test valid values are accepted
    // Test invalid values are rejected
    // Test default behavior
}

#[tokio::test]
async fn test_ollama_request_with_reasoning_effort() {
    // Test that reasoning effort is included in request
    // Test different effort levels
}
```

## 📋 **Acceptance Criteria**
- [ ] New `chat.reasoningEffort` setting available
- [ ] Valid values: "low", "medium", "high"
- [ ] Default value: "medium"
- [ ] Invalid values show helpful error message
- [ ] Setting applies to all thinking-capable Ollama models
- [ ] Non-thinking models ignore the setting gracefully
- [ ] Existing functionality unchanged

## 📁 **Files to Modify**
- `crates/chat-cli/src/database/settings.rs` - Add new setting (minimal upstream touch)
- `crates/chat-cli/src/providers/ollama/mod.rs` - Use setting in requests
- Add validation and error handling
- Add comprehensive tests

## 🎯 **Success Criteria**
**Before Task 12:**
```bash
# Hardcoded "medium" reasoning effort
# No user configuration available
```

**After Task 12:**
```bash
q settings chat.reasoningEffort high
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
# Uses high reasoning effort for better quality (slower)

q settings chat.reasoningEffort low  
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
# Uses low reasoning effort for faster responses
```

## 🔗 **Dependencies**
- **Requires**: Task 10 (basic thinking tool support) completed
- **Enables**: User control over reasoning quality vs. speed trade-offs
- **Future**: Could be extended to per-conversation or per-model settings
