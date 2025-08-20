mod credentials;
pub mod customization;
mod delay_interceptor;
mod endpoints;
mod error;
pub mod model;
mod ollama;
mod opt_out;
pub mod profile;
mod retry_classifier;
pub mod send_message_output;
use std::sync::Arc;
use std::time::Duration;

use amzn_codewhisperer_client::Client as CodewhispererClient;
use amzn_codewhisperer_client::operation::create_subscription_token::CreateSubscriptionTokenOutput;
use amzn_codewhisperer_client::types::Origin::Cli;
use amzn_codewhisperer_client::types::{
    Model,
    OptInFeatureToggle,
    OptOutPreference,
    SubscriptionStatus,
    TelemetryEvent,
    UserContext,
};
use amzn_codewhisperer_streaming_client::Client as CodewhispererStreamingClient;
use amzn_qdeveloper_streaming_client::Client as QDeveloperStreamingClient;
use amzn_qdeveloper_streaming_client::types::Origin;
use aws_config::retry::RetryConfig;
use aws_config::timeout::TimeoutConfig;
use aws_credential_types::Credentials;
use aws_credential_types::provider::ProvideCredentials;
use aws_types::request_id::RequestId;
use aws_types::sdk_config::StalledStreamProtectionConfig;
pub use endpoints::Endpoint;
pub use error::ApiClientError;
use parking_lot::Mutex;
pub use profile::list_available_profiles;
pub use ollama::{
    OllamaClient, 
    OllamaMessage, 
    OllamaChatRequest,
    OllamaTool,
    OllamaFunction,
};
pub use send_message_output::SendMessageOutput;
use serde_json::Map;
use tokio::sync::RwLock;
use tracing::{
    debug,
    error,
    warn,
};

use crate::api_client::credentials::CredentialsChain;
use crate::api_client::delay_interceptor::DelayTrackingInterceptor;
use crate::api_client::model::{
    ChatMessage,
    ChatResponseStream,
    ConversationState,
    ImageBlock,
    ImageFormat,
    ImageSource,
};
use crate::api_client::opt_out::OptOutInterceptor;
use crate::auth::builder_id::BearerResolver;
use crate::aws_common::{
    UserAgentOverrideInterceptor,
    app_name,
    behavior_version,
};
use crate::database::settings::Setting;
use crate::database::{
    AuthProfile,
    Database,
};
use crate::os::{
    Env,
    Fs,
};
use crate::util::ModelProvider;

// Opt out constants
pub const X_AMZN_CODEWHISPERER_OPT_OUT_HEADER: &str = "x-amzn-codewhisperer-optout";

// TODO(bskiser): confirm timeout is updated to an appropriate value?
const DEFAULT_TIMEOUT_DURATION: Duration = Duration::from_secs(60 * 5);

#[derive(Clone, Debug)]
pub struct ModelListResult {
    pub models: Vec<Model>,
    pub default_model: Model,
}

impl From<ModelListResult> for (Vec<Model>, Model) {
    fn from(v: ModelListResult) -> Self {
        (v.models, v.default_model)
    }
}

type ModelCache = Arc<RwLock<Option<ModelListResult>>>;

#[derive(Clone, Debug)]
pub struct ApiClient {
    client: CodewhispererClient,
    streaming_client: Option<CodewhispererStreamingClient>,
    sigv4_streaming_client: Option<QDeveloperStreamingClient>,
    ollama_client: Option<OllamaClient>,
    mock_client: Option<Arc<Mutex<std::vec::IntoIter<Vec<ChatResponseStream>>>>>,
    profile: Option<AuthProfile>,
    model_cache: ModelCache,
    provider: ModelProvider,
}

