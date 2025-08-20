use aws_types::request_id::RequestId;

use crate::api_client::ApiClientError;
use crate::api_client::model::ChatResponseStream;
use crate::api_client::ollama::{OllamaChatResponse, OllamaStreamReceiver};

#[derive(Debug)]
pub enum SendMessageOutput {
    Codewhisperer(
        amzn_codewhisperer_streaming_client::operation::generate_assistant_response::GenerateAssistantResponseOutput,
    ),
    QDeveloper(amzn_qdeveloper_streaming_client::operation::send_message::SendMessageOutput),
    Mock(Vec<ChatResponseStream>),
    Ollama(OllamaChatResponse),
    OllamaStreaming(OllamaStreamReceiver),
}

#[derive(Debug)]
pub enum ResponseMetadata {
    Aws {
        request_id: Option<String>,
    },
    Ollama {
        model: String,
        created_at: String,
        total_duration: Option<u64>,
        eval_count: Option<u32>,
        eval_duration: Option<u64>,
    },
}

impl SendMessageOutput {
    pub fn request_id(&self) -> Option<&str> {
        match self {
            SendMessageOutput::Codewhisperer(output) => output.request_id(),
            SendMessageOutput::QDeveloper(output) => output.request_id(),
            SendMessageOutput::Mock(_) => None,
            SendMessageOutput::Ollama(_) => None, // Ollama doesn't have AWS-style request IDs
            SendMessageOutput::OllamaStreaming(_) => None, // Ollama doesn't have AWS-style request IDs
        }
    }

    /// Get the message content from any provider response
    pub fn content(&self) -> Option<&str> {
        match self {
            SendMessageOutput::Codewhisperer(_) => {
                // AWS streaming - content comes through recv()
                None
            },
            SendMessageOutput::QDeveloper(_) => {
                // AWS streaming - content comes through recv()
                None
            },
            SendMessageOutput::Mock(_) => {
                // Mock - content comes through recv()
                None
            },
            SendMessageOutput::Ollama(response) => {
                response.message.content.as_deref()
            },
            SendMessageOutput::OllamaStreaming(_) => {
                // Ollama streaming - content comes through recv()
                None
            },
        }
    }
    
    /// Get the message role from any provider response
    pub fn role(&self) -> Option<&str> {
        match self {
            SendMessageOutput::Codewhisperer(_) => {
                // AWS streaming - role comes through recv()
                None
            },
            SendMessageOutput::QDeveloper(_) => {
                // AWS streaming - role comes through recv()
                None
            },
            SendMessageOutput::Mock(_) => {
                // Mock - role comes through recv()
                None
            },
            SendMessageOutput::Ollama(response) => {
                Some(&response.message.role)
            },
            SendMessageOutput::OllamaStreaming(_) => {
                // Ollama streaming - role comes through recv()
                None
            },
        }
    }
    
    /// Check if the response is complete
    pub fn is_done(&self) -> bool {
        match self {
            SendMessageOutput::Codewhisperer(_) => {
                // AWS streaming - completion determined by recv() returning None
                false
            },
            SendMessageOutput::QDeveloper(_) => {
                // AWS streaming - completion determined by recv() returning None
                false
            },
            SendMessageOutput::Mock(vec) => {
                vec.is_empty()
            },
            SendMessageOutput::Ollama(response) => {
                response.done
            },
            SendMessageOutput::OllamaStreaming(_) => {
                // Ollama streaming - completion determined by recv() returning None
                false
            },
        }
    }
    
    /// Get provider-specific metadata
    pub fn metadata(&self) -> ResponseMetadata {
        match self {
            SendMessageOutput::Codewhisperer(output) => {
                ResponseMetadata::Aws {
                    request_id: output.request_id().map(|s| s.to_string()),
                }
            },
            SendMessageOutput::QDeveloper(output) => {
                ResponseMetadata::Aws {
                    request_id: output.request_id().map(|s| s.to_string()),
                }
            },
            SendMessageOutput::Mock(_) => {
                ResponseMetadata::Aws {
                    request_id: Some("<mock-request-id>".to_string()),
                }
            },
            SendMessageOutput::Ollama(response) => {
                ResponseMetadata::Ollama {
                    model: response.model.clone(),
                    created_at: response.created_at.clone(),
                    total_duration: response.total_duration,
                    eval_count: response.eval_count,
                    eval_duration: response.eval_duration,
                }
            },
            SendMessageOutput::OllamaStreaming(_) => {
                // For streaming, we don't have the final metadata yet
                ResponseMetadata::Ollama {
                    model: "unknown".to_string(), // Will be updated when stream completes
                    created_at: "unknown".to_string(),
                    total_duration: None,
                    eval_count: None,
                    eval_duration: None,
                }
            },
        }
    }

