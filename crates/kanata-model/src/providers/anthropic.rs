//! Anthropic Messages API client with SSE streaming.

use std::pin::Pin;

use async_trait::async_trait;
use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, trace};

use kanata_types::llm::{LLMClient, StreamEvent, TokenUsage};
use kanata_types::message::Message;
use kanata_types::tool::ToolDefinition;
use kanata_types::KanataError;

use crate::key_pool::KeyPool;
use crate::retry::{with_retry, REQUEST_TIMEOUT};
use crate::token::cost_usd;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 16384;

/// Anthropic Messages API streaming client.
pub struct AnthropicClient {
    client: Client,
    key_pool: KeyPool,
    model: String,
}

impl AnthropicClient {
    /// Create a new Anthropic client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built (should never happen).
    pub fn new(api_keys: Vec<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
            key_pool: KeyPool::new(api_keys),
            model: model.into(),
        }
    }

    fn build_tools(tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect()
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, KanataError>> + Send>>, KanataError>
    {
        let api_key = self
            .key_pool
            .next_key()
            .ok_or_else(|| KanataError::Config("all API keys exhausted".into()))?
            .to_string();

        let mut body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "messages": messages,
        });

        if !system.is_empty() {
            body["system"] = json!(system);
        }

        let tool_defs = Self::build_tools(tools);
        if !tool_defs.is_empty() {
            body["tools"] = json!(tool_defs);
        }

        debug!(model = %self.model, "sending Anthropic streaming request");

        let resp = with_retry(|| {
            let body = body.clone();
            let api_key = api_key.clone();
            let client = self.client.clone();
            async move {
                let resp = client
                    .post(API_URL)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", API_VERSION)
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await?;
                Ok(resp)
            }
        })
        .await?;

        let model = self.model.clone();
        let byte_stream = resp.bytes_stream();

        let stream = futures::stream::unfold(
            SseState {
                byte_stream: Box::pin(byte_stream),
                buffer: String::new(),
                model,
                done: false,
                input_tokens: 0,
                output_tokens: 0,
            },
            |mut state| async move {
                if state.done {
                    return None;
                }
                loop {
                    if let Some(event) = parse_next_sse_event(&mut state.buffer) {
                        if let Some(stream_event) = handle_sse_event(&event, &mut state) {
                            if matches!(&stream_event, Ok(StreamEvent::MessageEnd { .. })) {
                                state.done = true;
                            }
                            return Some((stream_event, state));
                        }
                        continue;
                    }

                    match state.byte_stream.next().await {
                        Some(Ok(bytes)) => {
                            state.buffer.push_str(&String::from_utf8_lossy(&bytes));
                        }
                        Some(Err(e)) => {
                            state.done = true;
                            return Some((Err(KanataError::Http(e)), state));
                        }
                        None => {
                            return None;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(stream))
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

struct SseState {
    byte_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: String,
    model: String,
    done: bool,
    input_tokens: u32,
    output_tokens: u32,
}

struct SseEvent {
    event: Option<String>,
    data: String,
}

/// Try to parse the next complete SSE event from the buffer.
fn parse_next_sse_event(buffer: &mut String) -> Option<SseEvent> {
    let end = buffer.find("\n\n")?;
    let block = buffer[..end].to_string();
    *buffer = buffer[end + 2..].to_string();

    let mut event_type = None;
    let mut data_parts = Vec::new();

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event_type = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data: ") {
            data_parts.push(rest.to_string());
        }
    }

    Some(SseEvent {
        event: event_type,
        data: data_parts.join("\n"),
    })
}

/// Convert an SSE event into a `StreamEvent`, or `None` if it should be skipped.
fn handle_sse_event(sse: &SseEvent, state: &mut SseState) -> Option<Result<StreamEvent, KanataError>> {
    let event_type = sse.event.as_deref().unwrap_or("");
    trace!(event_type, "SSE event");

    match event_type {
        "content_block_start" => {
            let data: Value = serde_json::from_str(&sse.data).ok()?;
            let content_block = data.get("content_block")?;
            if content_block.get("type")?.as_str()? == "tool_use" {
                let id = content_block.get("id")?.as_str()?.to_string();
                let name = content_block.get("name")?.as_str()?.to_string();
                return Some(Ok(StreamEvent::ToolUseStart { id, name }));
            }
            None
        }
        "content_block_delta" => {
            let data: Value = serde_json::from_str(&sse.data).ok()?;
            let delta = data.get("delta")?;
            let delta_type = delta.get("type")?.as_str()?;
            match delta_type {
                "text_delta" => {
                    let text = delta.get("text")?.as_str()?.to_string();
                    Some(Ok(StreamEvent::TextDelta(text)))
                }
                "input_json_delta" => {
                    let partial = delta.get("partial_json")?.as_str()?.to_string();
                    Some(Ok(StreamEvent::ToolUseDelta(partial)))
                }
                _ => None,
            }
        }
        "content_block_stop" => Some(Ok(StreamEvent::ToolUseEnd)),
        "message_delta" => {
            let data: Value = serde_json::from_str(&sse.data).ok()?;
            if let Some(usage) = data.get("usage")
                && let Some(ot) = usage.get("output_tokens").and_then(Value::as_u64)
            {
                state.output_tokens = u32::try_from(ot).unwrap_or(u32::MAX);
            }
            None
        }
        "message_stop" => Some(Ok(StreamEvent::MessageEnd {
            usage: TokenUsage {
                input_tokens: state.input_tokens,
                output_tokens: state.output_tokens,
                model: state.model.clone(),
                cost_usd: cost_usd(&state.model, state.input_tokens, state.output_tokens),
                ..TokenUsage::default()
            },
        })),
        "message_start" => {
            let data: Value = serde_json::from_str(&sse.data).ok()?;
            if let Some(usage) = data.get("message").and_then(|m| m.get("usage"))
                && let Some(it) = usage.get("input_tokens").and_then(Value::as_u64)
            {
                state.input_tokens = u32::try_from(it).unwrap_or(u32::MAX);
            }
            None
        }
        "error" => {
            let data: Value = serde_json::from_str(&sse.data).ok()?;
            let msg = data
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error")
                .to_string();
            Some(Ok(StreamEvent::Error(msg)))
        }
        _ => None,
    }
}