impl ApiClient {
    pub async fn new(
        env: &Env,
        fs: &Fs,
        database: &mut Database,
        // endpoint is only passed here for list_profiles where it needs to be called for each region
        endpoint: Option<Endpoint>,
    ) -> Result<Self, ApiClientError> {
        let provider = ModelProvider::from_env()
            .map_err(|e| ApiClientError::InvalidConfiguration(e.to_string()))?;
        
        // For now, only AWS is implemented
        match provider {
            ModelProvider::Aws => {
                // Continue with existing AWS client creation logic
            },
            ModelProvider::Ollama => {
                let base_url = env.get("Q_CLI_MODEL_PROVIDER_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string());
                
                let ollama_client = OllamaClient::new(base_url);
                
                // Test connection during initialization (don't fail if it doesn't work)
                if let Err(e) = ollama_client.health_check().await {
                    warn!("Ollama health check failed: {}", e);
                }
                
                // We still need to create a minimal AWS client for compatibility
                // Use the same endpoint handling as the main AWS case
                let endpoint = endpoint.unwrap_or(Endpoint::configured_value(database));
                
                let credentials = Credentials::new("dummy", "dummy", None, None, "dummy");
                let bearer_sdk_config = aws_config::defaults(behavior_version())
                    .region(endpoint.region.clone())
                    .credentials_provider(credentials)
                    .timeout_config(timeout_config(database))
                    .retry_config(retry_config())
                    .load()
                    .await;
                
                let client = CodewhispererClient::from_conf(
                    amzn_codewhisperer_client::config::Builder::from(&bearer_sdk_config)
                        .http_client(crate::aws_common::http_client::client())
                        .interceptor(OptOutInterceptor::new(database))
                        .interceptor(UserAgentOverrideInterceptor::new())
                        .interceptor(DelayTrackingInterceptor::new())
                        .bearer_token_resolver(BearerResolver)
                        .app_name(app_name())
                        .endpoint_url(endpoint.url())
                        .retry_classifier(retry_classifier::QCliRetryClassifier::new())
                        .stalled_stream_protection(stalled_stream_protection_config())
                        .build(),
                );
                
                return Ok(Self {
                    client,
                    streaming_client: None,
                    sigv4_streaming_client: None,
                    ollama_client: Some(ollama_client),
                    mock_client: None,
                    profile: None,
                    model_cache: Arc::new(RwLock::new(None)),
                    provider,
                });
            },
            ModelProvider::OpenAi | ModelProvider::Anthropic => {
                return Err(ApiClientError::UnsupportedProvider(format!("{}", provider)));
            },
        }

        let endpoint = endpoint.unwrap_or(Endpoint::configured_value(database));

        let credentials = Credentials::new("xxx", "xxx", None, None, "xxx");
        let bearer_sdk_config = aws_config::defaults(behavior_version())
            .region(endpoint.region.clone())
            .credentials_provider(credentials)
            .timeout_config(timeout_config(database))
            .retry_config(retry_config())
            .load()
            .await;

        let client = CodewhispererClient::from_conf(
            amzn_codewhisperer_client::config::Builder::from(&bearer_sdk_config)
                .http_client(crate::aws_common::http_client::client())
                .interceptor(OptOutInterceptor::new(database))
                .interceptor(UserAgentOverrideInterceptor::new())
                .bearer_token_resolver(BearerResolver)
                .app_name(app_name())
                .endpoint_url(endpoint.url())
                .build(),
        );

        if cfg!(test) {
            let mut this = Self {
                client,
                streaming_client: None,
                sigv4_streaming_client: None,
                ollama_client: None,
                mock_client: None,
                profile: None,
                model_cache: Arc::new(RwLock::new(None)),
                provider,
            };

            if let Ok(json) = env.get("Q_MOCK_CHAT_RESPONSE") {
                this.set_mock_output(serde_json::from_str(fs.read_to_string(json).await.unwrap().as_str()).unwrap());
            }

            return Ok(this);
        }

        // If SIGV4_AUTH_ENABLED is true, use Q developer client
        let mut streaming_client = None;
        let mut sigv4_streaming_client = None;
        match env.get("AMAZON_Q_SIGV4").is_ok() {
            true => {
                let credentials_chain = CredentialsChain::new().await;
                if let Err(err) = credentials_chain.provide_credentials().await {
                    return Err(ApiClientError::Credentials(err));
                };

                sigv4_streaming_client = Some(QDeveloperStreamingClient::from_conf(
                    amzn_qdeveloper_streaming_client::config::Builder::from(
                        &aws_config::defaults(behavior_version())
                            .region(endpoint.region.clone())
                            .credentials_provider(credentials_chain)
                            .timeout_config(timeout_config(database))
                            .retry_config(retry_config())
                            .load()
                            .await,
                    )
                    .http_client(crate::aws_common::http_client::client())
                    .interceptor(OptOutInterceptor::new(database))
                    .interceptor(UserAgentOverrideInterceptor::new())
                    .interceptor(DelayTrackingInterceptor::new())
                    .app_name(app_name())
                    .endpoint_url(endpoint.url())
                    .retry_classifier(retry_classifier::QCliRetryClassifier::new())
                    .stalled_stream_protection(stalled_stream_protection_config())
                    .build(),
                ));
            },
            false => {
                streaming_client = Some(CodewhispererStreamingClient::from_conf(
                    amzn_codewhisperer_streaming_client::config::Builder::from(&bearer_sdk_config)
                        .http_client(crate::aws_common::http_client::client())
                        .interceptor(OptOutInterceptor::new(database))
                        .interceptor(UserAgentOverrideInterceptor::new())
                        .interceptor(DelayTrackingInterceptor::new())
                        .bearer_token_resolver(BearerResolver)
                        .app_name(app_name())
                        .endpoint_url(endpoint.url())
                        .retry_classifier(retry_classifier::QCliRetryClassifier::new())
                        .stalled_stream_protection(stalled_stream_protection_config())
                        .build(),
                ));
            },
        }

        let profile = match database.get_auth_profile() {
            Ok(profile) => profile,
            Err(err) => {
                error!("Failed to get auth profile: {err}");
                None
            },
        };

        Ok(Self {
            client,
            streaming_client,
            sigv4_streaming_client,
            ollama_client: None,
            mock_client: None,
            profile,
            model_cache: Arc::new(RwLock::new(None)),
            provider,
        })
    }

    /// Get the current model provider
    pub fn provider(&self) -> &ModelProvider {
        &self.provider
    }

    /// Test Ollama connection
    pub async fn test_ollama_connection(&self) -> Result<bool, ApiClientError> {
        match &self.ollama_client {
            Some(client) => Ok(client.health_check().await?),
            None => Ok(false),
        }
    }
    
    /// Get Ollama client reference
    pub fn ollama_client(&self) -> Option<&OllamaClient> {
        self.ollama_client.as_ref()
    }
    
    /// List available Ollama models
    pub async fn list_ollama_models(&self) -> Result<Vec<String>, ApiClientError> {
        match &self.ollama_client {
            Some(client) => {
                let response = client.list_models().await?;
                Ok(response.models.into_iter().map(|m| m.name).collect())
            },
            None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
        }
    }

    /// Send a message using Ollama provider
    pub async fn send_message_ollama(&self, messages: Vec<OllamaMessage>, model: &str) -> Result<SendMessageOutput, ApiClientError> {
        match &self.ollama_client {
            Some(client) => {
                let request = OllamaChatRequest {
                    model: model.to_string(),
                    messages,
                    tools: None, // No tools for this simple method
                    stream: Some(false), // Non-streaming for Task 3
                    format: None,
                    options: None,
                    keep_alive: None,
                };
                
                let response = client.chat(request).await?;
                Ok(SendMessageOutput::from_ollama(response))
            },
            None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
        }
    }

    pub async fn send_telemetry_event(
        &self,
        telemetry_event: TelemetryEvent,
        user_context: UserContext,
        telemetry_enabled: bool,
        model: Option<String>,
    ) -> Result<(), ApiClientError> {
        if cfg!(test) {
            return Ok(());
        }

        self.client
            .send_telemetry_event()
            .telemetry_event(telemetry_event)
            .user_context(user_context)
            .opt_out_preference(match telemetry_enabled {
                true => OptOutPreference::OptIn,
                false => OptOutPreference::OptOut,
            })
            .set_profile_arn(self.profile.as_ref().map(|p| p.arn.clone()))
            .set_model_id(model)
            .send()
            .await?;

        Ok(())
    }

    pub async fn list_available_profiles(&self) -> Result<Vec<AuthProfile>, ApiClientError> {
        if cfg!(test) {
            return Ok(vec![
                AuthProfile {
                    arn: "my:arn:1".to_owned(),
                    profile_name: "MyProfile".to_owned(),
                },
                AuthProfile {
                    arn: "my:arn:2".to_owned(),
                    profile_name: "MyOtherProfile".to_owned(),
                },
            ]);
        }

        let mut profiles = vec![];
        let mut stream = self.client.list_available_profiles().into_paginator().send();
        while let Some(profiles_output) = stream.next().await {
            profiles.extend(profiles_output?.profiles().iter().cloned().map(AuthProfile::from));
        }

        Ok(profiles)
    }

    pub async fn list_available_models(&self) -> Result<ModelListResult, ApiClientError> {
        if cfg!(test) {
            let m = Model::builder()
                .model_id("model-1")
                .description("Test Model 1")
                .build()
                .unwrap();

            return Ok(ModelListResult {
                models: vec![m.clone()],
                default_model: m,
            });
        }

        let mut models = Vec::new();
        let mut default_model = None;
        let request = self
            .client
            .list_available_models()
            .set_origin(Some(Cli))
            .set_profile_arn(self.profile.as_ref().map(|p| p.arn.clone()));
        let mut paginator = request.into_paginator().send();

        while let Some(result) = paginator.next().await {
            let models_output = result?;
            models.extend(models_output.models().iter().cloned());

            if default_model.is_none() {
                default_model = Some(models_output.default_model().clone());
            }
        }
        let default_model = default_model.ok_or_else(|| ApiClientError::DefaultModelNotFound)?;
        Ok(ModelListResult { models, default_model })
    }

    pub async fn list_available_models_cached(&self) -> Result<ModelListResult, ApiClientError> {
        {
            let cache = self.model_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                tracing::debug!("Returning cached model list");
                return Ok(cached.clone());
            }
        }

        tracing::debug!("Cache miss, fetching models from list_available_models API");
        let result = self.list_available_models().await?;
        {
            let mut cache = self.model_cache.write().await;
            *cache = Some(result.clone());
        }
        Ok(result)
    }

