# Task 7 Research: Tool Call Execution and Result Handling

## 🔍 **Problem Statement**

Tool calls with Ollama models are completely broken, resulting in blank responses when models attempt to use tools. This is a critical bug that prevents basic functionality.

## 🧪 **Evidence from User Testing**

### **Conversation Example**
```
User: try to save a test file to ~/git/q_fork_test.
Assistant: [BLANK RESPONSE]

User: did it work?
Assistant: [BLANK RESPONSE]

[Later in conversation]
bash: line 1: it: command not found
Self exited with status: exit status: 127

User: what tools can you use?
Assistant: Here's a quick rundown of the built‑in "tool" primitives...
[Lists all tools correctly]
```

### **Key Observations**
1. ✅ **Model knows about tools** - can describe them accurately
2. ✅ **Tools are sent to Ollama** - model has access to tool definitions
3. ✅ **Conversation history works** - model remembers context
4. ✅ **User sees streaming responses** - text appears progressively
5. ❌ **Tool calls result in blank responses** - critical failure
6. ❌ **Some corrupted tool execution** - wrong commands being run
7. ❌ **No tool results returned** - execution pipeline broken

## 🔬 **Technical Investigation**

### **Current Architecture**
```rust
// Ollama Provider sends non-streaming requests
let request = OllamaChatRequest {
    model,
    messages: ollama_messages,
    tools: Some(ollama_tools),
    stream: Some(false), // ← Non-streaming
    // ...
};

let response = self.client.chat(request).await?;
Ok(ProviderResponse::Ollama(response))
```

### **Response Processing**
```rust
// Non-streaming responses converted to Mock
ProviderResponse::Ollama(response) => {
    let content = response.message.content.unwrap_or_else(|| "No content".to_string());
    let mock_content = vec![
        ChatResponseStream::AssistantResponseEvent { content }
    ];
    SendMessageOutput::Mock(mock_content)
}
```

### **Mock Response Handling**
```rust
// Mock just pops from vector - no real streaming
SendMessageOutput::Mock(vec) => Ok(vec.pop())
```

## 🧩 **Root Cause Analysis**

### **The Streaming Paradox**
- **Code shows**: `stream: Some(false)` (non-streaming)
- **User experiences**: Progressive text streaming
- **Conclusion**: There's a disconnect between our implementation and what's actually happening

### **Tool Call Detection Gap**
1. **Ollama generates tool calls** ✅ (confirmed by direct API test)
2. **Tool calls ignored in non-streaming path** ❌
3. **Tool execution pipeline exists** ✅ (we see bash errors)
4. **Tool call parsing is broken** ❌ (wrong commands executed)

### **Direct API Test Results**
```bash
curl -X POST http://localhost:11434/api/chat \
  -d '{"model": "gpt-oss:20b", "messages": [...], "tools": [...], "stream": false}'
```

**Response:**
```json
{
  "message": {
    "role": "assistant",
    "content": "",  // ← Empty when tool calls present
    "tool_calls": [
      {
        "function": {
          "name": "fs_write",
          "arguments": {"command": "write", "path": "test.txt", "file_text": ""}
        }
      }
    ]
  },
  "done": true
}
```

**Key Finding**: Ollama returns `content: ""` when tool calls are present, but includes the tool calls in the response.

## 🎯 **The Real Issues**

### **Issue 1: Tool Call Detection Missing**
Our non-streaming response handler only extracts `content`, ignoring `tool_calls`:

```rust
// CURRENT (BROKEN)
let content = response.message.content.unwrap_or_else(|| "No content".to_string());
// ↑ This gets empty string when tool calls are present

// NEEDED
if let Some(tool_calls) = response.message.tool_calls {
    // Convert to ToolUseEvent and execute tools
} else {
    // Handle regular content
}
```

### **Issue 2: Streaming vs Non-Streaming Confusion**
- We're using non-streaming requests but user sees streaming
- Tool call detection is only implemented in streaming path
- Need to either:
  - Fix non-streaming tool call detection, OR
  - Switch to streaming and fix response conversion

### **Issue 3: Tool Execution Pipeline Corruption**
The mysterious `bash: line 1: it: command not found` suggests:
- Tool execution is partially working
- Command parsing is corrupted somewhere
- Tool results aren't being returned to conversation

## 🔧 **Solution Options**

