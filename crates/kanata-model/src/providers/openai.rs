//! OpenAI-compatible API client with SSE streaming.

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

/// OpenAI-compatible streaming client.
pub struct OpenAIClient {
    client: Client,
    key_pool: KeyPool,
    model: String,
    base_url: String,
}

impl OpenAIClient {
    /// Create a new OpenAI-compatible client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built.
    pub fn new(
        api_keys: Vec<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
            key_pool: KeyPool::new(api_keys),
            model: model.into(),
            base_url: base_url.into(),
        }
    }

    fn build_tools(tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
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

        let mut api_messages: Vec<Value> = Vec::new();
        if !system.is_empty() {
            api_messages.push(json!({"role": "system", "content": system}));
        }
        for msg in messages {
            api_messages.push(serde_json::to_value(msg).unwrap_or_default());
        }

        let mut body = json!({
            "model": self.model,
            "stream": true,
            "stream_options": {"include_usage": true},
            "messages": api_messages,
        });

        let tool_defs = Self::build_tools(tools);
        if !tool_defs.is_empty() {
            body["tools"] = json!(tool_defs);
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        debug!(model = %self.model, %url, "sending OpenAI-compatible streaming request");

        let resp = with_retry(|| {
            let body = body.clone();
            let api_key = api_key.clone();
            let client = self.client.clone();
            let url = url.clone();
            async move {
                let resp = client
                    .post(&url)
                    .bearer_auth(&api_key)
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
            OaiSseState {
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
                    if let Some(event) = parse_next_data_line(&mut state) {
                        if matches!(&event, Ok(StreamEvent::MessageEnd { .. })) {
                            state.done = true;
                        }
                        return Some((event, state));
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
                            state.done = true;
                            return Some((
                                Ok(StreamEvent::MessageEnd {
                                    usage: TokenUsage {
                                        input_tokens: state.input_tokens,
                                        output_tokens: state.output_tokens,
                                        model: state.model.clone(),
                                        cost_usd: cost_usd(
                                            &state.model,
                                            state.input_tokens,
                                            state.output_tokens,
                                        ),
                                        ..TokenUsage::default()
                                    },
                                }),
                                state,
                            ));
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

struct OaiSseState {
    byte_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: String,
    model: String,
    done: bool,
    input_tokens: u32,
    output_tokens: u32,
}

/// Parse the next `data: ...` line from the buffer.
fn parse_next_data_line(state: &mut OaiSseState) -> Option<Result<StreamEvent, KanataError>> {
    loop {
        let newline_pos = state.buffer.find('\n')?;
        let line = state.buffer[..newline_pos].to_string();
        state.buffer = state.buffer[newline_pos + 1..].to_string();

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };

        if data == "[DONE]" {
            return Some(Ok(StreamEvent::MessageEnd {
                usage: TokenUsage {
                    input_tokens: state.input_tokens,
                    output_tokens: state.output_tokens,
                    model: state.model.clone(),
                    cost_usd: cost_usd(&state.model, state.input_tokens, state.output_tokens),
                    ..TokenUsage::default()
                },
            }));
        }

        let chunk: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Extract usage if present
        if let Some(usage) = chunk.get("usage") {
            if let Some(it) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                state.input_tokens = u32::try_from(it).unwrap_or(u32::MAX);
            }
            if let Some(ot) = usage.get("completion_tokens").and_then(Value::as_u64) {
                state.output_tokens = u32::try_from(ot).unwrap_or(u32::MAX);
            }
        }

        let choices = chunk.get("choices")?.as_array()?;
        let choice = choices.first()?;
        let delta = choice.get("delta")?;

        // Text content
        if let Some(content) = delta.get("content").and_then(|c| c.as_str())
            && !content.is_empty()
        {
            return Some(Ok(StreamEvent::TextDelta(content.to_string())));
        }

        // Tool calls
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                if let Some(func) = tc.get("function") {
                    if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                        let id = tc
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("tool_0")
                            .to_string();
                        return Some(Ok(StreamEvent::ToolUseStart {
                            id,
                            name: name.to_string(),
                        }));
                    }
                    if let Some(args) = func.get("arguments").and_then(|a| a.as_str())
                        && !args.is_empty()
                    {
                        return Some(Ok(StreamEvent::ToolUseDelta(args.to_string())));
                    }
                }
            }
        }

        // finish_reason
        if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
            trace!(reason, "finish_reason");
            if reason == "tool_calls" {
                return Some(Ok(StreamEvent::ToolUseEnd));
            }
        }
    }
}