    pub async fn invalidate_model_cache(&self) {
        let mut cache = self.model_cache.write().await;
        *cache = None;
        tracing::info!("Model cache invalidated");
    }

    pub async fn get_available_models(&self, _region: &str) -> Result<ModelListResult, ApiClientError> {
        let res = self.list_available_models_cached().await?;
        // TODO: Once we have access to gpt-oss, add back.
        // if region == "us-east-1" {
        //     let gpt_oss = Model::builder()
        //         .model_id("OPENAI_GPT_OSS_120B_1_0")
        //         .model_name("openai-gpt-oss-120b-preview")
        //         .token_limits(TokenLimits::builder().max_input_tokens(128_000).build())
        //         .build()
        //         .map_err(ApiClientError::from)?;

        //     models.push(gpt_oss);
        // }

        Ok(res)
    }

    pub async fn is_mcp_enabled(&self) -> Result<bool, ApiClientError> {
        let request = self
            .client
            .get_profile()
            .set_profile_arn(self.profile.as_ref().map(|p| p.arn.clone()));

        let response = request.send().await?;
        let mcp_enabled = response
            .profile()
            .opt_in_features()
            .and_then(|features| features.mcp_configuration())
            .is_none_or(|config| matches!(config.toggle(), OptInFeatureToggle::On));
        Ok(mcp_enabled)
    }

    pub async fn create_subscription_token(&self) -> Result<CreateSubscriptionTokenOutput, ApiClientError> {
        if cfg!(test) {
            return Ok(CreateSubscriptionTokenOutput::builder()
                .set_encoded_verification_url(Some("test/url".to_string()))
                .set_status(Some(SubscriptionStatus::Inactive))
                .set_token(Some("test-token".to_string()))
                .build()?);
        }

        self.client
            .create_subscription_token()
            .send()
            .await
            .map_err(ApiClientError::CreateSubscriptionToken)
    }

