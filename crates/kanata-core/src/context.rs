//! Context assembler — manages message history and token window.

use kanata_types::message::Message;

/// Manages the conversation context window.
///
/// Tracks messages and ensures the total token count stays within the model's
/// context limit by truncating older messages when necessary.
pub struct ContextAssembler {
    /// Maximum tokens allowed in the context window.
    max_tokens: u32,
    /// Approximate tokens per message (rough heuristic: 4 chars ≈ 1 token).
    chars_per_token: u32,
}

impl ContextAssembler {
    /// Creates a new context assembler with the given token limit.
    pub fn new(max_tokens: u32) -> Self {
        Self { max_tokens, chars_per_token: 4 }
    }

    /// Estimates the token count for a message.
    ///
    /// Uses the raw text length of the content, stripping JSON syntax overhead.
    pub fn estimate_tokens(&self, message: &Message) -> u32 {
        let text_len = extract_text_length(&message.content);
        text_len / self.chars_per_token + 1
    }

    /// Estimates total tokens for a list of messages.
    pub fn estimate_total_tokens(&self, messages: &[Message]) -> u32 {
        messages.iter().map(|m| self.estimate_tokens(m)).sum()
    }

    /// Truncates the message list to fit within the token budget.
    ///
    /// Keeps the first message (often system context) and the most recent
    /// messages, dropping older ones from the middle. Emits a warning log
    /// if the remaining messages still exceed the budget.
    pub fn truncate_to_fit(&self, messages: &mut Vec<Message>) {
        while messages.len() > 2 && self.estimate_total_tokens(messages) > self.max_tokens {
            messages.remove(1);
        }

        if self.estimate_total_tokens(messages) > self.max_tokens {
            tracing::warn!(
                estimated = self.estimate_total_tokens(messages),
                limit = self.max_tokens,
                remaining = messages.len(),
                "Context still exceeds token budget after truncation"
            );
        }
    }

    /// Returns the configured maximum token count.
    pub fn max_tokens(&self) -> u32 {
        self.max_tokens
    }
}

impl Default for ContextAssembler {
    fn default() -> Self {
        Self::new(100_000)
    }
}

/// Extracts the approximate text length from a message content value.
///
/// For strings, returns the string length directly (without JSON quotes).
/// For arrays (tool results), sums up the text fields inside.
#[allow(clippy::cast_possible_truncation)]
fn extract_text_length(content: &serde_json::Value) -> u32 {
    match content {
        serde_json::Value::String(s) => s.len() as u32,
        serde_json::Value::Array(arr) => {
            let mut total = 0u32;
            for item in arr {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    total += text.len() as u32;
                }
                if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                    total += content.len() as u32;
                }
            }
            total
        }
        other => other.to_string().len() as u32,
    }
}

#[cfg(test)]
mod tests {
    use kanata_types::message::Role;

    use super::*;

    fn make_msg(text: &str) -> Message {
        Message { role: Role::User, content: serde_json::Value::String(text.to_string()) }
    }

    #[test]
    fn test_estimate_tokens() {
        let ctx = ContextAssembler::new(1000);
        let msg = make_msg("hello world!"); // 12 chars => ~4 tokens
        assert!(ctx.estimate_tokens(&msg) > 0);
    }

    #[test]
    fn test_estimate_tokens_no_json_overhead() {
        let ctx = ContextAssembler::new(1000);
        let msg = make_msg("hi");
        // "hi" is 2 chars => 2/4 + 1 = 1 token (not inflated by JSON quotes)
        assert_eq!(ctx.estimate_tokens(&msg), 1);
    }

    #[test]
    fn test_truncate_removes_old_messages() {
        let ctx = ContextAssembler::new(20);
        let mut messages = vec![
            make_msg("first message with some content"),
            make_msg("second message with some content"),
            make_msg("third message with some content"),
            make_msg("latest message"),
        ];
        ctx.truncate_to_fit(&mut messages);
        assert!(messages.len() < 4);
        assert!(messages.first().unwrap().content.to_string().contains("first"));
        assert!(messages.last().unwrap().content.to_string().contains("latest"));
    }

    #[test]
    fn test_no_truncation_when_under_limit() {
        let ctx = ContextAssembler::new(100_000);
        let mut messages = vec![make_msg("short"), make_msg("msg")];
        ctx.truncate_to_fit(&mut messages);
        assert_eq!(messages.len(), 2);
    }
}
