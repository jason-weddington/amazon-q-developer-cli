use reqwest;
use serde_json;
use tokio_stream::StreamExt;

use super::types::*;

/// Ollama HTTP client
#[derive(Clone, Debug)]
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
    
    /// Send a chat request to Ollama (non-streaming)
    pub async fn chat(&self, mut request: OllamaChatRequest) -> Result<OllamaChatResponse, OllamaError> {
        // Ensure non-streaming for non-streaming requests
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
    
    /// Send a streaming chat request to Ollama
    pub async fn chat_stream(&self, mut request: OllamaChatRequest) -> Result<OllamaStreamReceiver, OllamaError> {
        // Ensure streaming
        request.stream = Some(true);
        
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
            .map(|chunk_result| {
                let chunk = chunk_result.map_err(OllamaError::HttpError)?;
                let line = String::from_utf8(chunk.to_vec())
                    .map_err(|e| OllamaError::InvalidResponse(format!("Invalid UTF-8: {}", e)))?;
                
                // Skip empty lines
                if line.trim().is_empty() {
                    return Ok(None);
                }
                
                let ollama_response: OllamaChatResponse = serde_json::from_str(&line)
                    .map_err(|e| {
                        tracing::warn!("Failed to parse Ollama JSON: {} | Line: {}", e, line);
                        OllamaError::InvalidResponse(format!("Invalid JSON: {} | Line: {}", e, line))
                    })?;
                Ok(Some(ollama_response))
            })
            .filter_map(|result| {
                match result {
                    Ok(Some(response)) => Some(Ok(response)),
                    Ok(None) => None, // Skip empty lines
                    Err(e) => Some(Err(e)),
                }
            });
            
        Ok(OllamaStreamReceiver::new(Box::pin(stream)))
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
