# Task 8 Research: Tool Result Integration in Plugin Architecture

## 🔍 **Problem Analysis**

### **Current Behavior**
1. ✅ Ollama generates tool call → Tool executes successfully → File created
2. ❌ Tool result not included in conversation history → Model doesn't know it succeeded
3. ❌ Model tries same tool again → Infinite loop

### **Root Cause**
The issue is in our **conversation history conversion** in `convert_conversation_to_ollama()`. We convert assistant messages but don't include the tool results that follow them.

## 🔬 **Technical Investigation**

### **Current Code Analysis**
In `crates/chat-cli/src/providers/ollama/mod.rs`, the `convert_conversation_to_ollama()` function has critical TODOs:

```rust
ChatMessage::AssistantResponseMessage(assistant_msg) => {
    // TODO: Handle tool calls in assistant messages
    // For now, just convert as regular assistant message
    ollama_messages.push(OllamaMessage {
        role: "assistant".to_string(),
        content: Some(assistant_msg.content),
        thinking: None,
        images: None, // Assistants don't send images in Ollama
        tool_calls: None, // TODO: Convert tool uses to tool calls
        tool_call_id: None,
    });
},
```

### **AWS Data Structures**
From `crates/chat-cli/src/api_client/model.rs`:

```rust
pub struct AssistantResponseMessage {
    pub message_id: Option<String>,
    pub content: String,
    pub tool_uses: Option<Vec<ToolUse>>, // ← This is ignored!
}

pub struct ToolUse {
    pub tool_use_id: String,
    pub name: String,
    pub input: FigDocument,
}

pub struct UserInputMessageContext {
    pub env_state: Option<EnvState>,
    pub git_state: Option<GitState>,
    pub tool_results: Option<Vec<ToolResult>>, // ← This is also ignored!
    pub tools: Option<Vec<Tool>>,
}

pub struct ToolResult {
    pub tool_use_id: String,
    pub content: Vec<ToolResultContentBlock>,
    pub status: ToolResultStatus,
}
```

### **The Missing Flow**
AWS conversation flow:
```
User: "Create file"
Assistant: [tool_uses: [fs_write call]]
Tool Results: [success: "File created"]
User: "What did you create?"
Assistant: "I created the file as requested"
```

Current Ollama conversion:
```
User: "Create file"
Assistant: [content only, tool_uses ignored] ❌
[Tool results completely missing] ❌
User: "What did you create?"
Assistant: "Create file" [repeats because no tool context]
```

## 🎯 **Task 8 Implementation Plan**

# Task 8: Fix Tool Result Integration in Plugin Architecture

## **Description**
Fix the tool result integration issue where tool execution results are not included in conversation history, causing models to repeat tool calls infinitely. This affects all plugin providers (Ollama, OpenAI, Anthropic) and is a core plugin architecture issue.

## **Current Problem**
1. ✅ Tool calls work: Model → Tool Call → Tool Execution → Success
2. ❌ Tool results lost: Tool results not included in next conversation turn
3. ❌ Model loops: Model doesn't know tool succeeded, tries again infinitely

## **Root Cause**
The `convert_conversation_to_ollama()` function has two critical gaps:
1. **Assistant messages**: `tool_uses` are ignored (marked as TODO)
2. **User messages**: `tool_results` from `user_input_message_context` are ignored

## **Implementation Approach**

### **Phase 1: Add Tool Use Conversion**
Convert AWS `ToolUse` to Ollama `tool_calls` in assistant messages:

```rust
ChatMessage::AssistantResponseMessage(assistant_msg) => {
    if let Some(tool_uses) = &assistant_msg.tool_uses {
        // Convert tool uses to Ollama tool calls
        let ollama_tool_calls = tool_uses.iter().map(|tool_use| {
            OllamaToolCall {
                id: Some(tool_use.tool_use_id.clone()),
                function: OllamaFunctionCall {
                    name: tool_use.name.clone(),
                    arguments: serde_json::from_str(&tool_use.input.to_string())
                        .unwrap_or_else(|_| serde_json::Value::String(tool_use.input.to_string())),
                },
            }
        }).collect();
        
        ollama_messages.push(OllamaMessage {
            role: "assistant".to_string(),
            content: if assistant_msg.content.is_empty() { None } else { Some(assistant_msg.content.clone()) },
            thinking: None,
            images: None,
            tool_calls: Some(ollama_tool_calls),
            tool_call_id: None,
        });
    } else {
        // Regular assistant message without tools
        ollama_messages.push(OllamaMessage {
            role: "assistant".to_string(),
            content: Some(assistant_msg.content),
            thinking: None,
            images: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }
}
```

### **Phase 2: Add Tool Result Conversion**
Convert AWS `ToolResult` to Ollama tool result messages:

