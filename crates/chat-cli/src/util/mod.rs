pub mod consts;
pub mod directories;
pub mod knowledge_store;
pub mod open;
pub mod pattern_matching;
pub mod process;
pub mod spinner;
pub mod system_info;
#[cfg(test)]
pub mod test;

use std::fmt::Display;
use std::io::{
    ErrorKind,
    stdout,
};

use anstream::stream::IsTerminal;
pub use consts::*;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use eyre::{
    Context,
    Result,
    bail,
};
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum UtilError {
    #[error("io operation error")]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Directory(#[from] directories::DirectoryError),
    #[error(transparent)]
    StrUtf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Model provider for Amazon Q CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProvider {
    /// AWS-hosted models (CodeWhisperer, Q Developer)
    Aws,
    /// Local Ollama models
    Ollama,
    /// OpenAI models (GPT-3.5, GPT-4, etc.)
    OpenAi,
    /// Anthropic models (Claude, etc.)
    Anthropic,
}

impl ModelProvider {
    /// Parse model provider from environment variables with validation
    pub fn from_env() -> Result<Self> {
        let provider = std::env::var("Q_CLI_MODEL_PROVIDER")
            .unwrap_or_else(|_| "aws".to_string())
            .to_lowercase();
        
        match provider.as_str() {
            "aws" => Ok(Self::Aws),
            "ollama" => Ok(Self::Ollama),
            "openai" => {
                // Check for required API key
                if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                    bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using OpenAI provider");
                }
                Ok(Self::OpenAi)
            },
            "anthropic" => {
                // Check for required API key
                if std::env::var("Q_CLI_MODEL_PROVIDER_API_KEY").is_err() {
                    bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using Anthropic provider");
                }
                Ok(Self::Anthropic)
            },
            _ => bail!(
                "Invalid Q_CLI_MODEL_PROVIDER: '{}'. Valid values are: aws, ollama, openai, anthropic", 
                provider
            ),
        }
    }
    
    /// Internal method for testing - allows injecting environment variable values
    #[cfg(test)]
    fn from_test_env(provider_var: Option<&str>, api_key_var: Option<&str>) -> Result<Self> {
        let provider = provider_var
            .unwrap_or("aws")
            .to_lowercase();
        
        match provider.as_str() {
            "aws" => Ok(Self::Aws),
            "ollama" => Ok(Self::Ollama),
            "openai" => {
                // Check for required API key
                if api_key_var.is_none() {
                    bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using OpenAI provider");
                }
                Ok(Self::OpenAi)
            },
            "anthropic" => {
                // Check for required API key
                if api_key_var.is_none() {
                    bail!("Q_CLI_MODEL_PROVIDER_API_KEY environment variable is required when using Anthropic provider");
                }
                Ok(Self::Anthropic)
            },
            _ => bail!(
                "Invalid Q_CLI_MODEL_PROVIDER: '{}'. Valid values are: aws, ollama, openai, anthropic", 
                provider
            ),
        }
    }
    
    /// Check if this provider requires AWS authentication
    pub fn requires_auth(&self) -> bool {
        matches!(self, Self::Aws)
    }
    
    /// Check if this provider requires an API key
    pub fn requires_api_key(&self) -> bool {
        matches!(self, Self::OpenAi | Self::Anthropic)
    }
}

impl Display for ModelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aws => write!(f, "aws"),
            Self::Ollama => write!(f, "ollama"),
            Self::OpenAi => write!(f, "openai"),
            Self::Anthropic => write!(f, "anthropic"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnknownDesktopErrContext {
    xdg_current_desktop: String,
    xdg_session_desktop: String,
    gdm_session: String,
}

impl std::fmt::Display for UnknownDesktopErrContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XDG_CURRENT_DESKTOP: `{}`, ", self.xdg_current_desktop)?;
        write!(f, "XDG_SESSION_DESKTOP: `{}`, ", self.xdg_session_desktop)?;
        write!(f, "GDMSESSION: `{}`", self.gdm_session)
    }
}