    /// Create from Ollama response
    pub fn from_ollama(response: OllamaChatResponse) -> Self {
        Self::Ollama(response)
    }

    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        match self {
            SendMessageOutput::Codewhisperer(output) => Ok(output
                .generate_assistant_response_response
                .recv()
                .await?
                .map(|s| s.into())),
            SendMessageOutput::QDeveloper(output) => Ok(output.send_message_response.recv().await?.map(|s| s.into())),
            SendMessageOutput::Mock(vec) => Ok(vec.pop()),
            SendMessageOutput::Ollama(_) => {
                // Ollama responses are non-streaming in Task 3
                // Return None to indicate no more streaming data
                Ok(None)
            },
            SendMessageOutput::OllamaStreaming(receiver) => {
                // New streaming Ollama implementation
                receiver.recv().await
            },
        }
    }
}

impl RequestId for SendMessageOutput {
    fn request_id(&self) -> Option<&str> {
        match self {
            SendMessageOutput::Codewhisperer(output) => output.request_id(),
            SendMessageOutput::QDeveloper(output) => output.request_id(),
            SendMessageOutput::Mock(_) => Some("<mock-request-id>"),
            SendMessageOutput::Ollama(_) => None, // Ollama doesn't use AWS request IDs
            SendMessageOutput::OllamaStreaming(_) => None, // Ollama doesn't use AWS request IDs
        }
    }
}

impl From<OllamaChatResponse> for SendMessageOutput {
    fn from(response: OllamaChatResponse) -> Self {
        Self::Ollama(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_client::ollama::OllamaMessage;
    
    fn create_test_ollama_response() -> OllamaChatResponse {
        OllamaChatResponse {
            model: "llama3.2".to_string(),
            created_at: "2023-12-12T14:13:43.416799Z".to_string(),
            message: OllamaMessage {
                role: "assistant".to_string(),
                content: "Hello! How can I help you?".to_string(),
                images: None,
            },
            done: true,
            done_reason: Some("stop".to_string()),
            total_duration: Some(5191566416),
            load_duration: Some(2154458),
            prompt_eval_count: Some(26),
            prompt_eval_duration: Some(383809000),
            eval_count: Some(298),
            eval_duration: Some(4799921000),
        }
    }
    
    #[test]
    fn test_ollama_variant_creation() {
        let ollama_response = create_test_ollama_response();
        let output = SendMessageOutput::from_ollama(ollama_response);
        
        assert_eq!(output.content(), Some("Hello! How can I help you?"));
        assert_eq!(output.role(), Some("assistant"));
        assert!(output.is_done());
    }
    
    #[test]
    fn test_ollama_metadata() {
        let ollama_response = create_test_ollama_response();
        let output = SendMessageOutput::from_ollama(ollama_response);
        
        match output.metadata() {
            ResponseMetadata::Ollama { model, eval_count, .. } => {
                assert_eq!(model, "llama3.2");
                assert_eq!(eval_count, Some(298));
            },
            _ => panic!("Expected Ollama metadata"),
        }
    }
    
    #[test]
    fn test_from_trait() {
        let ollama_response = create_test_ollama_response();
        let output: SendMessageOutput = ollama_response.into();
        
        assert!(matches!(output, SendMessageOutput::Ollama(_)));
        assert_eq!(output.content(), Some("Hello! How can I help you?"));
    }
    
    #[test]
    fn test_ollama_request_id() {
        let ollama_response = create_test_ollama_response();
        let output = SendMessageOutput::from_ollama(ollama_response);
        
        // Ollama doesn't have AWS-style request IDs
        assert_eq!(output.request_id(), None);
    }
    
    #[test]
    fn test_ollama_recv_returns_none() {
        let ollama_response = create_test_ollama_response();
        let mut output = SendMessageOutput::from_ollama(ollama_response);
        
        // Ollama is non-streaming in Task 3, so recv should return None
        let result = futures::executor::block_on(output.recv()).unwrap();
        assert!(result.is_none());
    }
}
