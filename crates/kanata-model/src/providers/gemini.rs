//! Google Gemini API client with SSE streaming.

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

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Google Gemini API streaming client.
pub struct GeminiClient {
    client: Client,
    key_pool: KeyPool,
    model: String,
}

impl GeminiClient {
    /// Create a new Gemini client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built.
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

    fn build_tools(tools: &[ToolDefinition]) -> Option<Value> {
        if tools.is_empty() {
            return None;
        }
        let declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                })
            })
            .collect();
        Some(json!([{"functionDeclarations": declarations}]))
    }

    fn build_contents(messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    kanata_types::message::Role::User => "user",
                    kanata_types::message::Role::Assistant => "model",
                };
                json!({
                    "role": role,
                    "parts": [{"text": msg.content}],
                })
            })
            .collect()
    }
}

#[async_trait]
impl LLMClient for GeminiClient {
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

        let url = format!(
            "{}/{}:streamGenerateContent?alt=sse&key={}",
            BASE_URL, self.model, api_key
        );

        let mut body = json!({
            "contents": Self::build_contents(messages),
        });

        if !system.is_empty() {
            body["systemInstruction"] = json!({
                "parts": [{"text": system}]
            });
        }

        if let Some(tool_defs) = Self::build_tools(tools) {
            body["tools"] = tool_defs;
        }

        debug!(model = %self.model, "sending Gemini streaming request");

        let resp = with_retry(|| {
            let body = body.clone();
            let client = self.client.clone();
            let url = url.clone();
            async move {
                let resp = client
                    .post(&url)
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
            GeminiSseState {
                byte_stream: Box::pin(byte_stream),
                buffer: String::new(),
                model,
                done: false,
                input_tokens: 0,
                output_tokens: 0,
                pending_tool_args: None,
                pending_tool_end: false,
            },
            |mut state| async move {
                if state.done {
                    return None;
                }
                loop {
                    if let Some(event) = parse_gemini_sse(&mut state) {
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
                            // End of stream
                            if state.input_tokens > 0 || state.output_tokens > 0 {
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

struct GeminiSseState {
    byte_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: String,
    model: String,
    done: bool,
    input_tokens: u32,
    output_tokens: u32,
    pending_tool_args: Option<String>,
    pending_tool_end: bool,
}

/// Parse the next SSE data line from Gemini's streaming response.
fn parse_gemini_sse(state: &mut GeminiSseState) -> Option<Result<StreamEvent, KanataError>> {
    // Drain pending tool events from a previous function call chunk
    if let Some(args) = state.pending_tool_args.take() {
        return Some(Ok(StreamEvent::ToolUseDelta(args)));
    }
    if state.pending_tool_end {
        state.pending_tool_end = false;
        return Some(Ok(StreamEvent::ToolUseEnd));
    }

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

        let chunk: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Extract usage metadata
        if let Some(metadata) = chunk.get("usageMetadata") {
            if let Some(it) = metadata.get("promptTokenCount").and_then(Value::as_u64) {
                state.input_tokens = u32::try_from(it).unwrap_or(u32::MAX);
            }
            if let Some(ot) = metadata.get("candidatesTokenCount").and_then(Value::as_u64) {
                state.output_tokens = u32::try_from(ot).unwrap_or(u32::MAX);
            }
        }

        // Extract content from candidates
        let candidates = chunk.get("candidates")?.as_array()?;
        let candidate = candidates.first()?;
        let content = candidate.get("content")?;
        let parts = content.get("parts")?.as_array()?;

        for part in parts {
            // Text response
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                return Some(Ok(StreamEvent::TextDelta(text.to_string())));
            }

            // Function call — Gemini returns full call in one chunk.
            // We decompose it into ToolUseStart → ToolUseDelta → ToolUseEnd
            // but can only emit one event per parse call, so we emit ToolUseStart
            // and stash the args + pending end flag in the state for subsequent calls.
            if let Some(fc) = part.get("functionCall") {
                let name = fc.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let args = fc.get("args").map(Value::to_string).unwrap_or_default();
                state.pending_tool_args = Some(args);
                state.pending_tool_end = true;
                return Some(Ok(StreamEvent::ToolUseStart {
                    id: format!("gemini_{name}"),
                    name,
                }));
            }
        }

        // Check finish reason
        if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
            trace!(reason, "Gemini finish reason");
            if reason == "STOP" {
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
        }
    }
}