### **Option 1: Fix Non-Streaming Tool Calls (Recommended)**
```rust
impl From<ProviderResponse> for SendMessageOutput {
    fn from(response: ProviderResponse) -> Self {
        match response {
            ProviderResponse::Ollama(response) => {
                use crate::api_client::model::ChatResponseStream;
                
                // Check for tool calls first
                if let Some(tool_calls) = &response.message.tool_calls {
                    // Convert tool calls to ToolUseEvent
                    let mut events = Vec::new();
                    
                    for tool_call in tool_calls {
                        events.push(ChatResponseStream::ToolUseEvent {
                            tool_use_id: tool_call.id.clone().unwrap_or_else(|| 
                                format!("ollama_tool_{}", uuid::Uuid::new_v4())),
                            name: tool_call.function.name.clone(),
                            input: Some(serde_json::to_string(&tool_call.function.arguments).unwrap_or_default()),
                            stop: Some(false),
                        });
                    }
                    
                    SendMessageOutput::Mock(events)
                } else {
                    // Handle regular content
                    let content = response.message.content.unwrap_or_else(|| "No content".to_string());
                    let mock_content = vec![
                        ChatResponseStream::AssistantResponseEvent { content }
                    ];
                    SendMessageOutput::Mock(mock_content)
                }
            },
            // ... other variants
        }
    }
}
```

### **Option 2: Switch to Streaming**
```rust
// Enable streaming in Ollama provider
let request = OllamaChatRequest {
    stream: Some(true), // ← Enable streaming
    // ...
};

let stream_receiver = self.client.chat_stream(request).await?;
Ok(ProviderResponse::OllamaStreaming(stream_receiver))
```

### **Option 3: Hybrid Approach**
- Use streaming for tool calls
- Use non-streaming for simple responses
- Detect tool presence and choose approach

## 🧪 **Testing Strategy**

### **Unit Tests**
```rust
#[test]
fn test_tool_call_detection_non_streaming() {
    let ollama_response = OllamaChatResponse {
        message: OllamaMessage {
            content: Some("".to_string()), // Empty content
            tool_calls: Some(vec![
                OllamaToolCall {
                    function: OllamaFunctionCall {
                        name: "fs_write".to_string(),
                        arguments: serde_json::json!({"command": "create", "path": "test.txt"}),
                    }
                }
            ]),
            // ...
        },
        // ...
    };
    
    let output = SendMessageOutput::from(ProviderResponse::Ollama(ollama_response));
    
    // Should generate ToolUseEvent, not AssistantResponseEvent
    match output {
        SendMessageOutput::Mock(events) => {
            assert!(events.iter().any(|e| matches!(e, ChatResponseStream::ToolUseEvent { .. })));
        },
        _ => panic!("Expected Mock output"),
    }
}
```

### **Integration Tests**
```bash
# Test tool execution end-to-end
Q_CLI_MODEL_PROVIDER=ollama cargo run --bin chat_cli -- chat
# Input: "Create a file called test.txt"
# Expected: File is created, tool execution visible, proper response
```

## 📋 **Task 7 Definition**

### **Task 7: Fix Tool Call Execution with Ollama**

**Description**: Fix the critical bug where Ollama tool calls result in blank responses due to missing tool call detection in non-streaming responses.

**Current Problem**:
- Ollama models generate tool calls correctly ✅
- Tool calls are ignored in non-streaming response processing ❌
- Results in blank responses when tools should be executed ❌
- Tool execution pipeline is partially working but corrupted ❌

**Acceptance Criteria**:
- [ ] Tool calls in Ollama responses are detected and processed
- [ ] Tool execution works end-to-end (request → execution → result → response)
- [ ] No more blank responses when models attempt to use tools
- [ ] Tool results are properly integrated into conversation flow
- [ ] Multi-tool scenarios work correctly
- [ ] Error handling for tool execution failures

**Implementation Approach**:
1. **Add tool call detection** to non-streaming response conversion
2. **Fix tool execution pipeline** corruption issues
3. **Ensure tool results** are returned to conversation
4. **Test end-to-end** tool execution scenarios

**Success Criteria**:
```bash
User: "Create a file called test.txt"
Assistant: [Executes fs_write tool]
Assistant: "I've created the file test.txt for you."
# File actually exists on filesystem
```

## 🚨 **Priority: CRITICAL**

This is a critical bug that breaks core functionality. Tool calls are a fundamental feature, and their complete failure makes Ollama integration unusable for many scenarios.

## 🔄 **Next Steps**

1. **Implement Option 1** (fix non-streaming tool call detection)
2. **Test tool execution** end-to-end
3. **Debug tool execution pipeline** corruption
4. **Add comprehensive error handling**
5. **Verify conversation history** includes tool results

This research confirms that Task 7 should focus on fixing the broken tool call execution rather than just conversation history handling.
