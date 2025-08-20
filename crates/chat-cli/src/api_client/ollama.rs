use reqwest;
use serde::{Deserialize, Serialize};
use eyre::Result;
use thiserror::Error;
use futures::{Stream, TryStreamExt};
use std::pin::Pin;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::api_client::model::ChatResponseStream;
use crate::api_client::ApiClientError;

#[derive(Debug, Error)]
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

pub struct OllamaStreamReceiver {
    stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send>>,
    message_id: String,
    ended: bool,
    buffer: String, // Buffer for incomplete lines
}

impl std::fmt::Debug for OllamaStreamReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaStreamReceiver")
            .field("message_id", &self.message_id)
            .field("ended", &self.ended)
            .field("buffer_len", &self.buffer.len())
            .finish()
    }
}

impl OllamaStreamReceiver {
    pub fn new(stream: Pin<Box<dyn Stream<Item = Result<OllamaChatResponse, OllamaError>> + Send>>, message_id: String) -> Self {
        Self {
            stream,
            message_id,
            ended: false,
            buffer: String::new(),
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
            Some(Err(e)) => Err(e.into()),
            None => {
                self.ended = true;
                Ok(None)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    /// Create a new Ollama client with the specified base URL
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
    
    /// Send a chat request to Ollama (non-streaming)
    pub async fn chat(&self, mut request: OllamaChatRequest) -> Result<OllamaChatResponse, OllamaError> {
        // Ensure non-streaming for Task 2
        request.stream = Some(false);
        
        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(OllamaError::HttpError)?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let chat_response: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| OllamaError::InvalidResponse(e.to_string()))?;
            
        Ok(chat_response)
    }
    
    /// Send a chat request to Ollama (streaming)
    pub async fn chat_stream(&self, request: OllamaChatRequest) -> Result<OllamaStreamReceiver, OllamaError> {
        let mut request = request;
        request.stream = Some(true); // Force streaming
        
        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(OllamaError::HttpError)?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let stream = response
            .bytes_stream()
            .map(|chunk_result| -> Result<Option<OllamaChatResponse>, OllamaError> {
                let chunk = chunk_result.map_err(OllamaError::HttpError)?;
                let chunk_str = String::from_utf8(chunk.to_vec())
                    .map_err(|e| OllamaError::InvalidResponse(format!("Invalid UTF-8: {}", e)))?;
                
                // Process each line in the chunk
                for line in chunk_str.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    
                    // Try to parse JSON, but handle incomplete lines gracefully
                    match serde_json::from_str::<OllamaChatResponse>(line) {
                        Ok(ollama_response) => return Ok(Some(ollama_response)),
                        Err(e) => {
                            // Log the error but don't fail the stream for incomplete JSON
                            tracing::debug!("Skipping incomplete JSON line: {} (error: {})", line, e);
                            continue;
                        }
                    }
                }
                
                // No valid JSON found in this chunk
                Ok(None)
            })
            .try_filter_map(|opt_response| {
                futures::future::ready(Ok(opt_response))
            });
            
        let message_id = Uuid::new_v4().to_string();
        Ok(OllamaStreamReceiver::new(Box::pin(stream), message_id))
    }
    
    /// List available models
    pub async fn list_models(&self) -> Result<OllamaModelsResponse, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(OllamaError::HttpError)?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let models_response: OllamaModelsResponse = response
            .json()
            .await
            .map_err(|e| OllamaError::InvalidResponse(e.to_string()))?;
            
        Ok(models_response)
    }
    
