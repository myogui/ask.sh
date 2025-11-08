use async_trait::async_trait;
use futures::stream::StreamExt;
use futures::Stream;
use std::{error::Error, fmt::Debug, pin::Pin};
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
    pub keep_alive: Option<i32>,  // Amount of minutes to keep the model loaded (Ollama only)
    pub context_length: Option<u32>, // Context length to pass to Ollama (Ollama only)
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            api_key: String::new(),
            base_url: None,
            keep_alive: None,
            context_length: None,
        }
    }
}

/// Type alias for chat stream
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send + 'static>>;

/// Trait for LLM provider
#[async_trait]
pub trait LLMProvider: Send + Sync + Debug {
    fn with_system_prompt(&mut self, prompt: &str);

    /// Get chat completion as a stream
    async fn chat_stream(&mut self, user_message: String) -> Result<ChatStream, LLMError>;

    async fn chat(&mut self, user_input: String) -> Result<String, Box<dyn Error>> {
        let mut stream = self
            .chat_stream(user_input)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;

        let mut response_to_return = String::new();
        while let Some(result) = stream.next().await {
            match result {
                Ok(content) => {
                    response_to_return.push_str(&content);
                    eprint!("{}", content);
                }
                Err(err) => {
                    eprint!("{}", err);
                }
            }
        }
        Ok(response_to_return)
    }
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
    fn with_system_prompt(&mut self, prompt: &str) {
        match self {
            Provider::OpenAI(p) => p.with_system_prompt(prompt),
            Provider::Anthropic(p) => p.with_system_prompt(prompt),
            Provider::Ollama(p) => p.with_system_prompt(prompt),
        }
    }

    async fn chat_stream(&mut self, user_message: String) -> Result<ChatStream, LLMError> {
        match self {
            Provider::OpenAI(p) => p.chat_stream(user_message).await,
            Provider::Anthropic(p) => p.chat_stream(user_message).await,
            Provider::Ollama(p) => p.chat_stream(user_message).await,
        }
    }
}

/// Provider factory
pub fn create_llm_provider(config: LLMConfig) -> Result<Provider, LLMError> {
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