    pub async fn send_message(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
        debug!("Sending conversation: {:#?}", conversation);

        // NEW: Route based on provider
        match self.provider {
            ModelProvider::Ollama => {
                return self.send_message_ollama_internal(conversation).await;
            },
            ModelProvider::Aws => {
                // Continue with existing AWS logic below
            },
            ModelProvider::OpenAi | ModelProvider::Anthropic => {
                return Err(ApiClientError::UnsupportedProvider(format!("{:?}", self.provider)));
            },
        }

        let ConversationState {
            conversation_id,
            user_input_message,
            history,
        } = conversation;

        let model_id_opt: Option<String> = user_input_message.model_id.clone();

        if let Some(client) = &self.streaming_client {
            let conversation_state = amzn_codewhisperer_streaming_client::types::ConversationState::builder()
                .set_conversation_id(conversation_id)
                .current_message(
                    amzn_codewhisperer_streaming_client::types::ChatMessage::UserInputMessage(
                        user_input_message.into(),
                    ),
                )
                .chat_trigger_type(amzn_codewhisperer_streaming_client::types::ChatTriggerType::Manual)
                .set_history(
                    history
                        .map(|v| v.into_iter().map(|i| i.try_into()).collect::<Result<Vec<_>, _>>())
                        .transpose()?,
                )
                .build()
                .expect("building conversation should not fail");

            match client
                .generate_assistant_response()
                .conversation_state(conversation_state)
                .set_profile_arn(self.profile.as_ref().map(|p| p.arn.clone()))
                .send()
                .await
            {
                Ok(response) => Ok(SendMessageOutput::Codewhisperer(response)),
                Err(err) => {
                    let status_code = err.raw_response().map(|res| res.status().as_u16());
                    let is_quota_breach = status_code.is_some_and(|status| status == 429);
                    let is_context_window_overflow = err.as_service_error().is_some_and(|err| {
                        matches!(err, err if err.meta().code() == Some("ValidationException") && err.meta().message() == Some("Input is too long."))
                    });

                    let is_model_unavailable = {
                        // check if ThrottlingException
                        let is_throttling_exception = err
                            .as_service_error()
                            .is_some_and(|service_err| service_err.meta().code() == Some("ThrottlingException"));

                        // check if the response contains INSUFFICIENT_MODEL_CAPACITY
                        let has_insufficient_capacity = err
                            .raw_response()
                            .and_then(|resp| resp.body().bytes())
                            .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
                            .is_some_and(|body| body.contains("INSUFFICIENT_MODEL_CAPACITY"));

                        (is_throttling_exception && has_insufficient_capacity)
                        // Legacy error response fallback
                        || (model_id_opt.is_some()
                        && status_code.is_some_and(|status| status == 500)
                        && err.as_service_error().is_some_and(|err| {
                            err.meta().message() == Some(
                    "Encountered unexpectedly high load when processing the request, please try again.",
                )}))
                    };

                    let is_monthly_limit_err = err
                        .raw_response()
                        .and_then(|resp| resp.body().bytes())
                        .and_then(|bytes| match String::from_utf8(bytes.to_vec()) {
                            Ok(s) => Some(s.contains("MONTHLY_REQUEST_COUNT")),
                            Err(_) => None,
                        })
                        .unwrap_or(false);

                    if is_context_window_overflow {
                        return Err(ApiClientError::ContextWindowOverflow { status_code });
                    }

                    // Both ModelOverloadedError and QuotaBreach return 429,
                    // so check is_model_unavailable first.
                    if is_model_unavailable {
                        return Err(ApiClientError::ModelOverloadedError {
                            request_id: err
                                .as_service_error()
                                .and_then(|err| err.meta().request_id())
                                .map(|s| s.to_string()),
                            status_code,
                        });
                    }

                    if is_quota_breach {
                        return Err(ApiClientError::QuotaBreach {
                            message: "quota has reached its limit",
                            status_code,
                        });
                    }

                    if is_monthly_limit_err {
                        return Err(ApiClientError::MonthlyLimitReached { status_code });
                    }

                    Err(err.into())
                },
            }
        } else if let Some(client) = &self.sigv4_streaming_client {
            let conversation_state = amzn_qdeveloper_streaming_client::types::ConversationState::builder()
                .set_conversation_id(conversation_id)
                .current_message(amzn_qdeveloper_streaming_client::types::ChatMessage::UserInputMessage(
                    user_input_message.into(),
                ))
                .chat_trigger_type(amzn_qdeveloper_streaming_client::types::ChatTriggerType::Manual)
                .set_history(
                    history
                        .map(|v| v.into_iter().map(|i| i.try_into()).collect::<Result<Vec<_>, _>>())
                        .transpose()?,
                )
                .build()
                .expect("building conversation_state should not fail");

            match client
                .send_message()
                .conversation_state(conversation_state)
                .set_source(Some(Origin::from("CLI")))
                .send()
                .await
            {
                Ok(response) => Ok(SendMessageOutput::QDeveloper(response)),
                Err(err) => {
                    let status_code = err.raw_response().map(|res| res.status().as_u16());
                    let is_quota_breach = status_code.is_some_and(|status| status == 429);
                    let is_context_window_overflow = err.as_service_error().is_some_and(|err| {
                        matches!(err, err if err.meta().code() == Some("ValidationException") && err.meta().message() == Some("Input is too long."))
                    });

                    let is_model_unavailable = {
                        // check if ThrottlingException
                        let is_throttling_exception = err
                            .as_service_error()
                            .is_some_and(|service_err| service_err.meta().code() == Some("ThrottlingException"));

                        // check if the response contains INSUFFICIENT_MODEL_CAPACITY
                        let has_insufficient_capacity = err
                            .raw_response()
                            .and_then(|resp| resp.body().bytes())
                            .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
                            .is_some_and(|body| body.contains("INSUFFICIENT_MODEL_CAPACITY"));

                        (is_throttling_exception && has_insufficient_capacity)
                        // Legacy error response fallback
                        || (model_id_opt.is_some()
                        && status_code.is_some_and(|status| status == 500)
                        && err.as_service_error().is_some_and(|err| {
                            err.meta().message() == Some(
                    "Encountered unexpectedly high load when processing the request, please try again.",
                )}))
                    };

                    let is_monthly_limit_err = err
                        .raw_response()
                        .and_then(|resp| resp.body().bytes())
                        .and_then(|bytes| match String::from_utf8(bytes.to_vec()) {
                            Ok(s) => Some(s.contains("MONTHLY_REQUEST_COUNT")),
                            Err(_) => None,
                        })
                        .unwrap_or(false);

                    // Both ModelOverloadedError and QuotaBreach return 429,
                    // so check is_model_unavailable first.
                    if is_model_unavailable {
                        return Err(ApiClientError::ModelOverloadedError {
                            request_id: err
                                .as_service_error()
                                .and_then(|err| err.meta().request_id())
                                .map(|s| s.to_string()),
                            status_code,
                        });
                    }

                    if is_quota_breach {
                        return Err(ApiClientError::QuotaBreach {
                            message: "quota has reached its limit",
                            status_code,
                        });
                    }

                    if is_context_window_overflow {
                        return Err(ApiClientError::ContextWindowOverflow { status_code });
                    }

                    if is_monthly_limit_err {
                        return Err(ApiClientError::MonthlyLimitReached { status_code });
                    }

                    Err(err.into())
                },
            }
        } else if let Some(client) = &self.mock_client {
            let mut new_events = client.lock().next().unwrap_or_default().clone();
            new_events.reverse();

            return Ok(SendMessageOutput::Mock(new_events));
        } else {
            unreachable!("One of the clients must be created by this point");
        }
    }

