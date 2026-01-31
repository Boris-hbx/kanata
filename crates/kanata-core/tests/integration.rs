//! Integration tests for the full agent loop.
//!
//! These tests exercise the Agent with mock LLMs and real tools, verifying
//! end-to-end behavior including tool dispatch and multi-turn conversation.

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use futures::{Stream, StreamExt, stream};

use kanata_core::Agent;
use kanata_types::error::KanataError;
use kanata_types::llm::{LLMClient, StreamEvent, TokenUsage};
use kanata_types::message::Message;
use kanata_types::session::{AgentEvent, AgentSession};
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

/// A mock LLM that returns a simple text response.
struct TextOnlyLLM;

#[async_trait]
impl LLMClient for TextOnlyLLM {
    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        Ok(Box::pin(stream::iter(vec![
            Ok(StreamEvent::TextDelta("I can help with that.".into())),
            Ok(StreamEvent::MessageEnd {
                usage: TokenUsage {
                    input_tokens: 5,
                    output_tokens: 8,
                    model: "test".into(),
                    ..TokenUsage::default()
                },
            }),
        ])))
    }

    fn model_name(&self) -> &'static str {
        "test-text"
    }
}

/// A mock LLM that first requests a tool call, then returns text.
struct ToolThenTextLLM {
    call_count: AtomicUsize,
    tool_name: String,
    tool_input: String,
}

impl ToolThenTextLLM {
    fn new(tool_name: &str, tool_input: &str) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            tool_name: tool_name.into(),
            tool_input: tool_input.into(),
        }
    }
}

#[async_trait]
impl LLMClient for ToolThenTextLLM {
    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        let n = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events = if n == 0 {
            vec![
                Ok(StreamEvent::ToolUseStart {
                    id: "call_001".into(),
                    name: self.tool_name.clone(),
                }),
                Ok(StreamEvent::ToolUseDelta(self.tool_input.clone())),
                Ok(StreamEvent::ToolUseEnd),
                Ok(StreamEvent::MessageEnd {
                    usage: TokenUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                        model: "test".into(),
                        ..TokenUsage::default()
                    },
                }),
            ]
        } else {
            vec![
                Ok(StreamEvent::TextDelta("Done with tool.".into())),
                Ok(StreamEvent::MessageEnd {
                    usage: TokenUsage {
                        input_tokens: 20,
                        output_tokens: 10,
                        model: "test".into(),
                        ..TokenUsage::default()
                    },
                }),
            ]
        };
        Ok(Box::pin(stream::iter(events)))
    }

    fn model_name(&self) -> &'static str {
        "test-tool"
    }
}

/// An LLM that always requests a tool call (never returns text), for depth-limit testing.
struct InfiniteToolLLM;

#[async_trait]
impl LLMClient for InfiniteToolLLM {
    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        Ok(Box::pin(stream::iter(vec![
            Ok(StreamEvent::ToolUseStart { id: "inf".into(), name: "echo".into() }),
            Ok(StreamEvent::ToolUseDelta(r#"{"text":"hi"}"#.into())),
            Ok(StreamEvent::ToolUseEnd),
            Ok(StreamEvent::MessageEnd { usage: TokenUsage::default() }),
        ])))
    }

    fn model_name(&self) -> &'static str {
        "test-infinite"
    }
}

/// A trivial echo tool that returns its input.
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".into(),
            description: "Returns the input text.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("(empty)");
        Ok(ToolResult { content: text.to_string(), is_error: false })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Basic text conversation: user sends message, agent replies with text.
#[tokio::test]
async fn test_basic_text_conversation() {
    let agent = Agent::new(Box::new(TextOnlyLLM), vec![], "You are helpful.");
    let stream = agent.send_message("Hello").await.expect("send");
    let events: Vec<_> = stream.collect().await;

    // Should have: Thinking, TextDelta, Done
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Thinking)));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TextDelta(t) if t.contains("help"))));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Done { .. })));

    // Stats should reflect one turn.
    let stats = agent.stats();
    assert_eq!(stats.turns, 1);
    assert!(stats.total_input_tokens > 0);
    assert!(stats.total_output_tokens > 0);
}

