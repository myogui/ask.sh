use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{ChatStream, LLMConfig, LLMError, LLMProvider};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Debug)]
pub struct AnthropicProvider {
    client: Client,
    model: String,
    api_key: String,
    conversation_history: Vec<Message>,
}

#[derive(Serialize, Debug)]
struct AnthropicRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    max_tokens: u32,
}

#[derive(Serialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<Delta>,
}

#[derive(Deserialize, Debug)]
struct Delta {
    text: Option<String>,
}

impl AnthropicProvider {
    pub fn new(config: LLMConfig) -> Result<Self, LLMError> {
        let client = Client::builder()
            .build()
            .map_err(|e| LLMError::ConfigError(e.to_string()))?;

        Ok(Self {
            client,
            model: config.model,
            api_key: config.api_key,
            conversation_history: Vec::new(),
        })
    }

    fn parse_sse_line(line: &str) -> Option<String> {
        if line.is_empty() || line.starts_with(':') {
            return None;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                return None;
            }

            if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                if event.event_type == "content_block_delta" {
                    if let Some(delta) = event.delta {
                        return delta.text;
                    }
                }
            }
        }
        None
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    /// Add a system message at the start of the conversation
    fn with_system_prompt(&mut self, prompt: &str) {
        self.conversation_history.push(Message {
            role: "system".to_string(),
            content: prompt.to_string(),
        });
    }

    async fn chat_stream(&mut self, user_message: String) -> Result<ChatStream, LLMError> {
        // Add user message to history
        self.conversation_history.push(Message {
            role: "user".to_string(),
            content: user_message.to_string(),
        });

        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: self.conversation_history.clone(),
            stream: true,
            max_tokens: 4096,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LLMError::ApiError(format!(
                "Anthropic API error: {}",
                error_text
            )));
        }

        let stream = response.bytes_stream().map(move |result| match result {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let mut content = String::new();

                for line in text.lines() {
                    if let Some(text) = Self::parse_sse_line(line) {
                        content.push_str(&text);
                    }
                }

                if !content.is_empty() {
                    Ok(content)
                } else {
                    Ok(String::new())
                }
            }
            Err(e) => Err(LLMError::NetworkError(e.to_string())),
        });

        let filtered_stream = stream.filter(|result| {
            futures::future::ready(match result {
                Ok(content) => !content.is_empty(),
                Err(_) => true,
            })
        });

        Ok(Box::pin(filtered_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anthropic_provider_creation() {
        let config = LLMConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-opus-20240229".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            keep_alive: None,
            context_length: None,
        };

        let provider = AnthropicProvider::new(config).unwrap();
        assert_eq!(provider.model, "claude-3-opus-20240229");
    }
}