    /// Get tools based on model capabilities
    async fn get_ollama_tools(&self, model: &str) -> Result<Vec<OllamaTool>, ApiClientError> {
        use crate::api_client::ollama::{OllamaTool, OllamaFunction};
        
        let mut tools = Vec::new();
        
        // Always include core functional tools
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "fs_read".to_string(),
                description: "Read files, directories and images. Always provide an 'operations' array.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operations": {
                            "type": "array",
                            "description": "Array of operations to execute",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "mode": {
                                        "type": "string", 
                                        "enum": ["Line", "Directory", "Search", "Image"],
                                        "description": "The operation mode to run in"
                                    },
                                    "path": {
                                        "type": "string",
                                        "description": "Path to the file or directory"
                                    },
                                    "start_line": {
                                        "type": "integer", 
                                        "default": 1,
                                        "description": "Starting line number (for Line mode)"
                                    },
                                    "end_line": {
                                        "type": "integer", 
                                        "default": -1,
                                        "description": "Ending line number (for Line mode)"
                                    },
                                    "pattern": {
                                        "type": "string",
                                        "description": "Pattern to search for (for Search mode)"
                                    }
                                },
                                "required": ["mode", "path"]
                            }
                        },
                        "summary": {
                            "type": "string",
                            "description": "Optional description of the purpose of this operation"
                        }
                    },
                    "required": ["operations"]
                }),
            },
        });
        
        // execute_bash tool
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: if cfg!(windows) { "execute_cmd" } else { "execute_bash" }.to_string(),
                description: "Execute shell commands on the user's system".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string", 
                            "description": "Shell command to execute"
                        },
                        "summary": {
                            "type": "string", 
                            "description": "Brief explanation of what the command does"
                        }
                    },
                    "required": ["command"]
                }),
            },
        });
        
        // fs_write tool
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "fs_write".to_string(),
                description: "Create and edit files".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string", 
                            "enum": ["create", "str_replace", "insert", "append"],
                            "description": "The command to run"
                        },
                        "path": {
                            "type": "string",
                            "description": "Absolute path to file or directory"
                        },
                        "file_text": {
                            "type": "string",
                            "description": "Content of the file to be created (for create command)"
                        },
                        "old_str": {
                            "type": "string",
                            "description": "String to replace (for str_replace command)"
                        },
                        "new_str": {
                            "type": "string",
                            "description": "New string (for str_replace and insert commands)"
                        },
                        "insert_line": {
                            "type": "integer",
                            "description": "Line number to insert after (for insert command)"
                        },
                        "summary": {
                            "type": "string",
                            "description": "Brief explanation of what the file change does"
                        }
                    },
                    "required": ["command", "path"]
                }),
            },
        });
        
        // use_aws tool
        tools.push(OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunction {
                name: "use_aws".to_string(),
                description: "Make AWS CLI api calls".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "service_name": {
                            "type": "string",
                            "description": "The name of the AWS service"
                        },
                        "operation_name": {
                            "type": "string",
                            "description": "The name of the operation to perform"
                        },
                        "parameters": {
                            "type": "object",
                            "description": "The parameters for the operation"
                        },
                        "region": {
                            "type": "string",
                            "description": "Region name for calling the operation on AWS"
                        },
                        "label": {
                            "type": "string",
                            "description": "Human readable description of the api being called"
                        }
                    },
                    "required": ["service_name", "operation_name", "region", "label"]
                }),
            },
        });
        
        // Check if model supports tools via capability detection
        if let Some(ollama_client) = &self.ollama_client {
            // For now, skip thinking tool - will add capability detection later
            // TODO: Add thinking tool based on model capabilities and settings
            tracing::debug!("Model {} tool capabilities will be checked in future implementation", model);
        }
        
        Ok(tools)
    }

    /// Internal method to send messages to Ollama
    async fn send_message_ollama_internal(&self, conversation: ConversationState) -> Result<SendMessageOutput, ApiClientError> {
        let (ollama_messages, model) = self.convert_conversation_to_ollama(conversation)?;
        let ollama_tools = self.get_ollama_tools(&model).await?;
        
        match &self.ollama_client {
            Some(client) => {
                let request = OllamaChatRequest {
                    model,
                    messages: ollama_messages,
                    tools: Some(ollama_tools), // Include tools in request
                    stream: Some(true), // Enable streaming for Task 5
                    format: None,
                    options: None,
                    keep_alive: None,
                };
                
                let stream_receiver = client.chat_stream(request).await?;
                Ok(SendMessageOutput::OllamaStreaming(stream_receiver))
            },
            None => Err(ApiClientError::UnsupportedProvider("ollama".to_string())),
        }
    }

    /// Convert AWS ConversationState to Ollama message format
    fn convert_conversation_to_ollama(&self, conversation: ConversationState) -> Result<(Vec<OllamaMessage>, String), ApiClientError> {
        let mut ollama_messages = Vec::new();
        
        // Convert conversation history
        if let Some(history) = conversation.history {
            for chat_message in history {
                match chat_message {
                    ChatMessage::UserInputMessage(user_msg) => {
                        ollama_messages.push(OllamaMessage {
                            role: "user".to_string(),
                            content: Some(user_msg.content),
                            images: self.convert_images_to_ollama(user_msg.images)?,
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    },
                    ChatMessage::AssistantResponseMessage(assistant_msg) => {
                        // TODO: Handle tool calls in assistant messages
                        // For now, just convert as regular assistant message
                        ollama_messages.push(OllamaMessage {
                            role: "assistant".to_string(),
                            content: Some(assistant_msg.content),
                            images: None, // Assistants don't send images in Ollama
                            tool_calls: None, // TODO: Convert tool uses to tool calls
                            tool_call_id: None,
                        });
                    },
                }
            }
        }
        
        // Add current user message
        let current_message = OllamaMessage {
            role: "user".to_string(),
            content: Some(conversation.user_input_message.content),
            images: self.convert_images_to_ollama(conversation.user_input_message.images)?,
            tool_calls: None,
            tool_call_id: None,
        };
        ollama_messages.push(current_message);
        
        // Determine model to use
        let model = conversation.user_input_message.model_id
            .unwrap_or_else(|| "gpt-oss:120b".to_string()); // Hardcoded for testing
        
        Ok((ollama_messages, model))
    }

    /// Convert AWS ImageBlock format to Ollama base64 format
    fn convert_images_to_ollama(&self, aws_images: Option<Vec<ImageBlock>>) -> Result<Option<Vec<String>>, ApiClientError> {
        match aws_images {
            Some(images) => {
                let mut ollama_images = Vec::new();
                for image in images {
                    // Convert AWS ImageBlock to Ollama base64 format
                    let base64_image = match image.source {
                        ImageSource::Bytes(bytes) => {
                            use base64::{Engine as _, engine::general_purpose};
                            let base64_data = general_purpose::STANDARD.encode(&bytes);
                            let format_str = match image.format {
                                ImageFormat::Png => "png",
                                ImageFormat::Jpeg => "jpeg", 
                                ImageFormat::Gif => "gif",
                                ImageFormat::Webp => "webp",
                            };
                            format!("data:image/{};base64,{}", format_str, base64_data)
                        },
                        ImageSource::Unknown => {
                            warn!("Unknown image source, skipping");
                            continue;
                        }
                    };
                    ollama_images.push(base64_image);
                }
                Ok(if ollama_images.is_empty() { None } else { Some(ollama_images) })
            },
            None => Ok(None),
        }
    }

    /// Only meant for testing. Do not use outside of testing responses.
    pub fn set_mock_output(&mut self, json: serde_json::Value) {
        let mut mock = Vec::new();
        for response in json.as_array().unwrap() {
            let mut stream = Vec::new();
            for event in response.as_array().unwrap() {
                match event {
                    serde_json::Value::String(assistant_text) => {
                        stream.push(ChatResponseStream::AssistantResponseEvent {
                            content: assistant_text.clone(),
                        });
                    },
                    serde_json::Value::Object(tool_use) => {
                        stream.append(&mut split_tool_use_event(tool_use));
                    },
                    other => panic!("Unexpected value: {:?}", other),
                }
            }
            mock.push(stream);
        }

        self.mock_client = Some(Arc::new(Mutex::new(mock.into_iter())));
    }
}

fn timeout_config(database: &Database) -> TimeoutConfig {
    let timeout = database
        .settings
        .get_int(Setting::ApiTimeout)
        .and_then(|i| i.try_into().ok())
        .map_or(DEFAULT_TIMEOUT_DURATION, Duration::from_millis);

    TimeoutConfig::builder()
        .read_timeout(timeout)
        .operation_timeout(timeout)
        .operation_attempt_timeout(timeout)
        .connect_timeout(timeout)
        .build()
}

fn retry_config() -> RetryConfig {
    RetryConfig::adaptive()
        .with_max_attempts(3)
        .with_max_backoff(Duration::from_secs(10))
}

pub fn stalled_stream_protection_config() -> StalledStreamProtectionConfig {
    StalledStreamProtectionConfig::enabled()
        .grace_period(Duration::from_secs(60 * 5))
        .build()
}

fn split_tool_use_event(value: &Map<String, serde_json::Value>) -> Vec<ChatResponseStream> {
    let tool_use_id = value.get("tool_use_id").unwrap().as_str().unwrap().to_string();
    let name = value.get("name").unwrap().as_str().unwrap().to_string();
    let args_str = value.get("args").unwrap().to_string();
    let split_point = args_str.len() / 2;
    vec![
        ChatResponseStream::ToolUseEvent {
            tool_use_id: tool_use_id.clone(),
            name: name.clone(),
            input: None,
            stop: None,
        },
        ChatResponseStream::ToolUseEvent {
            tool_use_id: tool_use_id.clone(),
            name: name.clone(),
            input: Some(args_str.split_at(split_point).0.to_string()),
            stop: None,
        },
        ChatResponseStream::ToolUseEvent {
            tool_use_id: tool_use_id.clone(),
            name: name.clone(),
            input: Some(args_str.split_at(split_point).1.to_string()),
            stop: None,
        },
        ChatResponseStream::ToolUseEvent {
            tool_use_id: tool_use_id.clone(),
            name: name.clone(),
            input: None,
            stop: Some(true),
        },
    ]
}

#[cfg(test)]
mod tests {
    use amzn_codewhisperer_client::types::{
        ChatAddMessageEvent,
        IdeCategory,
        OperatingSystem,
    };
    use aws_types::region::Region;
    use aws_credential_types::Credentials;

    use super::*;
    use crate::api_client::model::UserInputMessage;
    use crate::api_client::ollama::OllamaMessage;

    #[tokio::test]
    async fn create_clients() {
        let env = Env::new();
        let fs = Fs::new();
        let mut database = crate::database::Database::new().await.unwrap();
        let _ = ApiClient::new(&env, &fs, &mut database, None).await;
    }

    #[tokio::test]
    async fn test_mock() {
        let env = Env::new();
        let fs = Fs::new();
        let mut database = crate::database::Database::new().await.unwrap();
        let mut client = ApiClient::new(&env, &fs, &mut database, None).await.unwrap();
        client
            .send_telemetry_event(
                TelemetryEvent::ChatAddMessageEvent(
                    ChatAddMessageEvent::builder()
                        .conversation_id("<conversation-id>")
                        .message_id("<message-id>")
                        .build()
                        .unwrap(),
                ),
                UserContext::builder()
                    .ide_category(IdeCategory::Cli)
                    .operating_system(OperatingSystem::Linux)
                    .product("<product>")
                    .build()
                    .unwrap(),
                false,
                Some("model".to_owned()),
            )
            .await
            .unwrap();

        client.mock_client = Some(Arc::new(Mutex::new(
            vec![vec![
                ChatResponseStream::AssistantResponseEvent {
                    content: "Hello!".to_owned(),
                },
                ChatResponseStream::AssistantResponseEvent {
                    content: " How can I".to_owned(),
                },
                ChatResponseStream::AssistantResponseEvent {
                    content: " assist you today?".to_owned(),
                },
            ]]
            .into_iter(),
        )));

        let mut output = client
            .send_message(ConversationState {
                conversation_id: None,
                user_input_message: UserInputMessage {
                    images: None,
                    content: "Hello".into(),
                    user_input_message_context: None,
                    user_intent: None,
                    model_id: Some("model".to_owned()),
                },
                history: None,
            })
            .await
            .unwrap();

        let mut output_content = String::new();
        while let Some(ChatResponseStream::AssistantResponseEvent { content }) = output.recv().await.unwrap() {
            output_content.push_str(&content);
        }
        assert_eq!(output_content, "Hello! How can I assist you today?");
    }
    
    #[tokio::test]
    async fn test_send_message_ollama_no_client() {
        // Test when no Ollama client is configured
        let api_client = ApiClient {
            client: create_dummy_aws_client().await,
            streaming_client: None,
            sigv4_streaming_client: None,
            ollama_client: None,
            mock_client: None,
            profile: None,
            model_cache: Arc::new(RwLock::new(None)),
            provider: ModelProvider::Aws,
        };
        
        let messages = vec![OllamaMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            images: None,
        }];
        
        let result = api_client.send_message_ollama(messages, "llama3.2").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiClientError::UnsupportedProvider(_)));
    }
    
    async fn create_dummy_aws_client() -> CodewhispererClient {
        let credentials = Credentials::new("dummy", "dummy", None, None, "dummy");
        let config = aws_config::defaults(behavior_version())
            .region(Region::new("us-east-1"))
            .credentials_provider(credentials)
            .load()
            .await;
        
        CodewhispererClient::from_conf(
            amzn_codewhisperer_client::config::Builder::from(&config).build(),
        )
    }
}

