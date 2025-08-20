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
            ProviderResponse::OllamaStreaming(receiver) => {
                // TODO: Convert plugin stream receiver to core stream receiver
                // For now, this will cause compilation errors - we'll fix in Step 4
                SendMessageOutput::OllamaStreaming(receiver.into())
            },
            ProviderResponse::Ollama(response) => {
                // TODO: Convert plugin response to core response
                // For now, this will cause compilation errors - we'll fix in Step 4
                SendMessageOutput::Ollama(response.into())
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
