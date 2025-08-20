# Plugin Architecture Refactor Analysis

## 🎯 **Goal**
Refactor Tasks 1-6 implementation from direct core modifications to clean plugin pattern.

## 📊 **Current Changes Analysis**

### **Modified Core Files** (Need Plugin Pattern)
1. **`crates/chat-cli/src/main.rs`** - Environment validation
2. **`crates/chat-cli/src/util/mod.rs`** - ModelProvider enum
3. **`crates/chat-cli/src/auth/builder_id.rs`** - Auth bypass
4. **`crates/chat-cli/src/api_client/mod.rs`** - Provider routing, Ollama methods
5. **`crates/chat-cli/src/api_client/send_message_output.rs`** - Ollama variants
6. **`crates/chat-cli/src/api_client/error.rs`** - Ollama error types
7. **`crates/chat-cli/src/cli/chat/cli/model.rs`** - Model selection
8. **`crates/chat-cli/src/database/settings.rs`** - Settings (if any)

### **New Files** (Keep As-Is, Maybe Reorganize)
1. **`crates/chat-cli/src/api_client/ollama.rs`** - Ollama client implementation

### **Dependencies Added**
1. **`Cargo.toml`** - Workspace dependencies
2. **`crates/chat-cli/Cargo.toml`** - Ollama-specific dependencies

## 🏗️ **Target Plugin Architecture**

### **New Structure**
```
crates/chat-cli/src/
├── providers/                    # NEW: Plugin system
│   ├── mod.rs                   # Provider trait & registry
│   ├── ollama/                  # Ollama plugin
│   │   ├── mod.rs              # Plugin implementation
│   │   ├── client.rs           # HTTP client (current ollama.rs)
│   │   ├── types.rs            # Ollama-specific types
│   │   └── auth.rs             # Ollama auth (none needed)
│   ├── openai/                  # Future: OpenAI plugin
│   └── anthropic/               # Future: Anthropic plugin
├── api_client/
│   ├── mod.rs                   # MINIMAL changes - plugin integration
│   ├── send_message_output.rs   # REVERT Ollama variants
│   └── error.rs                 # REVERT Ollama errors
├── util/
│   └── mod.rs                   # REVERT ModelProvider changes
├── auth/
│   └── builder_id.rs            # REVERT auth bypass
└── main.rs                      # REVERT env validation
```

### **Plugin Interface**
```rust
// providers/mod.rs
pub trait MessageProvider: Send + Sync {
    async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ProviderError>;
    fn provider_name(&self) -> &'static str;
    fn requires_auth(&self) -> bool;
    fn supports_streaming(&self) -> bool;
}

pub struct ProviderResponse {
    pub content: Option<String>,
    pub stream: Option<Box<dyn Stream<Item = ResponseChunk>>>,
    pub metadata: ProviderMetadata,
}

pub enum ProviderError {
    NetworkError(String),
    AuthError(String),
    ModelError(String),
    UnsupportedFeature(String),
}
```

## 📋 **Refactor Steps**

### **Step 1: Create Plugin Infrastructure**
- [ ] Create `providers/mod.rs` with trait definition
- [ ] Create provider registry system
- [ ] Create unified response types

### **Step 2: Extract Ollama to Plugin**
- [ ] Move `ollama.rs` to `providers/ollama/client.rs`
- [ ] Create `providers/ollama/mod.rs` implementing MessageProvider
- [ ] Create `providers/ollama/types.rs` for Ollama-specific types

### **Step 3: Minimal Core Integration**
- [ ] Add single `external_providers` field to ApiClient
- [ ] Add single provider check in `send_message()`
- [ ] Keep all existing AWS logic unchanged

### **Step 4: Revert Core Modifications**
- [ ] Revert `ModelProvider` enum changes
- [ ] Revert `SendMessageOutput` Ollama variants
- [ ] Revert auth bypass logic
- [ ] Revert environment validation

### **Step 5: Plugin-Based Environment Handling**
- [ ] Move environment validation to plugin initialization
- [ ] Move auth bypass to plugin-specific logic
- [ ] Move model selection to plugin-specific logic

### **Step 6: Update Tests & Validation**
- [ ] Update all validation scripts
- [ ] Update test commands
- [ ] Verify all Tasks 1-6 functionality still works

## 🎯 **Success Criteria**

### **Functionality Preserved**
- [ ] All Tasks 1-6 features work identically
- [ ] Environment variables work the same
- [ ] Model selection works the same
- [ ] Tool calls work the same
- [ ] Streaming works the same

### **Architecture Improved**
- [ ] Core Q CLI files minimally modified
- [ ] Plugin system supports future providers
- [ ] Clean separation of concerns
- [ ] Easy to add OpenAI/Anthropic later

### **Maintainability Enhanced**
- [ ] Rebase conflicts minimized
- [ ] Provider code isolated
- [ ] Clear ownership boundaries
- [ ] Independent testing possible

## 🚨 **Risk Mitigation**

### **Backup Strategy**
- [ ] Create git branch before starting
- [ ] Commit working state before each major step
- [ ] Keep validation scripts updated throughout

### **Rollback Plan**
- [ ] If refactor fails, revert to current working state
- [ ] Continue with current architecture if time constraints
- [ ] Document technical debt for future cleanup

## 📅 **Estimated Effort**
- **Step 1-2**: 2-3 hours (Plugin infrastructure + Ollama extraction)
- **Step 3-4**: 1-2 hours (Core integration + revert changes)
- **Step 5-6**: 1-2 hours (Environment handling + testing)
- **Total**: 4-7 hours of focused work

## 🤔 **Decision Points**
1. **Scope**: Full refactor vs minimal plugin pattern?
2. **Timing**: Now vs after completing more features?
3. **Testing**: How much validation during refactor?
4. **Rollback**: At what point do we commit to the new architecture?
