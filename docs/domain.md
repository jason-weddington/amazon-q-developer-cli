# Domain Concepts

Amazon Q Developer CLI is a command-line interface that provides AI-powered assistance for developers, integrating with AWS services and supporting extensible agent-based workflows through the Model Context Protocol (MCP).

## Core Terminology

- **Agent**: A configurable AI assistant with specific tools, prompts, and capabilities defined in JSON configuration files
- **MCP (Model Context Protocol)**: An open protocol that standardizes how applications provide context to LLMs through locally running servers
- **MCP Server**: A process that provides additional tools and resources to extend agent capabilities
- **Tool**: A function that agents can invoke to perform actions (built-in tools like `fs_read`, `execute_bash`, or MCP server tools)
- **Conversation**: A chat session between a user and an agent, persisted in the local database
- **Context**: Information provided to agents including files, directories, system state, and conversation history
- **Knowledge Store**: A semantic search system for storing and retrieving contextual information
- **Streaming Client**: Real-time communication interface with Amazon Q Developer services
- **Bearer Token**: Authentication mechanism for AWS Builder ID and IAM Identity Center users
- **Telemetry**: Usage analytics and performance metrics sent to AWS services

## Business Rules

- **Authentication Required**: Chat and profile commands require valid AWS authentication (Builder ID or IAM Identity Center)
- **Agent Isolation**: Each agent operates with its own configuration, tools, and MCP servers
- **Tool Permissions**: Tools can be configured as allowed (no prompt), denied, or require user confirmation
- **File Access Control**: File operations respect configured allowed/denied path patterns
- **Command Safety**: Bash commands can be restricted through allow/deny lists and read-only modes
- **Conversation Persistence**: All conversations are stored locally in SQLite database
- **Token Limits**: Conversations have token limits with warnings and automatic compaction
- **Timeout Management**: MCP server requests have configurable timeouts (default 120 seconds)
- **Telemetry Opt-out**: Users can opt out of telemetry collection
- **Cross-Platform Support**: Must work on macOS, Linux, and Windows with platform-specific optimizations

## User Roles

- **Developer**: Primary user who interacts with agents for coding assistance, AWS operations, and development tasks
- **Agent Administrator**: User who configures agents, MCP servers, and tool permissions
- **System Administrator**: User who manages CLI installation, authentication, and system-wide settings

## Process Flows

### 1. Authentication Flow
1. User runs `q login` command
2. CLI opens browser for AWS Builder ID or IAM Identity Center authentication
3. User completes OAuth flow in browser
4. CLI receives and stores authentication tokens
5. Tokens are refreshed automatically as needed

### 2. Agent Conversation Flow
1. User starts chat with `q chat` or `q chat --agent <name>`
2. CLI loads agent configuration and initializes MCP servers
3. User sends message to agent
4. Agent processes message and may invoke tools
5. Tool results are incorporated into agent response
6. Response is streamed back to user
7. Conversation state is persisted to database

### 3. MCP Server Integration Flow
1. Agent configuration specifies MCP servers
2. CLI starts MCP server processes using stdio transport
3. CLI discovers available tools and resources from servers
4. Agent can invoke MCP tools during conversations
5. MCP servers are managed (started/stopped) automatically

### 4. Knowledge Management Flow
1. User or agent adds context to knowledge store
2. Content is processed and indexed (BM25 + semantic embeddings)
3. During conversations, relevant context is retrieved
4. Context is included in agent prompts for better responses

## Domain Models

### Agent Configuration
- **name**: Agent identifier (derived from filename)
- **description**: Human-readable agent purpose
- **prompt**: System-level context and instructions
- **mcpServers**: Map of MCP server configurations
- **tools**: List of available tools (built-in and MCP)
- **allowedTools**: Tools that don't require user confirmation
- **toolsSettings**: Per-tool configuration options
- **resources**: Available resources and permissions
- **hooks**: Commands executed at specific trigger points

### Conversation State
- **conversationId**: Unique identifier for conversation
- **messages**: Ordered list of user and assistant messages
- **tokenCount**: Current token usage tracking
- **metadata**: Request/response metadata and timing
- **toolUses**: History of tool invocations and results

### Authentication Profile
- **profileType**: Builder ID or IAM Identity Center
- **credentials**: AWS credentials and tokens
- **region**: AWS region for API calls
- **expirationTime**: Token expiration timestamp
- **refreshToken**: Token for credential renewal

### MCP Server Configuration
- **command**: Executable command to start server
- **args**: Command-line arguments
- **env**: Environment variables
- **timeout**: Request timeout in milliseconds
- **transport**: Communication protocol (stdio, websocket)

### Tool Specification
- **name**: Tool identifier
- **description**: Tool purpose and usage
- **inputSchema**: JSON schema for tool parameters
- **permissions**: Access control settings
- **source**: Built-in or MCP server origin

### Knowledge Context
- **contextId**: Unique identifier
- **name**: Human-readable name
- **description**: Context purpose
- **indexType**: Fast (BM25) or Best (semantic)
- **includePaths**: File patterns to include
- **excludePaths**: File patterns to exclude
- **chunkSize**: Text segmentation size
- **embeddings**: Vector representations for semantic search