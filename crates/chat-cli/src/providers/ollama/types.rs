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
    pub model: String,
    pub created_at: String,
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
    stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send>>,
    ended: bool,
}

impl OllamaStreamReceiver {
    pub fn new(stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send>>) -> Self {
        Self {
            stream,
            ended: false,
        }
    }
    
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        if self.ended {
            return Ok(None);
        }
        
        match StreamExt::next(&mut self.stream).await {
            Some(Ok(ollama_response)) => {
                if ollama_response.done {
                    self.ended = true;
                    // Return None to signal end of stream (parser handles EndStream event)
                    Ok(None)
                } else if let Some(tool_calls) = &ollama_response.message.tool_calls {
                    // Convert tool calls to ToolUseEvent
                    if let Some(tool_call) = tool_calls.first() {
                        // Generate ID if not provided by Ollama
                        let tool_use_id = tool_call.id.clone()
                            .unwrap_or_else(|| format!("ollama_tool_{}", uuid::Uuid::new_v4()));
                        
                        // Convert arguments to JSON string
                        let arguments_str = match &tool_call.function.arguments {
                            serde_json::Value::String(s) => s.clone(),
                            other => serde_json::to_string(other).unwrap_or_default(),
                        };
                        
                        Ok(Some(ChatResponseStream::ToolUseEvent {
                            tool_use_id,
                            name: tool_call.function.name.clone(),
                            input: Some(arguments_str),
                            stop: Some(false),
                        }))
                    } else {
                        // Empty tool calls, skip
                        Box::pin(self.recv()).await
                    }
                } else if let Some(content) = &ollama_response.message.content {
                    if !content.is_empty() {
                        // Convert to ChatResponseStream::AssistantResponseEvent
                        Ok(Some(ChatResponseStream::AssistantResponseEvent {
                            content: content.clone(),
                        }))
                    } else {
                        // Skip empty content chunks, get next recursively
                        Box::pin(self.recv()).await
                    }
                } else {
                    // No content or tool calls, skip
                    Box::pin(self.recv()).await
                }
            },
            Some(Err(e)) => Err(ApiClientError::OllamaError(e.into())),
            None => {
                self.ended = true;
                Ok(None)
            }
        }
    }
}

// Convert plugin OllamaError to the core OllamaError type that ApiClientError expects
impl From<OllamaError> for crate::api_client::ollama::OllamaError {
    fn from(err: OllamaError) -> Self {
        match err {
            OllamaError::HttpError(e) => crate::api_client::ollama::OllamaError::HttpError(e),
            OllamaError::ServerError { status, message } => crate::api_client::ollama::OllamaError::ServerError { status, message },
            OllamaError::ModelNotFound { model } => crate::api_client::ollama::OllamaError::ModelNotFound { model },
            OllamaError::ConnectionFailed { url } => crate::api_client::ollama::OllamaError::ConnectionFailed { url },
            OllamaError::InvalidResponse(msg) => crate::api_client::ollama::OllamaError::InvalidResponse(msg),
        }
    }
}
