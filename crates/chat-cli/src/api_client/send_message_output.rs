use aws_types::request_id::RequestId;

use crate::api_client::ApiClientError;
use crate::api_client::model::ChatResponseStream;

#[derive(Debug)]
pub enum SendMessageOutput {
    Codewhisperer(
        amzn_codewhisperer_streaming_client::operation::generate_assistant_response::GenerateAssistantResponseOutput,
    ),
    QDeveloper(amzn_qdeveloper_streaming_client::operation::send_message::SendMessageOutput),
    Mock(Vec<ChatResponseStream>),
    /// Generic provider streaming (Ollama, OpenAI, Anthropic, etc.)
    ProviderStreaming(Box<dyn crate::providers::StreamReceiver>),
}

impl SendMessageOutput {
    pub async fn recv(&mut self) -> Result<Option<ChatResponseStream>, ApiClientError> {
        match self {
            SendMessageOutput::Codewhisperer(output) => {
                Ok(output.generate_assistant_response_response.recv().await?.map(|s| s.into()))
            },
            SendMessageOutput::QDeveloper(output) => {
                Ok(output.send_message_response.recv().await?.map(|s| s.into()))
            },
            SendMessageOutput::Mock(vec) => Ok(vec.pop()),
            SendMessageOutput::ProviderStreaming(stream_receiver) => {
                // Delegate to the generic StreamReceiver
                stream_receiver.recv().await
            },
        }
    }

    pub fn request_id(&self) -> Option<String> {
        match self {
            SendMessageOutput::Codewhisperer(output) => output.request_id().map(|r| r.to_string()),
            SendMessageOutput::QDeveloper(output) => output.request_id().map(|r| r.to_string()),
            SendMessageOutput::Mock(_) => None,
            SendMessageOutput::ProviderStreaming(_) => None, // Provider streaming doesn't have AWS request IDs
        }
    }
}
