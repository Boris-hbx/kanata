//! Agent main loop — orchestrates conversation between LLM and tools.

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use futures::stream;
use futures::{Stream, StreamExt};

use kanata_types::error::KanataError;
use kanata_types::llm::{LLMClient, StreamEvent, TokenUsage};
use kanata_types::message::{Message, Role};
use kanata_types::session::{AgentEvent, AgentSession, SessionTokenStats};
use kanata_types::tool::ToolDefinition;

use crate::dispatcher::ToolDispatcher;

/// Maximum number of consecutive tool-use turns before aborting.
const MAX_TOOL_TURNS: usize = 20;

/// The core agent that drives the conversation loop.
pub struct Agent {
    llm: Box<dyn LLMClient>,
    dispatcher: ToolDispatcher,
    messages: std::sync::Mutex<Vec<Message>>,
    system_prompt: String,
    stats: std::sync::Mutex<SessionTokenStats>,
}

impl Agent {
    /// Creates a new agent with the given LLM client and tools.
    pub fn new(
        llm: Box<dyn LLMClient>,
        tools: Vec<Box<dyn kanata_types::tool::Tool>>,
        system_prompt: impl Into<String>,
    ) -> Self {
        let model = llm.model_name().to_string();
        Self {
            llm,
            dispatcher: ToolDispatcher::new(tools),
            messages: std::sync::Mutex::new(Vec::new()),
            system_prompt: system_prompt.into(),
            stats: std::sync::Mutex::new(SessionTokenStats {
                model,
                ..SessionTokenStats::default()
            }),
        }
    }

    /// Returns tool definitions for all registered tools.
    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.dispatcher.definitions()
    }

    /// Accumulates token usage into session stats.
    fn accumulate_usage(&self, usage: &TokenUsage) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.total_input_tokens += usage.input_tokens;
            stats.total_output_tokens += usage.output_tokens;
            stats.total_cost_usd += usage.cost_usd;
        }
    }

    /// Runs one turn with a depth limit to prevent unbounded recursion.
    fn run_turn(
        &self,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<AgentEvent>, KanataError>> + Send + '_>> {
        Box::pin(self.run_turn_inner(depth))
    }

    /// Inner implementation of a single agent turn.
    async fn run_turn_inner(&self, depth: usize) -> Result<Vec<AgentEvent>, KanataError> {
        if depth >= MAX_TOOL_TURNS {
            return Ok(vec![AgentEvent::Error(format!(
                "Reached maximum tool recursion depth ({MAX_TOOL_TURNS})"
            ))]);
        }

        let mut events = vec![AgentEvent::Thinking];
        let tool_defs = self.tool_definitions();
        let messages = self
            .messages
            .lock()
            .map_err(|e| KanataError::Config(format!("Lock poisoned: {e}")))?
            .clone();

        let mut llm_stream =
            self.llm.chat_stream(&messages, &tool_defs, &self.system_prompt).await?;

        let mut text_accum = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_input = String::new();
        let mut pending_tool_calls: Vec<(String, String, String)> = Vec::new();
        let mut final_usage: Option<TokenUsage> = None;

        while let Some(event_result) = llm_stream.next().await {
            match event_result? {
                StreamEvent::TextDelta(text) => {
                    events.push(AgentEvent::TextDelta(text.clone()));
                    text_accum.push_str(&text);
                }
                StreamEvent::ToolUseStart { id, name } => {
                    current_tool_id = id;
                    current_tool_name = name;
                    current_tool_input.clear();
                }
                StreamEvent::ToolUseDelta(delta) => {
                    current_tool_input.push_str(&delta);
                }
                StreamEvent::ToolUseEnd => {
                    pending_tool_calls.push((
                        current_tool_id.clone(),
                        current_tool_name.clone(),
                        current_tool_input.clone(),
                    ));
                }
                StreamEvent::MessageEnd { usage } => {
                    final_usage = Some(usage);
                }
                StreamEvent::Error(e) => {
                    events.push(AgentEvent::Error(e));
                }
            }
        }

        // Build the assistant message with proper content blocks (Anthropic format).
        let assistant_content = build_assistant_content(&text_accum, &pending_tool_calls);
        if assistant_content != serde_json::Value::Null
            && let Ok(mut msgs) = self.messages.lock()
        {
            msgs.push(Message { role: Role::Assistant, content: assistant_content });
        }

        if let Some(usage) = &final_usage {
            self.accumulate_usage(usage);
        }

        // Execute any tool calls.
        if !pending_tool_calls.is_empty() {
            for (call_id, name, input_json) in &pending_tool_calls {
                events.push(AgentEvent::ToolStart {
                    name: name.clone(),
                    input_preview: truncate_string(input_json, 100),
                });

                let input: serde_json::Value = serde_json::from_str(input_json).unwrap_or_default();

                let result = match self.dispatcher.dispatch(name, input).await {
                    Ok(r) => r,
                    Err(e) => kanata_types::ToolResult { content: e.to_string(), is_error: true },
                };

                events.push(AgentEvent::ToolEnd {
                    name: name.clone(),
                    result_preview: truncate_string(&result.content, 200),
                });

                // Add tool result to message history for next turn.
                if let Ok(mut msgs) = self.messages.lock() {
                    msgs.push(Message {
                        role: Role::User,
                        content: serde_json::json!([{
                            "type": "tool_result",
                            "tool_use_id": call_id,
                            "content": result.content,
                            "is_error": result.is_error,
                        }]),
                    });
                }
            }

            // Recurse with incremented depth.
            let follow_up = self.run_turn(depth + 1).await?;
            events.extend(follow_up);
        }

        Ok(events)
    }
}