#[cfg(test)]
mod task4_tests {
    use super::*;
    use crate::api_client::model::{AssistantResponseMessage, UserInputMessage};
    use crate::util::ModelProvider;
    use aws_credential_types::Credentials;
    use aws_types::region::Region;
    
    async fn create_dummy_aws_client() -> CodewhispererClient {
        let credentials = Credentials::new("dummy", "dummy", None, None, "dummy");
        let config = aws_config::defaults(behavior_version())
            .region(Region::new("us-east-1"))
            .credentials_provider(credentials)
            .load()
            .await;
        
        CodewhispererClient::from_conf(
            amzn_codewhisperer_client::config::Builder::from(&config).build(),
        )
    }

    async fn create_test_api_client() -> ApiClient {
        ApiClient {
            client: create_dummy_aws_client().await,
            streaming_client: None,
            sigv4_streaming_client: None,
            ollama_client: None,
            mock_client: None,
            profile: None,
            model_cache: Arc::new(RwLock::new(None)),
            provider: ModelProvider::Ollama,
        }
    }
    
    #[tokio::test]
    async fn test_convert_simple_conversation_to_ollama() {
        let api_client = create_test_api_client().await;
        let conversation = ConversationState {
            conversation_id: Some("test-conv".to_string()),
            user_input_message: UserInputMessage {
                content: "Hello, world!".to_string(),
                model_id: Some("llama3.2".to_string()),
                user_input_message_context: None,
                user_intent: None,
                images: None,
            },
            history: None,
        };
        
        let (messages, model) = api_client.convert_conversation_to_ollama(conversation).unwrap();
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello, world!");
        assert_eq!(messages[0].images, None);
        assert_eq!(model, "llama3.2");
    }
    
