#![allow(dead_code)]

/// Multi-line input box with editing and history support.
///
/// Will be integrated with crossterm key events and ratatui rendering
/// in a later phase.
pub struct InputBox {
    content: String,
    cursor_pos: usize,
    history: Vec<String>,
}

impl InputBox {
    /// Create an empty input box.
    pub fn new() -> Self {
        Self { content: String::new(), cursor_pos: 0, history: Vec::new() }
    }

    /// Return the current content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Replace the entire content and move cursor to end.
    pub fn set_content(&mut self, content: String) {
        self.cursor_pos = content.len();
        self.content = content;
    }

    /// Clear all content and reset cursor.
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_pos = 0;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.content.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    /// Submit the current content: push to history and return it.
    pub fn submit(&mut self) -> String {
        let text = self.content.clone();
        if !text.is_empty() {
            self.history.push(text.clone());
        }
        self.clear();
        text
    }
}

impl Default for InputBox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_box_insert_and_submit() {
        let mut input = InputBox::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.content(), "hi");

        let submitted = input.submit();
        assert_eq!(submitted, "hi");
        assert_eq!(input.content(), "");
    }

    #[test]
    fn test_input_box_set_content() {
        let mut input = InputBox::new();
        input.set_content("hello world".to_string());
        assert_eq!(input.content(), "hello world");
    }
}