/// Multi-turn conversation: two sequential messages accumulate stats.
#[tokio::test]
async fn test_multi_turn_conversation() {
    let agent = Agent::new(Box::new(TextOnlyLLM), vec![], "");
    let _ = agent.send_message("first").await.expect("send");
    let _ = agent.send_message("second").await.expect("send");

    let stats = agent.stats();
    assert_eq!(stats.turns, 2);
    // Two turns with 5 input tokens each.
    assert_eq!(stats.total_input_tokens, 10);
}

/// Agent dispatches a tool call and returns tool events.
#[tokio::test]
async fn test_tool_use_round_trip() {
    let llm = ToolThenTextLLM::new("echo", r#"{"text":"ping"}"#);
    let tools: Vec<Box<dyn Tool>> = vec![Box::new(EchoTool)];
    let agent = Agent::new(Box::new(llm), tools, "");

    let stream = agent.send_message("echo ping").await.expect("send");
    let events: Vec<_> = stream.collect().await;

    // Should contain ToolStart and ToolEnd events.
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::ToolStart { name, .. } if name == "echo"))
    );
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolEnd { name, .. } if name == "echo")));
    // Should also have the follow-up text.
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TextDelta(t) if t.contains("Done"))));
}

/// Unknown tool returns an error result but doesn't crash the agent.
#[tokio::test]
async fn test_unknown_tool_handled_gracefully() {
    let llm = ToolThenTextLLM::new("nonexistent_tool", r"{}");
    let agent = Agent::new(Box::new(llm), vec![], "");

    let stream = agent.send_message("try it").await.expect("send");
    let events: Vec<_> = stream.collect().await;

    // Should have a ToolEnd with error info in the result preview.
    assert!(events.iter().any(|e| matches!(
        e,
        AgentEvent::ToolEnd {
            result_preview, ..
        } if result_preview.contains("Unknown tool")
    )));
}

/// Agent stops after `MAX_TOOL_TURNS` to prevent infinite loops.
#[tokio::test]
async fn test_depth_limit_prevents_infinite_loop() {
    let tools: Vec<Box<dyn Tool>> = vec![Box::new(EchoTool)];
    let agent = Agent::new(Box::new(InfiniteToolLLM), tools, "");

    let stream = agent.send_message("loop forever").await.expect("send");
    let events: Vec<_> = stream.collect().await;

    // Should hit the max depth error.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Error(msg) if msg.contains("maximum tool recursion")))
    );

    // Count ToolStart events — should be exactly MAX_TOOL_TURNS (20).
    let tool_starts = events.iter().filter(|e| matches!(e, AgentEvent::ToolStart { .. })).count();
    assert_eq!(tool_starts, 20);
}

/// Agent with a real `ReadTool` can read a temp file.
#[tokio::test]
async fn test_agent_with_read_tool() {
    use std::io::Write;

    let tmp = tempfile::NamedTempFile::new().expect("tmp");
    writeln!(tmp.as_file(), "hello from file").expect("write");
    let path = tmp.path().to_string_lossy().to_string();

    let input = format!(r#"{{"path":"{path}"}}"#).replace('\\', "\\\\");
    let llm = ToolThenTextLLM::new("Read", &input);
    let tools: Vec<Box<dyn Tool>> = vec![Box::new(kanata_tools::ReadTool::new())];
    let agent = Agent::new(Box::new(llm), tools, "");

    let stream = agent.send_message("read the file").await.expect("send");
    let events: Vec<_> = stream.collect().await;

    // Tool should have been called and should contain file content in preview.
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolEnd { result_preview, .. } if result_preview.contains("hello from file"))));
}
