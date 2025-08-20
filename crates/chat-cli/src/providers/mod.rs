use async_trait::async_trait;
use eyre::Result;

use crate::api_client::model::ConversationState;
use crate::api_client::send_message_output::SendMessageOutput;
use crate::api_client::ApiClientError;

/// Trait for external model providers (Ollama, OpenAI, Anthropic, etc.)
#[async_trait]
pub trait MessageProvider: Send + Sync {
    /// Send a message and get a response
    async fn send_message(&self, conversation: ConversationState) -> Result<ProviderResponse, ApiClientError>;
    
    /// Get the provider name for logging/debugging
    fn provider_name(&self) -> &'static str;
    
    /// Whether this provider requires authentication
    fn requires_auth(&self) -> bool;
    
    /// Whether this provider supports streaming responses
    fn supports_streaming(&self) -> bool;
    
    /// Whether this provider supports tool calling
    fn supports_tools(&self) -> bool;
    
    /// Get available models for this provider
    async fn list_models(&self) -> Result<Vec<String>, ApiClientError>;
    
    /// Test connection to the provider
    async fn test_connection(&self) -> Result<bool, ApiClientError>;
}

/// Provider response wrapper that can be converted to SendMessageOutput
pub enum ProviderResponse {
    /// Ollama streaming response
    OllamaStreaming(crate::providers::ollama::OllamaStreamReceiver),
    /// Ollama non-streaming response  
    Ollama(crate::providers::ollama::OllamaChatResponse),
    // Future: OpenAI, Anthropic responses
}

impl From<ProviderResponse> for SendMessageOutput {
    fn from(response: ProviderResponse) -> Self {
        match response {
            ProviderResponse::OllamaStreaming(_receiver) => {
                // Create a bridge that converts plugin streaming to core streaming
                // We'll create a mock stream that pulls from the plugin receiver
                use crate::api_client::model::ChatResponseStream;
                
                // For now, let's create a simple mock that shows we're getting the plugin response
                // In Step 4, we'll properly integrate this with the core streaming system
                let mock_content = vec![
                    ChatResponseStream::AssistantResponseEvent {
                        content: "Ollama plugin is working! (Streaming response bridge active)".to_string(),
                    }
                ];
                SendMessageOutput::Mock(mock_content)
            },
            ProviderResponse::Ollama(response) => {
                // Convert plugin response to core response
                use crate::api_client::model::ChatResponseStream;
                let content = response.message.content.unwrap_or_else(|| "No content from Ollama".to_string());
                let mock_content = vec![
                    ChatResponseStream::AssistantResponseEvent {
                        content,
                    }
                ];
                SendMessageOutput::Mock(mock_content)
            },
        }
    }
}

/// Registry for managing external providers
pub struct ProviderRegistry {
    providers: std::collections::HashMap<String, Box<dyn MessageProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: std::collections::HashMap::new(),
        }
    }
    
    /// Register a provider
    pub fn register<P: MessageProvider + 'static>(&mut self, name: String, provider: P) {
        self.providers.insert(name, Box::new(provider));
    }
    
    /// Get a provider by name
    pub fn get(&self, name: &str) -> Option<&dyn MessageProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }
    
    /// List all registered providers
    pub fn list_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export for convenience
pub use self::ollama::OllamaProvider;

// Provider modules
pub mod ollama;
