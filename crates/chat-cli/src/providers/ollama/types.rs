use serde::{Deserialize, Serialize};
use std::pin::Pin;
use futures::Stream;
use tokio_stream::StreamExt;
use uuid;

use crate::api_client::model::ChatResponseStream;
use crate::api_client::ApiClientError;

#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("Ollama server error: {status} - {message}")]
    ServerError { status: u16, message: String },
    
    #[error("Model not found: {model}")]
    ModelNotFound { model: String },
    
    #[error("Connection failed to {url}")]
    ConnectionFailed { url: String },
    
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

// Ollama API types based on official documentation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,    // "system", "user", "assistant", "tool"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>, // For reasoning models like gpt-oss
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>, // For tool result messages
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Optional - Ollama doesn't always provide this
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>, // Optional - "function"
    pub function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaFunctionCall {
    pub name: String,
    pub arguments: serde_json::Value, // Can be object or string
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaTool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: OllamaFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

#[derive(Serialize, Debug)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OllamaTool>>, // Add tools support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>, // Default true, we'll set to false for Task 2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>, // "json" for JSON mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>, // Model parameters like temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>, // e.g. "5m"
}

#[derive(Deserialize, Debug, Clone)]
pub struct OllamaChatResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>, // Make optional to handle edge cases
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>, // Make optional to handle edge cases
    pub message: OllamaMessage,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_reason: Option<String>, // "stop", "length", etc.
    
    // Performance metrics (only in final response when done=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModel {
    pub name: String,
    pub model: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModel>,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModelInfo {
    pub modelfile: String,
    pub parameters: String,
    pub template: String,
    pub details: OllamaModelDetails,
    pub model_info: serde_json::Value,
    pub capabilities: Vec<String>, // ✨ Key field for capability detection
    pub modified_at: String,
}

#[derive(Deserialize, Debug)]
pub struct OllamaModelDetails {
    pub parent_model: String,
    pub format: String,
    pub family: String,
    pub families: Vec<String>,
    pub parameter_size: String,
    pub quantization_level: String,
}

/// Streaming response receiver for Ollama
pub struct OllamaStreamReceiver {
    stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send + Sync>>,
    ended: bool,
    pending_tool_events: Vec<ChatResponseStream>, // Queue for simulating AWS tool event pattern
}

impl std::fmt::Debug for OllamaStreamReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaStreamReceiver")
            .field("ended", &self.ended)
            .field("pending_events", &self.pending_tool_events.len())
            .field("stream", &"<stream>")
            .finish()
    }
}

impl OllamaStreamReceiver {
    pub fn new(stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send + Sync>>) -> Self {
        Self {
            stream,
            ended: false,
            pending_tool_events: Vec::new(),
        }
    }
    
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        // First, check if we have pending tool events to send
        if let Some(event) = self.pending_tool_events.pop() {
            return Ok(Some(event));
        }
        
        if self.ended {
            return Ok(None);
        }
        
        // Use loop instead of recursion to avoid stack overflow
        loop {
            match StreamExt::next(&mut self.stream).await {
                Some(Ok(ollama_response)) => {
                    // Check for tool calls FIRST, even in done messages
                    if let Some(tool_calls) = &ollama_response.message.tool_calls {
                        if let Some(tool_call) = tool_calls.first() {
                            // Generate ID if not provided by Ollama
                            let tool_use_id = tool_call.id.clone()
                                .unwrap_or_else(|| format!("ollama_tool_{}", uuid::Uuid::new_v4()));
                            
                            // Convert arguments to JSON string
                            let arguments_str = match &tool_call.function.arguments {
                                serde_json::Value::String(s) => s.clone(),
                                other => serde_json::to_string(other).unwrap_or_default(),
                            };
                            
                            // Simulate AWS pattern: multiple events for one tool call
                            // Event 2: Arguments event (will be sent next)
                            self.pending_tool_events.push(ChatResponseStream::ToolUseEvent {
                                tool_use_id: tool_use_id.clone(),
                                name: tool_call.function.name.clone(),
                                input: Some(arguments_str),
                                stop: Some(true), // Final event with complete arguments
                            });
                            
                            // Mark as ended if this is a done message
                            if ollama_response.done {
                                self.ended = true;
                            }
                            
                            // Event 1: Initial event (return immediately)
                            return Ok(Some(ChatResponseStream::ToolUseEvent {
                                tool_use_id,
                                name: tool_call.function.name.clone(),
                                input: None, // First event has no input (AWS pattern)
                                stop: Some(false), // Not the final event
                            }));
                        }
                    }
                    
                    // Handle done messages without tool calls
                    if ollama_response.done {
                        self.ended = true;
                        return Ok(None); // End of stream
                    }
                    
                    // Handle thinking content (skip)
                    if let Some(thinking) = &ollama_response.message.thinking {
                        if !thinking.is_empty() {
                            // Skip thinking content, continue loop instead of recursion
                            continue;
                        }
                    }
                    
                    // Handle regular content
                    if let Some(content) = &ollama_response.message.content {
                        if !content.is_empty() {
                            return Ok(Some(ChatResponseStream::AssistantResponseEvent {
                                content: content.clone(),
                            }));
                        }
                    }
                    
                    // Skip empty chunks, continue loop instead of recursion
                    continue;
                },
                Some(Err(e)) => return Err(ApiClientError::from(e)),
                None => {
                    self.ended = true;
                    return Ok(None);
                }
            }
        }
    }
}

// Implement the general StreamReceiver trait for extensibility
#[async_trait::async_trait]
impl crate::providers::StreamReceiver for OllamaStreamReceiver {
    async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        // Delegate to the existing recv implementation
        OllamaStreamReceiver::recv(self).await
    }
    
    fn metadata(&self) -> crate::providers::ResponseMetadata {
        crate::providers::ResponseMetadata::Ollama {
            model: "unknown".to_string(), // TODO: Track actual model
            total_duration: None,
            eval_count: None,
        }
    }
    
    fn is_ended(&self) -> bool {
        self.ended && self.pending_tool_events.is_empty()
    }
}

// Convert plugin OllamaError directly to ApiClientError
impl From<OllamaError> for ApiClientError {
    fn from(err: OllamaError) -> Self {
        match err {
            OllamaError::HttpError(e) => {
                // Create a generic error since we can't access the old OllamaError type
                ApiClientError::InvalidConfiguration(format!("Ollama HTTP error: {}", e))
            },
            OllamaError::ServerError { status, message } => {
                ApiClientError::InvalidConfiguration(format!("Ollama server error {}: {}", status, message))
            },
            OllamaError::ModelNotFound { model } => {
                ApiClientError::InvalidConfiguration(format!("Ollama model not found: {}", model))
            },
            OllamaError::ConnectionFailed { url } => {
                ApiClientError::InvalidConfiguration(format!("Ollama connection failed: {}", url))
            },
            OllamaError::InvalidResponse(msg) => {
                ApiClientError::InvalidConfiguration(format!("Ollama invalid response: {}", msg))
            },
        }
    }
}