/// Builds an assistant content value in Anthropic Messages API format.
///
/// Returns an array of content blocks: text blocks and `tool_use` blocks.
fn build_assistant_content(
    text: &str,
    tool_calls: &[(String, String, String)],
) -> serde_json::Value {
    let mut blocks = Vec::new();

    if !text.is_empty() {
        blocks.push(serde_json::json!({
            "type": "text",
            "text": text,
        }));
    }

    for (id, name, input_json) in tool_calls {
        let input: serde_json::Value = serde_json::from_str(input_json).unwrap_or_default();
        blocks.push(serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }));
    }

    if blocks.is_empty() { serde_json::Value::Null } else { serde_json::Value::Array(blocks) }
}

/// Truncates a string to at most `max_len` bytes at a valid UTF-8 boundary,
/// appending `...` if truncated.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find the last valid char boundary at or before max_len.
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

#[async_trait]
impl AgentSession for Agent {
    async fn send_message(
        &self,
        content: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, KanataError> {
        // Add user message to history.
        if let Ok(mut msgs) = self.messages.lock() {
            msgs.push(Message {
                role: Role::User,
                content: serde_json::Value::String(content.to_string()),
            });
        }

        // Increment turn count.
        if let Ok(mut stats) = self.stats.lock() {
            stats.turns += 1;
        }

        let events = self.run_turn(0).await?;

        // Append Done event with current stats.
        let stats = self.stats();
        let mut all_events = events;
        all_events.push(AgentEvent::Done { usage: stats });

        Ok(Box::pin(stream::iter(all_events)))
    }

    fn stats(&self) -> SessionTokenStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }

    async fn execute_command(&self, cmd: &str, _args: &str) -> Result<String, KanataError> {
        match cmd {
            "/help" => Ok("Available commands: /help, /model, /cost, /clear".to_string()),
            _ => Ok(format!("Unknown command: {cmd}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    /// A simple mock LLM for agent tests (no kanata-model dependency needed).
    struct SimpleMockLLM;

    #[async_trait]
    impl kanata_types::llm::LLMClient for SimpleMockLLM {
        async fn chat_stream(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _system: &str,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
        {
            let events = vec![
                Ok(StreamEvent::TextDelta("Hello!".to_string())),
                Ok(StreamEvent::MessageEnd { usage: TokenUsage::default() }),
            ];
            Ok(Box::pin(stream::iter(events)))
        }

        fn model_name(&self) -> &'static str {
            "test-mock"
        }
    }

    #[tokio::test]
    async fn test_agent_send_message_returns_events() {
        let agent = Agent::new(Box::new(SimpleMockLLM), vec![], "You are helpful.");
        let stream = agent.send_message("hi").await.expect("send");
        let events: Vec<_> = stream.collect().await;

        assert!(events.iter().any(|e| matches!(e, AgentEvent::Thinking)));
        assert!(events.iter().any(|e| matches!(e, AgentEvent::TextDelta(_))));
        assert!(events.iter().any(|e| matches!(e, AgentEvent::Done { .. })));
    }

    #[tokio::test]
    async fn test_agent_stats_increments_turns() {
        let agent = Agent::new(Box::new(SimpleMockLLM), vec![], "");
        let _ = agent.send_message("first").await.expect("send");
        assert_eq!(agent.stats().turns, 1);
        let _ = agent.send_message("second").await.expect("send");
        assert_eq!(agent.stats().turns, 2);
    }

    #[tokio::test]
    async fn test_agent_execute_command_help() {
        let agent = Agent::new(Box::new(SimpleMockLLM), vec![], "");
        let result = agent.execute_command("/help", "").await.expect("cmd");
        assert!(result.contains("Available commands"));
    }

    #[test]
    fn test_truncate_string_ascii() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 5), "hello...");
    }

    #[test]
    fn test_truncate_string_utf8() {
        // "你好世界" is 12 bytes (3 bytes per CJK char).
        let s = "你好世界";
        let result = truncate_string(s, 7);
        // Should truncate at char boundary (6 bytes = 2 chars).
        assert!(result.ends_with("..."));
        assert!(!result.is_empty());
    }

    #[test]
    fn test_build_assistant_content_text_only() {
        let content = build_assistant_content("Hello!", &[]);
        let blocks = content.as_array().expect("array");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
    }

    #[test]
    fn test_build_assistant_content_with_tool_use() {
        let tools =
            vec![("tool_1".to_string(), "Read".to_string(), r#"{"path":"test.rs"}"#.to_string())];
        let content = build_assistant_content("Let me read that.", &tools);
        let blocks = content.as_array().expect("array");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "tool_use");
        assert_eq!(blocks[1]["name"], "Read");
    }

    #[test]
    fn test_build_assistant_content_empty() {
        let content = build_assistant_content("", &[]);
        assert!(content.is_null());
    }
}