    #[tokio::test]
    async fn test_convert_conversation_with_history() {
        let api_client = create_test_api_client().await;
        let conversation = ConversationState {
            conversation_id: Some("test-conv".to_string()),
            user_input_message: UserInputMessage {
                content: "Follow up question".to_string(),
                model_id: Some("llama3.2".to_string()),
                user_input_message_context: None,
                user_intent: None,
                images: None,
            },
            history: Some(vec![
                ChatMessage::UserInputMessage(UserInputMessage {
                    content: "Initial question".to_string(),
                    model_id: None,
                    user_input_message_context: None,
                    user_intent: None,
                    images: None,
                }),
                ChatMessage::AssistantResponseMessage(AssistantResponseMessage {
                    message_id: None,
                    content: "Initial response".to_string(),
                    tool_uses: None,
                }),
            ]),
        };
        
        let (messages, model) = api_client.convert_conversation_to_ollama(conversation).unwrap();
        
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Initial question");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Initial response");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, "Follow up question");
        assert_eq!(model, "llama3.2");
    }
    
    #[tokio::test]
    async fn test_convert_conversation_default_model() {
        let api_client = create_test_api_client().await;
        let conversation = ConversationState {
            conversation_id: Some("test-conv".to_string()),
            user_input_message: UserInputMessage {
                content: "Hello".to_string(),
                model_id: None, // No model specified
                user_input_message_context: None,
                user_intent: None,
                images: None,
            },
            history: None,
        };
        
        let (messages, model) = api_client.convert_conversation_to_ollama(conversation).unwrap();
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(model, "gpt-oss:120b"); // Updated default model
    }
    
    #[tokio::test]
    async fn test_convert_images_to_ollama() {
        let api_client = create_test_api_client().await;
        
        // Test with no images
        let result = api_client.convert_images_to_ollama(None).unwrap();
        assert_eq!(result, None);
        
        // Test with empty images
        let result = api_client.convert_images_to_ollama(Some(vec![])).unwrap();
        assert_eq!(result, None);
        
        // Test with PNG image
        let png_image = ImageBlock {
            format: ImageFormat::Png,
            source: ImageSource::Bytes(vec![137, 80, 78, 71]), // PNG header bytes
        };
        let result = api_client.convert_images_to_ollama(Some(vec![png_image])).unwrap();
        assert!(result.is_some());
        let images = result.unwrap();
        assert_eq!(images.len(), 1);
        assert!(images[0].starts_with("data:image/png;base64,"));
        
        // Test with JPEG image
        let jpeg_image = ImageBlock {
            format: ImageFormat::Jpeg,
            source: ImageSource::Bytes(vec![255, 216, 255, 224]), // JPEG header bytes
        };
        let result = api_client.convert_images_to_ollama(Some(vec![jpeg_image])).unwrap();
        assert!(result.is_some());
        let images = result.unwrap();
        assert_eq!(images.len(), 1);
        assert!(images[0].starts_with("data:image/jpeg;base64,"));
    }
    
    #[tokio::test]
    async fn test_convert_images_unknown_source() {
        let api_client = create_test_api_client().await;
        
        let unknown_image = ImageBlock {
            format: ImageFormat::Png,
            source: ImageSource::Unknown,
        };
        let result = api_client.convert_images_to_ollama(Some(vec![unknown_image])).unwrap();
        assert_eq!(result, None); // Unknown source should be skipped
    }
}
