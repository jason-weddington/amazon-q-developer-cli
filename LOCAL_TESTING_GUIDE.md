# 🧪 Local Testing Guide

## Current Implementation Status

### ✅ **Completed Tasks**
- **Task 1**: Environment Variable Validation and Provider Selection
- **Task 2**: Basic Ollama HTTP Client

### 🚧 **Remaining Tasks**
- **Task 3**: Add Ollama Variant to SendMessageOutput Enum
- **Task 4**: Implement Message Format Conversion  
- **Task 5**: Add Streaming Response Handling
- **Task 6**: Implement Tool Call Mapping

---

## 🔧 **Setup Instructions**

### 1. **Rust Environment**
```bash
# Source Rust environment (required for all tests)
source ~/.cargo/env

# Verify Rust is available
rustc --version
cargo --version
```

### 2. **Optional: Install Ollama for Full Integration Testing**
```bash
# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Start Ollama server
ollama serve &

# Pull a test model
ollama pull llama3.2
```

---

## 🧪 **Testing What's Currently Working**

### **Task 1: Environment Variable Validation**

#### ✅ **Test Invalid Provider**
```bash
Q_CLI_MODEL_PROVIDER=invalid cargo run --bin chat_cli -- --help
```
**Expected**: Clear error message with valid options

#### ✅ **Test OpenAI Without API Key**
```bash
Q_CLI_MODEL_PROVIDER=openai cargo run --bin chat_cli -- --help
```
**Expected**: Error requiring `Q_CLI_MODEL_PROVIDER_API_KEY`

#### ✅ **Test Ollama Provider**
```bash
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --help
```
**Expected**: Help text displays normally (no "UnsupportedProvider" error)

#### ✅ **Test Default Behavior**
```bash
cargo run --bin chat_cli -- --help
```
**Expected**: Works exactly as before (AWS provider)

### **Task 2: Ollama HTTP Client**

#### ✅ **Unit Tests (All Mocked)**
```bash
# Run all Ollama unit tests
cargo test ollama::tests

# Run with output
cargo test ollama::tests -- --nocapture
```
**Expected**: 7/7 tests pass

#### ✅ **Custom Base URL**
```bash
Q_CLI_MODEL_PROVIDER=ollama Q_CLI_MODEL_PROVIDER_BASE_URL=http://localhost:8080 cargo run --bin chat_cli -- --help
```
**Expected**: Works with custom URL

#### ✅ **Integration with Real Ollama Server** (if installed)
```bash
# Start Ollama first: ollama serve &
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --help
```
**Expected**: No connection warnings in logs

---

## ❌ **What Doesn't Work Yet**

### **Chat Commands (Need Tasks 3-4)**
```bash
# This will fail - message conversion not implemented
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
```
**Expected Error**: Will fail because Ollama messages aren't converted to AWS format yet

### **Streaming (Need Task 5)**
- Streaming responses not implemented
- Currently forced to non-streaming mode

### **Tool Calls (Need Task 6)**
- Tool calling not implemented yet

---

## 🔍 **Detailed Test Results**

### **Environment Validation Results**
```bash
# ❌ Invalid provider
Q_CLI_MODEL_PROVIDER=invalid cargo run --bin chat_cli -- --help
# Error: Invalid Q_CLI_MODEL_PROVIDER: 'invalid'. Valid values are: aws, ollama, openai, anthropic

# ❌ OpenAI without API key  
Q_CLI_MODEL_PROVIDER=openai cargo run --bin chat_cli -- --help
# Error: Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using OpenAI provider

# ✅ Ollama works
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- --help
# Success: Shows help menu

# ✅ Default works
cargo run --bin chat_cli -- --help  
# Success: Shows help menu (AWS provider)
```

### **HTTP Client Test Results**
```bash
# ✅ All unit tests pass
cargo test ollama::tests
# test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured

# Tests cover:
# - Successful chat requests
# - Server error handling  
# - Model listing
# - Health checks
# - Stream override (forced to false)
# - Base URL configuration
```

---

## 🚀 **Next Steps for Full Ollama Support**

### **Task 3: SendMessageOutput Integration**
- Add `Ollama(OllamaChatResponse)` variant to enum
- Enable proper response handling

### **Task 4: Message Conversion**  
- Convert between AWS and Ollama message formats
- Handle role mapping (user/assistant/system)

### **Task 5: Streaming**
- Implement streaming response handling
- Real-time message display

### **Task 6: Tool Calls**
- Map Ollama tool calls to AWS format
- Enable function calling

---

## 🐛 **Troubleshooting**

### **"cargo: command not found"**
```bash
# Source Rust environment
source ~/.cargo/env
```

### **Compilation Warnings**
- Warnings about unused imports/fields are expected
- These will be used in upcoming tasks

### **Connection Warnings**
- Health check failures are logged as warnings but don't prevent startup
- Normal when Ollama server isn't running

### **Test Failures**
- Unit tests should always pass (they use mocked responses)
- Integration tests require Ollama server for full functionality

---

## 📊 **Current Architecture**

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Environment   │───▶│   ModelProvider  │───▶│   ApiClient     │
│   Variables     │    │   Selection      │    │   Creation      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                         │
                                                         ▼
                       ┌─────────────────────────────────────────┐
                       │            ApiClient                    │
                       │  ┌─────────────┐  ┌─────────────────┐  │
                       │  │ AWS Client  │  │ Ollama Client   │  │
                       │  │ (existing)  │  │ (new in Task 2) │  │
                       │  └─────────────┘  └─────────────────┘  │
                       └─────────────────────────────────────────┘
```

The foundation is solid and ready for the remaining message conversion and streaming tasks!
