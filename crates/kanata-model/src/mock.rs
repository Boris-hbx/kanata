//! Mock LLM client for development and testing.
//!
//! Returns preset streaming responses so that `kanata-core` can be developed
//! and tested without a real API key.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use futures::stream;

use kanata_types::KanataError;
use kanata_types::llm::{LLMClient, StreamEvent, TokenUsage};
use kanata_types::message::Message;
use kanata_types::tool::ToolDefinition;

/// A mock LLM client that returns a canned text response.
pub struct MockLLMClient {
    model: String,
    /// The preset text response to return from `chat_stream`.
    response_text: String,
}

impl MockLLMClient {
    /// Creates a new mock client with a default response.
    pub fn new() -> Self {
        Self {
            model: "mock-model".to_string(),
            response_text: "Hello! I am a mock LLM response.".to_string(),
        }
    }

    /// Creates a mock client that returns the given text.
    pub fn with_response(response_text: impl Into<String>) -> Self {
        Self { model: "mock-model".to_string(), response_text: response_text.into() }
    }
}

impl Default for MockLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for MockLLMClient {
    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        let events = vec![
            Ok(StreamEvent::TextDelta(self.response_text.clone())),
            Ok(StreamEvent::MessageEnd {
                usage: TokenUsage {
                    input_tokens: 10,
                    output_tokens: 20,
                    model: self.model.clone(),
                    ..TokenUsage::default()
                },
            }),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// A mock LLM client that returns a tool-use response followed by text.
pub struct MockToolUseLLMClient {
    model: String,
    /// Tool name to invoke.
    tool_name: String,
    /// JSON input for the tool.
    tool_input_json: String,
    /// Whether the next call should return tool use or final text.
    /// Uses interior mutability for the trait signature.
    returned_tool: std::sync::atomic::AtomicBool,
}

impl MockToolUseLLMClient {
    /// Creates a mock that first returns a tool call, then a text response.
    pub fn new(tool_name: impl Into<String>, tool_input_json: impl Into<String>) -> Self {
        Self {
            model: "mock-model".to_string(),
            tool_name: tool_name.into(),
            tool_input_json: tool_input_json.into(),
            returned_tool: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl LLMClient for MockToolUseLLMClient {
    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        let already_called = self.returned_tool.swap(true, std::sync::atomic::Ordering::SeqCst);

        let events = if already_called {
            // Second call: return final text after tool result
            vec![
                Ok(StreamEvent::TextDelta("Done! I used the tool successfully.".to_string())),
                Ok(StreamEvent::MessageEnd {
                    usage: TokenUsage {
                        input_tokens: 30,
                        output_tokens: 15,
                        model: self.model.clone(),
                        ..TokenUsage::default()
                    },
                }),
            ]
        } else {
            // First call: return tool use
            vec![
                Ok(StreamEvent::ToolUseStart {
                    id: "tool_001".to_string(),
                    name: self.tool_name.clone(),
                }),
                Ok(StreamEvent::ToolUseDelta(self.tool_input_json.clone())),
                Ok(StreamEvent::ToolUseEnd),
                Ok(StreamEvent::MessageEnd {
                    usage: TokenUsage {
                        input_tokens: 20,
                        output_tokens: 10,
                        model: self.model.clone(),
                        ..TokenUsage::default()
                    },
                }),
            ]
        };
        Ok(Box::pin(stream::iter(events)))
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn test_mock_llm_returns_text_delta() {
        let client = MockLLMClient::new();
        let stream = client.chat_stream(&[], &[], "").await.expect("stream");
        let events: Vec<_> =
            stream.collect::<Vec<_>>().await.into_iter().filter_map(Result::ok).collect();
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], StreamEvent::TextDelta(_)));
        assert!(matches!(&events[1], StreamEvent::MessageEnd { .. }));
    }

    #[tokio::test]
    async fn test_mock_tool_use_client_returns_tool_then_text() {
        let client = MockToolUseLLMClient::new("Read", r#"{"path":"/tmp/test.rs"}"#);

        // First call: tool use
        let events: Vec<_> = client
            .chat_stream(&[], &[], "")
            .await
            .expect("stream")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect();
        assert!(matches!(&events[0], StreamEvent::ToolUseStart { .. }));

        // Second call: text
        let events: Vec<_> = client
            .chat_stream(&[], &[], "")
            .await
            .expect("stream")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect();
        assert!(matches!(&events[0], StreamEvent::TextDelta(_)));
    }
}