pub fn choose(prompt: impl Display, options: &[impl ToString]) -> Result<Option<usize>> {
    if options.is_empty() {
        bail!("no options passed to choose")
    }

    if !stdout().is_terminal() {
        warn!("called choose while stdout is not a terminal");
        return Ok(Some(0));
    }

    match Select::with_theme(&dialoguer_theme())
        .items(options)
        .default(0)
        .with_prompt(prompt.to_string())
        .interact_opt()
    {
        Ok(ok) => Ok(ok),
        Err(dialoguer::Error::IO(io)) if io.kind() == ErrorKind::Interrupted => Ok(None),
        Err(e) => Err(e).wrap_err("Failed to choose"),
    }
}

pub fn input(prompt: &str, initial_text: Option<&str>) -> Result<String> {
    if !stdout().is_terminal() {
        warn!("called input while stdout is not a terminal");
        return Ok(String::new());
    }

    let theme = dialoguer_theme();
    let mut input = dialoguer::Input::with_theme(&theme).with_prompt(prompt);

    if let Some(initial_text) = initial_text {
        input = input.with_initial_text(initial_text);
    }

    Ok(input.interact_text()?)
}

pub fn dialoguer_theme() -> ColorfulTheme {
    ColorfulTheme {
        prompt_prefix: dialoguer::console::style("?".into()).for_stderr().magenta(),
        ..ColorfulTheme::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_env_default() {
        let provider = ModelProvider::from_test_env(None, None).unwrap();
        assert_eq!(provider, ModelProvider::Aws);
    }

    #[test]
    fn test_provider_from_env_valid_values() {
        for (env_val, expected) in [
            ("aws", ModelProvider::Aws),
            ("AWS", ModelProvider::Aws),
            ("ollama", ModelProvider::Ollama),
            ("OLLAMA", ModelProvider::Ollama),
        ] {
            let provider = ModelProvider::from_test_env(Some(env_val), None).unwrap();
            assert_eq!(provider, expected);
        }
    }

    #[test]
    fn test_provider_from_env_invalid() {
        let result = ModelProvider::from_test_env(Some("invalid"), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid Q_CLI_MODEL_PROVIDER"));
    }

    #[test]
    fn test_openai_requires_api_key() {
        let result = ModelProvider::from_test_env(Some("openai"), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Q_CLI_MODEL_PROVIDER_API_KEY"));
    }

    #[test]
    fn test_anthropic_requires_api_key() {
        let result = ModelProvider::from_test_env(Some("anthropic"), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Q_CLI_MODEL_PROVIDER_API_KEY"));
    }

    #[test]
    fn test_ollama_no_api_key_required() {
        let provider = ModelProvider::from_test_env(Some("ollama"), None).unwrap();
        assert_eq!(provider, ModelProvider::Ollama);
    }

    #[test]
    fn test_openai_with_api_key_succeeds() {
        let provider = ModelProvider::from_test_env(Some("openai"), Some("test-key")).unwrap();
        assert_eq!(provider, ModelProvider::OpenAi);
    }

    #[test]
    fn test_provider_requires_auth() {
        assert!(ModelProvider::Aws.requires_auth());
        assert!(!ModelProvider::Ollama.requires_auth());
        assert!(!ModelProvider::OpenAi.requires_auth());
        assert!(!ModelProvider::Anthropic.requires_auth());
    }

    #[test]
    fn test_provider_requires_api_key() {
        assert!(!ModelProvider::Aws.requires_api_key());
        assert!(!ModelProvider::Ollama.requires_api_key());
        assert!(ModelProvider::OpenAi.requires_api_key());
        assert!(ModelProvider::Anthropic.requires_api_key());
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(ModelProvider::Aws.to_string(), "aws");
        assert_eq!(ModelProvider::Ollama.to_string(), "ollama");
        assert_eq!(ModelProvider::OpenAi.to_string(), "openai");
        assert_eq!(ModelProvider::Anthropic.to_string(), "anthropic");
    }
}
