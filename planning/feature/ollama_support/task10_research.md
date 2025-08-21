# Task 10 Research: Enhanced Thinking Tool Capability Detection

## 🔍 **Problem Analysis**

### **Current State**
The thinking tool is already implemented in the core CLI but is NOT exposed to Ollama models:
- ✅ **Core Implementation**: `crates/chat-cli/src/cli/chat/tools/thinking.rs` exists
- ✅ **Tool Integration**: Thinking tool is part of `NATIVE_TOOLS` array
- ✅ **Settings Support**: `q settings chat.enableThinking true` enables it
- ❌ **Ollama Integration**: NOT included in `get_ollama_tools()` method
- ❌ **Capability Detection**: No check for "thinking" capability in models

### **Current TODO in Code**
```rust
// In get_ollama_tools() method:
// Check if model supports tools via capability detection
// For now, skip thinking tool - will add capability detection later
// TODO: Add thinking tool based on model capabilities and settings
debug!("Model {} tool capabilities will be checked in future implementation", model);
```

## 🔬 **Technical Investigation**

### **Thinking Tool Architecture**
The thinking tool is a special experimental tool that:
1. **Purpose**: Allows models to reason through complex problems during response generation
2. **Beta Feature**: Controlled by `chat.enableThinking` setting
3. **No System Operations**: Purely for model's internal reasoning process
4. **Display Logic**: Shows reasoning process to user, then returns empty output

### **Ollama Model Capabilities**
From API testing:
- **deepseek-r1:8b**: `["completion", "thinking"]` ✅ (HAS thinking capability)
- **gpt-oss:20b**: `["completion", "tools", "thinking"]` ✅ (HAS thinking capability)
- **llama3.2**: `["completion", "tools"]` ❌ (NO thinking capability)

## 🎯 **Implementation Plan**

### **Phase 1: Add Database to OllamaProvider**
Store database during provider construction to avoid circular dependency:

```rust
pub struct OllamaProvider {
    client: OllamaClient,
    base_url: String,
    database: Database, // ← Add this field
}

impl OllamaProvider {
    pub fn new(base_url: String, database: Database) -> Self {
        let client = OllamaClient::new(base_url.clone());
        Self { client, base_url, database }
    }
}
```

### **Phase 2: Minimal Upstream Change**
Pass database clone during provider creation:

```rust
// In ApiClient::new() - ONLY ONE LINE CHANGE
ModelProvider::Ollama => {
    let base_url = env.get("Q_CLI_MODEL_PROVIDER_BASE_URL")...;
    Some(Box::new(OllamaProvider::new(base_url, database.clone())))
    //                                           ^^^^^^^^^^^^^^^^ Add this
},
```

### **Phase 3: Add Thinking Tool**
Use stored database to check settings:

```rust
// In get_ollama_tools() - NO SIGNATURE CHANGES
if self.client.supports_capability(model, "thinking").await.unwrap_or(false) 
   && self.database.settings.get_bool(Setting::EnabledThinking).unwrap_or(false) {
    // Add thinking tool
}
```

### **Benefits of This Approach:**
- ✅ **Minimal Upstream Changes**: Only one line in ApiClient constructor
- ✅ **No Trait Changes**: MessageProvider signature unchanged
- ✅ **Standard CLI Behavior**: Settings loaded at startup (restart to apply changes)
- ✅ **Simple Implementation**: No complex parameter passing
- ✅ **Rebase Safe**: Fewer places to create merge conflicts

### **Trade-offs Accepted:**
- ⚠️ **Stale Settings**: Changes require restart (standard CLI behavior)
- ⚠️ **Memory Usage**: Settings map duplicated (minimal impact)

### **Success Criteria**
- Models with "thinking" capability get the thinking tool when enabled
- Models without "thinking" capability don't get the thinking tool
- Setting `chat.enableThinking false` disables thinking tool for all models
- Existing functionality unchanged
