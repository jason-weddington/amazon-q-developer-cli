use amzn_codewhisperer_client::types::Model;
use clap::Args;
use crossterm::style::{
    self,
    Color,
};
use crossterm::{
    execute,
    queue,
};
use dialoguer::Select;
use serde::{
    Deserialize,
    Serialize,
};

use crate::api_client::Endpoint;
use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};
use crate::os::Os;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    /// Actual model id to send in the API
    pub model_id: String,
    /// Size of the model's context window, in tokens
    #[serde(default = "default_context_window")]
    pub context_window_tokens: usize,
}

impl ModelInfo {
    pub fn from_api_model(model: &Model) -> Self {
        let context_window_tokens = model
            .token_limits()
            .and_then(|limits| limits.max_input_tokens())
            .map_or(default_context_window(), |tokens| tokens as usize);
        Self {
            model_id: model.model_id().to_string(),
            model_name: model.model_name().map(|s| s.to_string()),
            context_window_tokens,
        }
    }

    /// create a default model with only valid model_id（be compatoble with old stored model data）
    pub fn from_id(model_id: String) -> Self {
        Self {
            model_id,
            model_name: None,
            context_window_tokens: 200_000,
        }
    }

    pub fn display_name(&self) -> &str {
        self.model_name.as_deref().unwrap_or(&self.model_id)
    }
}
#[deny(missing_docs)]
#[derive(Debug, PartialEq, Args)]
pub struct ModelArgs;

impl ModelArgs {
    pub async fn execute(self, os: &mut Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        Ok(select_model(os, session).await?.unwrap_or(ChatState::PromptUser {
            skip_printing_tools: false,
        }))
    }
}

pub async fn select_model(os: &mut Os, session: &mut ChatSession) -> Result<Option<ChatState>, ChatError> {
    queue!(session.stderr, style::Print("\n"))?;

    // Fetch available models from service
    let (models, _default_model) = get_available_models(os).await?;

    if models.is_empty() {
        queue!(
            session.stderr,
            style::SetForegroundColor(Color::Red),
            style::Print("No models available\n"),
            style::ResetColor
        )?;
        return Ok(None);
    }

    let active_model_id = session.conversation.model_info.as_ref().map(|m| m.model_id.as_str());

    let labels: Vec<String> = models
        .iter()
        .map(|model| {
            let display_name = model.display_name();
            if Some(model.model_id.as_str()) == active_model_id {
                format!("{} (active)", display_name)
            } else {
                display_name.to_owned()
            }
        })
        .collect();

    let selection: Option<_> = match Select::with_theme(&crate::util::dialoguer_theme())
        .with_prompt("Select a model for this chat session")
        .items(&labels)
        .default(0)
        .interact_on_opt(&dialoguer::console::Term::stdout())
    {
        Ok(sel) => {
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::style::SetForegroundColor(crossterm::style::Color::Magenta)
            );
            sel
        },
        // Ctrl‑C -> Err(Interrupted)
        Err(dialoguer::Error::IO(ref e)) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(None),
        Err(e) => return Err(ChatError::Custom(format!("Failed to choose model: {e}").into())),
    };

    queue!(session.stderr, style::ResetColor)?;

    if let Some(index) = selection {
        let selected = models[index].clone();
        session.conversation.model_info = Some(selected.clone());
        let display_name = selected.display_name();

        // Save the selected model for future sessions
        if let Ok(provider) = std::env::var("Q_CLI_MODEL_PROVIDER") {
            if provider.to_lowercase() == "ollama" {
                // Save Ollama model selection
                if let Err(e) = os.database.settings.set(crate::database::settings::Setting::ChatDefaultOllamaModel, selected.model_id.as_str()).await {
                    tracing::warn!("Failed to save Ollama model preference: {}", e);
                }
            }
            // Note: AWS model saving would go here if it doesn't exist elsewhere
        }

        queue!(
            session.stderr,
            style::Print("\n"),
            style::Print(format!(" Using {}\n\n", display_name)),
            style::ResetColor,
            style::SetForegroundColor(Color::Reset),
            style::SetBackgroundColor(Color::Reset),
        )?;
    }

    execute!(session.stderr, style::ResetColor)?;

    Ok(Some(ChatState::PromptUser {
        skip_printing_tools: false,
    }))
}