    /// Get model capabilities from Ollama
    pub async fn get_model_capabilities(&self, model: &str) -> Result<Vec<String>, OllamaError> {
        let url = format!("{}/api/show", self.base_url);
        let request = serde_json::json!({
            "name": model
        });
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(OllamaError::HttpError)?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OllamaError::ServerError { status, message });
        }
        
        let model_info: OllamaModelInfo = response
            .json()
            .await
            .map_err(|e| OllamaError::InvalidResponse(e.to_string()))?;
            
        Ok(model_info.capabilities)
    }
    
    /// Check if model supports specific capability
    pub async fn supports_capability(&self, model: &str, capability: &str) -> Result<bool, OllamaError> {
        let capabilities = self.get_model_capabilities(model).await?;
        Ok(capabilities.contains(&capability.to_string()))
    }
    
    /// Health check - verify Ollama server is accessible
    pub async fn health_check(&self) -> Result<bool, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Get the base URL for this client
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ollama_chat_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "model": "llama3.2",
                "created_at": "2023-12-12T14:13:43.416799Z",
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "done": true,
                "total_duration": 5191566416,
                "load_duration": 2154458,
                "prompt_eval_count": 26,
                "prompt_eval_duration": 383809000,
                "eval_count": 298,
                "eval_duration": 4799921000
            }"#)
            .create_async()
            .await;
            
        let client = OllamaClient::new(server.url());
        let request = OllamaChatRequest {
            model: "llama3.2".to_string(),
            messages: vec![OllamaMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                images: None,
            }],
            stream: Some(false),
            format: None,
            options: None,
            keep_alive: None,
        };
        
        let response = client.chat(request).await.unwrap();
        assert_eq!(response.message.content, "Hello! How can I help you today?");
        assert!(response.done);
        assert_eq!(response.model, "llama3.2");
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_ollama_list_models() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "models": [
                    {
                        "name": "llama3.2:latest",
                        "model": "llama3.2:latest",
                        "modified_at": "2023-12-07T09:32:18.757212583-08:00",
                        "size": 2019393189,
                        "digest": "sha256:a80c4f17acd55265feec403c7aef86be0c25983ab279d83f3bcd3abbcb5b8b72"
                    }
                ]
            }"#)
            .create_async()
            .await;
            
        let client = OllamaClient::new(server.url());
        let response = client.list_models().await.unwrap();
        assert_eq!(response.models.len(), 1);
        assert_eq!(response.models[0].name, "llama3.2:latest");
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_ollama_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;
            
        let client = OllamaClient::new(server.url());
        let request = OllamaChatRequest {
            model: "nonexistent".to_string(),
            messages: vec![],
            stream: Some(false),
            format: None,
            options: None,
            keep_alive: None,
        };
        
        let result = client.chat(request).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            OllamaError::ServerError { status, .. } => assert_eq!(status, 500),
            _ => panic!("Expected ServerError"),
        }
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_health_check_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/tags")
            .with_status(200)
            .with_body(r#"{"models": []}"#)
            .create_async()
            .await;
            
        let client = OllamaClient::new(server.url());
        let healthy = client.health_check().await.unwrap();
        assert!(healthy);
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_health_check_failure() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/tags")
            .with_status(500)
            .create_async()
            .await;
            
        let client = OllamaClient::new(server.url());
        let healthy = client.health_check().await.unwrap();
        assert!(!healthy);
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_base_url_getter() {
        let base_url = "http://localhost:11434";
        let client = OllamaClient::new(base_url.to_string());
        assert_eq!(client.base_url(), base_url);
    }
    
    #[tokio::test]
    async fn test_stream_forced_to_false() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .match_body(mockito::Matcher::JsonString(r#"{"model":"test","messages":[],"stream":false}"#.to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "model": "test",
                "created_at": "2023-12-12T14:13:43.416799Z",
                "message": {"role": "assistant", "content": "test"},
                "done": true
            }"#)
            .create_async()
            .await;
            
        let client = OllamaClient::new(server.url());
        let request = OllamaChatRequest {
            model: "test".to_string(),
            messages: vec![],
            stream: Some(true), // This should be overridden to false
            format: None,
            options: None,
            keep_alive: None,
        };
        
        let _response = client.chat(request).await.unwrap();
        mock.assert_async().await;
        // The mock will fail if stream isn't set to false
    }
}
