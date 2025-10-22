use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{ChatStream, LLMConfig, LLMError, LLMProvider};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    keep_alive: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

// For Ollama native format
#[derive(Debug, Deserialize)]
struct OllamaNativeResponse {
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug)]
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
    keep_alive: Option<i64>,
}

impl OllamaProvider {
    pub fn new(config: LLMConfig) -> Result<Self, LLMError> {
        let base_url = config
            .base_url
            .unwrap_or_else(|| "http://localhost:11434/api".to_string());

        Ok(Self {
            client: Client::new(),
            base_url,
            model: config.model,
            keep_alive: config.keep_alive,
        })
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn keep_alive(&self) -> Option<i64> {
        self.keep_alive
    }

    async fn chat_stream(
        &self,
        system_message: String,
        user_message: String,
    ) -> Result<ChatStream, LLMError> {
        // Use Ollama's native endpoint
        let url = format!("{}/chat", self.base_url);

        let request = OllamaRequest {
            model: self.model.clone(),
            keep_alive: self.keep_alive.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_message,
                },
                Message {
                    role: "user".to_string(),
                    content: user_message,
                },
            ],
            stream: true,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LLMError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        // Parse Ollama's native streaming format
        let stream = response.bytes_stream();
        let mapped_stream = stream.filter_map(|result| async move {
            match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);

                    // Ollama native API returns newline-delimited JSON (not SSE format)
                    for line in text.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }

                        // Try parsing as Ollama native format
                        if let Ok(response) = serde_json::from_str::<OllamaNativeResponse>(line) {
                            if let Some(message) = response.message {
                                let content = message.content;

                                if !content.is_empty() {
                                    return Some(Ok(content));
                                }
                            }
                        }
                    }
                    None
                }
                Err(e) => Some(Err(LLMError::ApiError(e.to_string()))),
            }
        });

        Ok(Box::pin(mapped_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ollama_provider_creation() {
        let config = LLMConfig {
            provider: "ollama".to_string(),
            model: "gemma3".to_string(),
            api_key: String::new(), // Not needed for Ollama
            base_url: Some("http://localhost:11434".to_string()),
            keep_alive: Some(-1),
        };

        let provider = OllamaProvider::new(config).unwrap();
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model(), "gemma3");
    }
}
