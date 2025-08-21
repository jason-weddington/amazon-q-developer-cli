# Task 13 Research: Accurate Context Window Reporting for Ollama Models

## 🔍 **Problem Analysis**

### **Current Issue**
- **Actual Model**: gpt-oss has **128K tokens** context window
- **Q CLI Reports**: **200K tokens** context window  
- **Usage Calculation**: Based on incorrect 200K limit (9.70% vs should be ~15.1%)
- **User Impact**: Misleading information about available context space

### **Real-World Testing Results**
```bash
!> /usage

Current context window (19390 of 200k tokens used)
██|██████████████████████████████████████████████████████████████████████████████ 9.70%

# Should actually show:
Current context window (19390 of 128k tokens used)
██|██████████████████████████████████████████████████████████████████████████████ 15.1%
```

## 🔬 **Root Cause Analysis**

### **Data Flow Investigation**
```
1. /usage command calls context_window_tokens(session.conversation.model_info.as_ref())
2. context_window_tokens() checks if model_info exists
3. If model_info exists: uses model_info.context_window_tokens
4. If model_info is None: uses default_context_window() → 200_000
```

### **Source Code Analysis**

#### **File: `crates/chat-cli/src/cli/chat/cli/model.rs`**

#### **Problem Source 1: Hardcoded Default (200K)**
```rust
// Line 247-249
fn default_context_window() -> usize {
    200_000  // ← This is where 200K comes from!
}
```

#### **Problem Source 2: Ollama Model Creation (200K)**
```rust
// Line 181-188
let ollama_models: Vec<ModelInfo> = model_names
    .into_iter()
    .map(|name| ModelInfo {
        model_name: Some(name.clone()),
        model_id: name,
        context_window_tokens: 200_000, // ← Hardcoded default for ALL Ollama models!
    })
    .collect();
```

#### **Problem Source 3: Fallback Model Creation (200K)**
```rust
// Line 51-56
pub fn from_id(model_id: String) -> Self {
    Self {
        model_id,
        model_name: None,
        context_window_tokens: 200_000, // ← Another hardcoded 200K
    }
}
```

### **AWS vs Ollama Difference**

#### **AWS Models (Correct Behavior):**
```rust
// Line 39-42
let context_window_tokens = model
    .token_limits()                    // ← Gets real limits from AWS API
    .and_then(|limits| limits.max_input_tokens())
    .map_or(default_context_window(), |tokens| tokens as usize);
```

#### **Ollama Models (Broken Behavior):**
```rust
// Line 186
context_window_tokens: 200_000, // ← Hardcoded, no API query
```

## 🎯 **Solution: Dynamic Ollama Querying**

### **Ollama API Support**
From previous research, Ollama's `/api/show` endpoint provides model metadata including context window information.

#### **API Call Example:**
```bash
curl -s -X POST http://localhost:11434/api/show -d '{"name": "gpt-oss:20b"}' | jq
```

#### **Expected Response Structure:**
```json
{
  "modelfile": "...",
  "parameters": "...",
  "template": "...",
  "details": {
    "parent_model": "",
    "format": "gguf",
    "family": "gpt-oss",
    "families": ["gpt-oss"],
    "parameter_size": "20B",
    "quantization_level": "Q4_0"
  },
  "model_info": {
    "general.architecture": "gpt-oss",
    "general.parameter_count": 20000000000,
    "gpt-oss.context_length": 128000,  // ← This is what we need!
    "gpt-oss.embedding_length": 4096
  }
}
```

### **Implementation Plan**

#### **Phase 1: Enhance OllamaClient**
Add method to query model context window:

```rust
impl OllamaClient {
    pub async fn get_model_context_window(&self, model: &str) -> Result<Option<usize>, OllamaError> {
        let model_info = self.get_model_info(model).await?;
        
        // Extract context window from model_info
        if let Some(context_length) = model_info.model_info.get("gpt-oss.context_length") {
            if let Some(length) = context_length.as_u64() {
                return Ok(Some(length as usize));
            }
        }
        
        // Try other common context length fields
        for field in ["context_length", "max_position_embeddings", "n_ctx"] {
            if let Some(length) = model_info.model_info.get(field) {
                if let Some(length) = length.as_u64() {
                    return Ok(Some(length as usize));
                }
            }
        }
        
        Ok(None) // No context window info found
    }
}
```

#### **Phase 2: Update Ollama Model Creation**
Replace hardcoded 200K with dynamic querying:

```rust
// In model.rs, around line 181-188
let mut ollama_models = Vec::new();
for name in model_names {
    let context_window_tokens = if let Some(provider) = os.client.ollama_client() {
        provider.get_model_context_window_with_fallback(&name).await
    } else {
        200_000 // Fallback if no Ollama client (shouldn't happen in Ollama mode)
    };
    
    ollama_models.push(ModelInfo {
        model_name: Some(name.clone()),
        model_id: name,
        context_window_tokens,
    });
}
```

#### **Phase 3: Add Caching & Error Handling**
Cache model metadata to avoid repeated API calls:

```rust
// Add to OllamaClient
struct ModelMetadataCache {
    cache: std::collections::HashMap<String, usize>,
    last_updated: std::time::Instant,
}

impl OllamaClient {
    pub async fn get_cached_context_window(&mut self, model: &str) -> Result<usize, OllamaError> {
        // Check cache first
        if let Some(&cached_size) = self.metadata_cache.cache.get(model) {
            return Ok(cached_size);
        }
        
        // Query and cache
        let context_window = self.get_model_context_window(model).await?
            .unwrap_or(200_000); // Fallback to 200K if no context info found
        
        self.metadata_cache.cache.insert(model.to_string(), context_window);
        Ok(context_window)
    }
}
```

### **Error Handling Strategy**
For cases where dynamic querying fails:

```rust
impl OllamaClient {
    pub async fn get_model_context_window_with_fallback(&self, model: &str) -> usize {
        match self.get_model_context_window(model).await {
            Ok(Some(size)) => size,
            Ok(None) => {
                tracing::warn!("No context window info found for model {}, using default 200K", model);
                200_000
            },
            Err(e) => {
                tracing::warn!("Failed to query context window for model {}: {}, using default 200K", model, e);
                200_000
            }
        }
    }
}
```

**Rationale**: If Ollama isn't working, users can't use Ollama models anyway, so fallback to 200K is sufficient for error cases.

## 🧪 **Testing Strategy**

### **Unit Tests**
```rust
#[tokio::test]
async fn test_ollama_context_window_query() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/show")
        .with_status(200)
        .with_body(r#"{
            "model_info": {
                "gpt-oss.context_length": 128000
            }
        }"#)
        .create_async()
        .await;
        
    let client = OllamaClient::new(server.url());
    let context_window = client.get_model_context_window("gpt-oss:20b").await.unwrap();
    
    assert_eq!(context_window, Some(128_000));
    mock.assert_async().await;
}
```

### **Integration Tests**
```bash
# Test 1: gpt-oss model shows correct context window
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
/usage
# Should show "128k tokens" not "200k tokens"

# Test 2: Unknown model falls back gracefully
Q_CLI_MODEL_PROVIDER=ollama q chat --model unknown-model:latest
/usage
# Should show "200k tokens" (fallback)

# Test 3: Network failure handling
# Stop Ollama server temporarily
Q_CLI_MODEL_PROVIDER=ollama q chat --model gpt-oss:20b
/usage
# Should show "200k tokens" (fallback) and not crash
```

## 📋 **Implementation Requirements**

### **Phase 1: Core Functionality**
- [ ] Add `get_model_context_window()` method to `OllamaClient`
- [ ] Update Ollama model creation to use dynamic querying
- [ ] Add graceful fallback to 200K when query fails
- [ ] Handle network errors and timeouts

### **Phase 2: Performance & Reliability**
- [ ] Add metadata caching to avoid repeated API calls
- [ ] Implement known model lookup table for common models
- [ ] Add retry logic for failed queries
- [ ] Add configuration for cache TTL

### **Phase 3: User Experience**
- [ ] Add debug logging for context window detection
- [ ] Show warning when falling back to default
- [ ] Add `/model info` command to show model metadata
- [ ] Update help text to explain context window detection

## 🎯 **Success Criteria**

**Before Fix:**
```bash
!> /usage
Current context window (19390 of 200k tokens used) 9.70%
```

**After Fix:**
```bash
!> /usage
Current context window (19390 of 128k tokens used) 15.1%
```

## 📁 **Files to Modify**
- `crates/chat-cli/src/providers/ollama/client.rs` - Add context window querying
- `crates/chat-cli/src/providers/ollama/types.rs` - Add model info types
- `crates/chat-cli/src/cli/chat/cli/model.rs` - Update Ollama model creation (line 186)
- Add comprehensive tests and error handling

## 🔗 **Dependencies**
- **Requires**: Existing Ollama client and `/api/show` endpoint support
- **Enhances**: `/usage` command accuracy
- **Future**: Could enable per-model optimization and warnings
