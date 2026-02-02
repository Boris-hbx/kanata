//! Model router: selects the appropriate provider based on model name.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use kanata_types::llm::{LLMClient, StreamEvent};
use kanata_types::message::Message;
use kanata_types::tool::ToolDefinition;
use kanata_types::{KanataConfig, KanataError};

use crate::providers::anthropic::AnthropicClient;
use crate::providers::gemini::GeminiClient;
use crate::providers::openai::OpenAIClient;

/// Routes LLM requests to the appropriate provider based on model name.
pub struct ModelRouter {
    inner: Box<dyn LLMClient>,
}

impl ModelRouter {
    /// Create a router from configuration, using the default model.
    ///
    /// # Errors
    ///
    /// Returns `KanataError::Config` if the API key is missing or the model
    /// is unsupported.
    pub fn from_config(config: &KanataConfig) -> Result<Self, KanataError> {
        Self::for_model(&config.default_model, config)
    }

    /// Create a router for a specific model name.
    ///
    /// # Errors
    ///
    /// Returns `KanataError::Config` if the API key is missing or the model
    /// is unsupported.
    pub fn for_model(model: &str, config: &KanataConfig) -> Result<Self, KanataError> {
        let inner: Box<dyn LLMClient> = if model.starts_with("claude") {
            let keys = get_keys(config, "anthropic")?;
            Box::new(AnthropicClient::new(keys, model))
        } else if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
            let keys = get_keys(config, "openai")?;
            Box::new(OpenAIClient::new(
                keys,
                model,
                "https://api.openai.com/v1",
            ))
        } else if model.starts_with("deepseek") {
            let keys = get_keys(config, "deepseek")?;
            Box::new(OpenAIClient::new(
                keys,
                model,
                "https://api.deepseek.com/v1",
            ))
        } else if model.starts_with("gemini") {
            let keys = get_keys(config, "google")?;
            Box::new(GeminiClient::new(keys, model))
        } else if model.starts_with("grok") {
            let keys = get_keys(config, "xai")?;
            Box::new(OpenAIClient::new(
                keys,
                model,
                "https://api.x.ai/v1",
            ))
        } else if model.starts_with("qwen") {
            let keys = get_keys(config, "qwen")?;
            Box::new(OpenAIClient::new(
                keys,
                model,
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ))
        } else {
            return Err(KanataError::Config(format!(
                "unsupported model: {model}"
            )));
        };

        Ok(Self { inner })
    }
}

fn get_keys(config: &KanataConfig, provider: &str) -> Result<Vec<String>, KanataError> {
    let key = config
        .api_keys
        .get(provider)
        .ok_or_else(|| KanataError::Config(format!("missing API key for provider: {provider}")))?
        .clone();
    // Support comma-separated keys for pool
    let keys: Vec<String> = key.split(',').map(|s| s.trim().to_string()).collect();
    Ok(keys)
}

#[async_trait]
impl LLMClient for ModelRouter {
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        self.inner.chat_stream(messages, tools, system).await
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}
