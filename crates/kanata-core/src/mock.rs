//! Mock agent session for CLI development and testing.
//!
//! Allows Dev A to build the full TUI without depending on a real agent or LLM.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use futures::stream;

use kanata_types::error::KanataError;
use kanata_types::session::{AgentEvent, AgentSession, SessionTokenStats};

/// A mock agent session that returns canned events.
pub struct MockAgentSession {
    stats: SessionTokenStats,
}

impl MockAgentSession {
    /// Creates a new mock session.
    pub fn new() -> Self {
        Self {
            stats: SessionTokenStats {
                model: "mock-model".to_string(),
                ..SessionTokenStats::default()
            },
        }
    }
}

impl Default for MockAgentSession {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentSession for MockAgentSession {
    async fn send_message(
        &self,
        content: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, KanataError> {
        let response = format!("Mock response to: {content}");
        let events = vec![
            AgentEvent::Thinking,
            AgentEvent::TextDelta(response),
            AgentEvent::Done {
                usage: SessionTokenStats {
                    total_input_tokens: 10,
                    total_output_tokens: 20,
                    total_cost_usd: 0.001,
                    turns: 1,
                    model: "mock-model".to_string(),
                },
            },
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn stats(&self) -> SessionTokenStats {
        self.stats.clone()
    }

    async fn execute_command(&self, cmd: &str, _args: &str) -> Result<String, KanataError> {
        Ok(format!("Mock executed: {cmd}"))
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn test_mock_session_returns_events() {
        let session = MockAgentSession::new();
        let stream = session.send_message("hello").await.expect("stream");
        let events: Vec<_> = stream.collect().await;

        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], AgentEvent::Thinking));
        assert!(matches!(&events[1], AgentEvent::TextDelta(t) if t.contains("hello")));
        assert!(matches!(&events[2], AgentEvent::Done { .. }));
    }

    #[tokio::test]
    async fn test_mock_session_execute_command() {
        let session = MockAgentSession::new();
        let result = session.execute_command("/help", "").await.expect("cmd");
        assert!(result.contains("/help"));
    }
}
