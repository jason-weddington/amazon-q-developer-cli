use async_trait::async_trait;
use eyre::{Result, bail};

use crate::api_client::model::{ConversationState, ChatResponseStream};
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
    
    /// Get available models for this provider
    async fn list_models(&self) -> Result<Vec<String>, ApiClientError>;
    
    /// Get context window size for a specific model
    async fn get_model_context_window(&self, model: &str) -> Result<Option<usize>, ApiClientError>;
}

/// Provider response wrapper that can be converted to SendMessageOutput
#[derive(Debug)]
pub enum ProviderResponse {
    /// Streaming response with async receiver
    Streaming(Box<dyn StreamReceiver>),
}

/// Generic trait for streaming responses from any provider
#[async_trait]
pub trait StreamReceiver: Send + Sync + std::fmt::Debug {
    /// Receive the next event from the stream
    async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError>;
}

impl From<ProviderResponse> for SendMessageOutput {
    fn from(response: ProviderResponse) -> Self {
        match response {
            ProviderResponse::Streaming(stream_receiver) => {
                // Use the new ProviderStreaming variant for proper integration
                SendMessageOutput::ProviderStreaming(stream_receiver)
            },
        }
    }
}

/// Environment validation for external providers
pub fn validate_provider_environment() -> Result<String> {
    let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
        .unwrap_or_else(|_| "aws".to_string())
        .to_lowercase();
    
    match provider.as_str() {
        "aws" => Ok(provider),
        "ollama" => {
            // Ollama doesn't require API key, just validate it's a known provider
            Ok(provider)
        },
        "openai" => {
            // Check for required API key
            if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using OpenAI provider");
            }
            Ok(provider)
        },
        "anthropic" => {
            // Check for required API key
            if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using Anthropic provider");
            }
            Ok(provider)
        },
        _ => bail!(
            "Invalid Q_CLI_MODEL_PROVIDER: '{}'. Valid values are: aws, ollama, openai, anthropic", 
            provider
        ),
    }
}

/// Check if the current provider requires authentication
pub fn current_provider_requires_auth() -> bool {
    let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
        .unwrap_or_else(|_| "aws".to_string())
        .to_lowercase();
    
    match provider.as_str() {
        "aws" => true,
        "ollama" => false,
        "openai" | "anthropic" => false, // They use API keys, not AWS auth
        _ => true, // Default to requiring auth for unknown providers
    }
}

// Re-export for convenience
pub use self::ollama::OllamaProvider;

// Provider modules
pub mod ollama;