```rust
// Add current user message with tool results
let current_message = OllamaMessage {
    role: "user".to_string(),
    content: Some(conversation.user_input_message.content),
    thinking: None,
    images: self.convert_images_to_ollama(conversation.user_input_message.images)?,
    tool_calls: None,
    tool_call_id: None,
};

// Check for tool results in user message context
if let Some(context) = &conversation.user_input_message.user_input_message_context {
    if let Some(tool_results) = &context.tool_results {
        // Add tool result messages BEFORE the user message
        for tool_result in tool_results {
            let content = match &tool_result.content[0] {
                ToolResultContentBlock::Text(text) => text.clone(),
                ToolResultContentBlock::Json(json) => json.to_string(),
            };
            
            ollama_messages.push(OllamaMessage {
                role: "tool".to_string(),
                content: Some(content),
                thinking: None,
                images: None,
                tool_calls: None,
                tool_call_id: Some(tool_result.tool_use_id.clone()),
            });
        }
    }
}

ollama_messages.push(current_message);
```

### **Phase 3: Handle Multi-Tool Scenarios**
Ensure multiple tool calls and results are properly sequenced:

```rust
// Proper message sequence for multi-tool:
// 1. user: "Do X and Y"
// 2. assistant: [tool_calls: [call_X, call_Y]]
// 3. tool: result_X (tool_call_id: call_X)  
// 4. tool: result_Y (tool_call_id: call_Y)
// 5. user: "Follow up question"
```

## **Testing Strategy**

### **Unit Tests**
```rust
#[test]
fn test_convert_conversation_with_tool_uses() {
    let conversation = create_conversation_with_tool_uses();
    let (messages, _) = provider.convert_conversation_to_ollama(conversation).unwrap();
    
    // Should have: user → assistant (with tool_calls) → tool results → new user
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[1].role, "assistant");
    assert!(messages[1].tool_calls.is_some());
    assert_eq!(messages[2].role, "tool");
    assert!(messages[2].tool_call_id.is_some());
}

#[test]
fn test_convert_conversation_with_tool_results() {
    let conversation = create_conversation_with_tool_results();
    let (messages, _) = provider.convert_conversation_to_ollama(conversation).unwrap();
    
    // Tool results should appear before current user message
    let tool_message = messages.iter().find(|m| m.role == "tool").unwrap();
    assert!(tool_message.tool_call_id.is_some());
    assert!(tool_message.content.is_some());
}
```

### **Integration Tests**
```bash
# Test 1: Single tool call with result
echo "Create file test.txt" | Q_CLI_MODEL_PROVIDER=ollama q chat --trust-all-tools
# Expected: Tool executes once, model acknowledges success, no loop

# Test 2: Multi-tool scenario  
echo "Create file A.txt and file B.txt" | Q_CLI_MODEL_PROVIDER=ollama q chat --trust-all-tools
# Expected: Both tools execute, model acknowledges both, no loop

# Test 3: Follow-up after tool use
echo -e "Create file test.txt\nWhat did you create?" | Q_CLI_MODEL_PROVIDER=ollama q chat --trust-all-tools
# Expected: Tool executes, model responds about the file created
```

## **Success Criteria**
- [ ] Tool calls execute once and stop (no infinite loops)
- [ ] Model acknowledges tool execution results
- [ ] Multi-tool scenarios work correctly
- [ ] Follow-up questions after tool use work
- [ ] Tool results are visible in conversation history
- [ ] All existing functionality preserved

## **Files to Modify**
- `crates/chat-cli/src/providers/ollama/mod.rs` - Main conversion logic
- Add comprehensive unit tests for conversation conversion
- Update integration tests to verify tool result flow

## **Expected Outcome**
After Task 8, tool conversations will work naturally:
```
User: "Create a file called hello.txt"
Assistant: [Executes fs_write tool]
Assistant: "I've created the file hello.txt for you."
User: "What's in the file?"
Assistant: "The file contains..." [No tool loop, natural conversation]
```

This fix will benefit **all future providers** (OpenAI, Anthropic, etc.) since it's a core plugin architecture improvement.

## **Why This Isn't Ollama-Specific**

This is a **plugin architecture** issue that would affect any external provider:

- **AWS**: Handles conversation flow internally, tool results automatically included
- **Plugins**: Must manually convert conversation history between formats
- **Missing Piece**: Tool result round-trip conversion

The fix establishes the proper conversation history conversion pattern that all future providers (OpenAI, Anthropic, etc.) will benefit from.

## **Architecture Impact**

This task completes the plugin architecture by implementing the missing **conversation history round-trip**:

```
Plugin Model → Core Tool Execution → Tool Results → Plugin Model (next turn)
     ↑                                                        ↑
     └── This works ✅                    This was missing ❌ ──┘
```

After Task 8, the plugin architecture will be complete and robust for all providers.
