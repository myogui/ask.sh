use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::fmt::Debug;

use crate::llm::{ChatResponse, Message};

use super::{ChatStream, LLMConfig, LLMError, LLMProvider};

#[derive(Debug)]
pub struct OpenAIProvider {
    client: Client<OpenAIConfig>,
    model: String,
    conversation_history: Vec<ChatCompletionRequestMessage>,
}

impl OpenAIProvider {
    pub fn new(config: LLMConfig) -> Result<Self, LLMError> {
        let mut openai_config = OpenAIConfig::new().with_api_key(config.api_key);

        // Set custom base_url if specified
        if let Some(base_url) = config.base_url {
            openai_config = openai_config.with_api_base(&base_url);
        }

        let client = Client::with_config(openai_config);

        Ok(Self {
            client,
            model: config.model,
            conversation_history: Vec::new(),
        })
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    /// Add a system message at the start of the conversation
    fn with_system_prompt(&mut self, prompt: &str) {
        let message = ChatCompletionRequestSystemMessageArgs::default()
            .content(prompt)
            .build()
            .expect("Failed to build system message")
            .into();

        self.conversation_history.push(message);
    }

    async fn chat_stream(&mut self, user_message: &Message) -> Result<ChatStream, LLMError> {
        // Add user message to history
        self.conversation_history.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(user_message.content.as_str())
                .build()
                .map_err(|e| LLMError::InvalidRequestError(e.to_string()))?
                .into(),
        );

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(self.conversation_history.clone())
            .build()
            .map_err(|e| LLMError::InvalidRequestError(e.to_string()))?;

        let stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .map_err(|e| LLMError::ApiError(e.to_string()))?;

        // Convert OpenAI stream to a stream using LLMError
        let mapped_stream = stream.map(|result| match result {
            Ok(response) => {
                let content = response
                    .choices
                    .iter()
                    .filter_map(|choice| choice.delta.content.as_ref())
                    .fold(String::new(), |mut acc, s| {
                        acc.push_str(s);
                        acc
                    });

                let chat_response = ChatResponse {
                    content: content,
                    tool_calls: None,
                };

                Ok(chat_response)
            }
            Err(err) => Err(LLMError::ApiError(err.to_string())),
        });

        Ok(Box::pin(mapped_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_openai_provider_creation() {
        let config = LLMConfig {
            provider: "openai".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            keep_alive: None,
            context_length: None,
        };

        let provider = OpenAIProvider::new(config).unwrap();
        assert_eq!(provider.model, "gpt-3.5-turbo");
    }
}
