use async_trait::async_trait;
use futures::Stream;
use std::{fmt::Debug, pin::Pin};
use thiserror::Error;

/// Error from LLM provider
#[derive(Debug, Error)]
pub enum LLMError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Invalid request: {0}")]
    InvalidRequestError(String),
}

/// LLM configuration
#[derive(Debug, Clone)]
pub struct LLMConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>, // Custom endpoint URL (for OpenAI and Ollama)
    pub keep_alive: Option<i64>,  // Amount of minutes to keep the model loaded (Ollama only)
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            api_key: String::new(),
            base_url: None,
            keep_alive: None,
        }
    }
}

/// Type alias for chat stream
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send + 'static>>;

/// Trait for LLM provider
#[async_trait]
pub trait LLMProvider: Send + Sync + Debug {
    /// Returns the provider name
    fn name(&self) -> &'static str;

    /// Returns the current model name
    fn model(&self) -> &str;

    fn keep_alive(&self) -> Option<i64>;

    /// Get chat completion as a stream
    async fn chat_stream(
        &self,
        system_message: String,
        user_message: String,
    ) -> Result<ChatStream, LLMError>;
}

pub mod anthropic;
pub mod ollama;
pub mod openai;

/// Available LLM providers
#[derive(Debug)]
pub enum Provider {
    OpenAI(openai::OpenAIProvider),
    Anthropic(anthropic::AnthropicProvider),
    Ollama(ollama::OllamaProvider),
}

#[async_trait]
impl LLMProvider for Provider {
    fn name(&self) -> &'static str {
        match self {
            Provider::OpenAI(p) => p.name(),
            Provider::Anthropic(p) => p.name(),
            Provider::Ollama(p) => p.name(),
        }
    }

    fn model(&self) -> &str {
        match self {
            Provider::OpenAI(p) => p.model(),
            Provider::Anthropic(p) => p.model(),
            Provider::Ollama(p) => p.model(),
        }
    }

    fn keep_alive(&self) -> Option<i64> {
        match self {
            Provider::OpenAI(p) => p.keep_alive(),
            Provider::Anthropic(p) => p.keep_alive(),
            Provider::Ollama(p) => p.keep_alive(),
        }
    }

    async fn chat_stream(
        &self,
        system_message: String,
        user_message: String,
    ) -> Result<ChatStream, LLMError> {
        match self {
            Provider::OpenAI(p) => p.chat_stream(system_message, user_message).await,
            Provider::Anthropic(p) => p.chat_stream(system_message, user_message).await,
            Provider::Ollama(p) => p.chat_stream(system_message, user_message).await,
        }
    }
}

/// Provider factory
pub fn create_provider(config: LLMConfig) -> Result<Provider, LLMError> {
    match config.provider.as_str() {
        "openai" => Ok(Provider::OpenAI(openai::OpenAIProvider::new(config)?)),
        "anthropic" => Ok(Provider::Anthropic(anthropic::AnthropicProvider::new(
            config,
        )?)),
        "ollama" => Ok(Provider::Ollama(ollama::OllamaProvider::new(config)?)),
        _ => Err(LLMError::ConfigError(format!(
            "Unknown provider: {}",
            config.provider
        ))),
    }
}
