use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::{
    llm::{ChatResponse, Message},
    tools::Tool,
};

use super::{ChatStream, LLMConfig, LLMError, LLMProvider};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    keep_alive: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ModelOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ModelOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
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
    keep_alive: Option<i32>,
    context_length: Option<u32>,
    conversation_history: Vec<Message>,
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
            context_length: config.context_length,
            conversation_history: Vec::new(),
        })
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    /// Add a system message at the start of the conversation
    fn with_system_prompt(&mut self, prompt: &str) {
        self.conversation_history.push(Message {
            role: "system".to_string(),
            content: prompt.to_string(),
            ..Default::default()
        });
    }

    async fn chat_stream(&mut self, user_message: &Message) -> Result<ChatStream, LLMError> {
        // Use Ollama's native endpoint
        let url = format!("{}/chat", self.base_url);

        // Add user message to history
        self.conversation_history.push(user_message.clone());

        let request = OllamaRequest {
            model: self.model.clone(),
            keep_alive: self.keep_alive.clone(),
            messages: self.conversation_history.clone(),
            stream: true,
            tools: Some(self.get_available_tools()),
            options: Some(ModelOptions {
                num_ctx: self.context_length.clone(),
                ..Default::default()
            }),
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
                                let tool_calls = message.tool_calls.unwrap_or_default();

                                if !content.is_empty() || !tool_calls.is_empty() {
                                    let chat_response = ChatResponse {
                                        content: content,
                                        tool_calls: Some(tool_calls),
                                    };
                                    return Some(Ok(chat_response));
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
            context_length: Some(8192),
        };

        let provider = OllamaProvider::new(config).unwrap();
        assert_eq!(provider.model, "gemma3");
    }
}