pub async fn get_model_info(model_id: &str, os: &Os) -> Result<ModelInfo, ChatError> {
    let (models, _) = get_available_models(os).await?;

    models
        .into_iter()
        .find(|m| m.model_id == model_id)
        .ok_or_else(|| ChatError::Custom(format!("Model '{}' not found", model_id).into()))
}

/// Get available models with caching support
pub async fn get_available_models(os: &Os) -> Result<(Vec<ModelInfo>, ModelInfo), ChatError> {
    // Check if using Ollama provider
    if let Ok(provider) = std::env::var("Q_CLI_MODEL_PROVIDER") {
        if provider.to_lowercase() == "ollama" {
            // For Ollama, use the external provider to get models
            match os.client.list_external_models().await {
                Ok(model_names) => {
                    if model_names.is_empty() {
                        return Err(ChatError::Custom(
                            "No Ollama models found. Pull a model first:\n  ollama pull llama3.2\n  ollama pull codellama".into()
                        ));
                    }
                    
                    // Build models with dynamic context window querying
                    let mut external_models = Vec::new();
                    for name in model_names {
                        // Use external provider to get context window info
                        let context_window_tokens = match os.client.get_external_model_context_window(&name).await {
                            Ok(Some(size)) => size,
                            Ok(None) | Err(_) => 200_000, // Fallback to default
                        };
                        
                        external_models.push(ModelInfo {
                            model_name: Some(name.clone()),
                            model_id: name,
                            context_window_tokens,
                        });
                    }
                    
                    // Check for saved Ollama model preference
                    let default_model = if let Some(saved_model_id) = os.database.settings.get_string(crate::database::settings::Setting::ChatDefaultOllamaModel) {
                        // Try to find the saved model in available models
                        external_models.iter()
                            .find(|m| m.model_id == saved_model_id)
                            .cloned()
                            .unwrap_or_else(|| {
                                tracing::warn!("Saved Ollama model '{}' not found, using first available", saved_model_id);
                                external_models[0].clone()
                            })
                    } else {
                        // No saved preference, use first available
                        external_models[0].clone()
                    };
                    
                    return Ok((external_models, default_model));
                },
                Err(e) => {
                    tracing::error!("Failed to fetch Ollama models: {}", e);
                    return Err(ChatError::Custom(
                        "Failed to connect to Ollama server. Make sure it's running:\n  ollama serve".into()
                    ));
                }
            }
        }
    }

    // Existing AWS logic
    let endpoint = Endpoint::configured_value(&os.database);
    let region = endpoint.region().as_ref();

    match os.client.get_available_models(region).await {
        Ok(api_res) => {
            let models: Vec<ModelInfo> = api_res.models.iter().map(ModelInfo::from_api_model).collect();
            let default_model = ModelInfo::from_api_model(&api_res.default_model);

            tracing::debug!("Successfully fetched {} models from API", models.len());
            Ok((models, default_model))
        },
        // In case of API throttling or other errors, fall back to hardcoded models
        Err(e) => {
            tracing::error!("Failed to fetch models from API: {}, using fallback list", e);

            let models = get_fallback_models();
            let default_model = models[0].clone();

            Ok((models, default_model))
        },
    }
}

/// Returns the context window length in tokens for the given model_id.
/// Uses cached model data when available
pub fn context_window_tokens(model_info: Option<&ModelInfo>) -> usize {
    model_info.map_or_else(default_context_window, |m| m.context_window_tokens)
}

fn default_context_window() -> usize {
    200_000
}

fn get_fallback_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            model_name: Some("claude-sonnet-4".to_string()),
            model_id: "claude-sonnet-4".to_string(),
            context_window_tokens: 200_000,
        },
        ModelInfo {
            model_name: Some("claude-3.7-sonnet".to_string()),
            model_id: "claude-3.7-sonnet".to_string(),
            context_window_tokens: 200_000,
        },
    ]
}

pub fn normalize_model_name(name: &str) -> &str {
    match name {
        "claude-4-sonnet" => "claude-sonnet-4",
        // can add more mapping for backward compatibility
        _ => name,
    }
}

pub fn find_model<'a>(models: &'a [ModelInfo], name: &str) -> Option<&'a ModelInfo> {
    let normalized = normalize_model_name(name);
    models.iter().find(|m| {
        m.model_name
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(normalized))
            || m.model_id.eq_ignore_ascii_case(normalized)
    })
}
