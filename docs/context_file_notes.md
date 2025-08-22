# Context Files Analysis

## How Context Files Work in Q CLI

### Overview
Context files are re-injected into **every conversation turn** but do **NOT** become part of the permanent conversation history.

### Key Findings

#### Per-Turn Context Loading
- The `context_messages()` method in `conversation.rs` is called for **every message send** (line 458)
- Context is dynamically loaded fresh each turn, not cached from conversation start

#### Dynamic File Processing
Each turn calls `context_manager.collect_context_files_with_limit(os).await` which:
- Re-reads all context files from disk
- Processes glob patterns fresh each time (e.g., `docs/**/*.md` expands to current file state)
- Applies token limits (75% of model's context window via `calc_max_context_files_size`)
- Drops files if they exceed the limit

#### Context Injection Format
Context is injected as a synthetic conversation pair:
- **User message**: Contains all context files wrapped in `--- CONTEXT ENTRY BEGIN/END ---` markers
- **Assistant response**: "I will fully incorporate this information when generating my responses..."

#### Context Structure Per Turn
Each turn's context includes:
1. **Conversation summary** (if available)
2. **All matching context files** (re-read from disk)
3. **Additional context** (from hooks)
4. **Agent prompt** (system instructions)

#### Token Management
- Context files limited to 75% of model's context window
- Files dropped if they exceed limit (shown in `/context show` as "dropped")
- Current example: ~13,710 tokens across 4 files

### Architecture Details

#### Separation from History
In `BackendConversationState`, context is kept separate:
```rust
pub struct BackendConversationStateImpl {
    pub history: T,              // Permanent conversation history
    pub context_messages: U,     // Ephemeral context (not saved)
    // ...
}
```

#### Temporary Injection
The `context_messages()` method creates temporary `HistoryEntry` objects that get combined with real history only for the API call:
```rust
// Line 756 in conversation.rs
let history = flatten_history(self.context_messages.unwrap_or_default().iter().chain(self.history));
```

#### Not Persisted
- Context messages are created fresh each turn
- Never added to `self.history` 
- Just chained together for the API request
- Database only stores actual user/assistant exchanges

### Benefits of This Design

1. **Fresh Content**: File modifications between turns are automatically picked up
2. **Dynamic Patterns**: Glob patterns expand to current file state each time
3. **Token Awareness**: Respects model context limits per conversation turn
4. **Memory Efficient**: Conversation database doesn't grow by context file size per turn
5. **Consistent Format**: Uses same `--- CONTEXT ENTRY BEGIN/END ---` format across all contexts

### What the AI Sees vs. What's Stored

- **API Request**: Context files + conversation history as one combined stream
- **Database Storage**: Only real user/assistant message exchanges
- **Memory Usage**: Context files don't accumulate in conversation storage
- **User Experience**: Context appears "always available" but is actually re-injected each turn

### Implementation Files

Key files involved in context processing:
- `crates/chat-cli/src/cli/chat/context.rs` - Context manager and file processing
- `crates/chat-cli/src/cli/chat/conversation.rs` - Context injection into conversations
- `crates/chat-cli/src/cli/chat/cli/context.rs` - Context CLI commands (`/context show`, etc.)

### Example Context Output

```
👤 Agent (q_fork):
    AmazonQ.md (1 match)
    README.md (1 match)
    .amazonq/rules/**/*.md
    docs/domain.md (1 match)
    docs/codebase.md (1 match)

4 matched files in use:
👤 /Users/jason/git/amazon-q-developer-cli/AmazonQ.md (~4750 tkns)
👤 /Users/jason/git/amazon-q-developer-cli/docs/domain.md (~1460 tkns)
👤 /Users/jason/git/amazon-q-developer-cli/README.md (~600 tkns)
👤 /Users/jason/git/amazon-q-developer-cli/docs/codebase.md (~6900 tkns)

Total: ~13710 tokens
```

This represents files that are re-read and re-injected into every conversation turn, but never stored as part of the conversation history.
