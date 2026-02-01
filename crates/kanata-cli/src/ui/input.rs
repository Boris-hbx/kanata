use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use super::theme;

/// Multi-line input box with editing and history support.
pub struct InputBox {
    content: String,
    cursor_pos: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    active: bool,
}

impl InputBox {
    /// Create an empty input box.
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_index: None,
            active: true,
        }
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
        self.history_index = None;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.content.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    /// Insert a newline at the cursor position.
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char_before(&mut self) {
        if self.cursor_pos > 0 {
            // Find the previous character boundary
            let prev = self.content[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.content.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    /// Delete the character at the cursor (delete key).
    pub fn delete_char_at(&mut self) {
        if self.cursor_pos < self.content.len() {
            let next = self.content[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map_or(self.content.len(), |(i, _)| self.cursor_pos + i);
            self.content.drain(self.cursor_pos..next);
        }
    }

    /// Move cursor left by one character.
    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.content[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
        }
    }

    /// Move cursor right by one character.
    pub fn move_right(&mut self) {
        if self.cursor_pos < self.content.len() {
            self.cursor_pos = self.content[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map_or(self.content.len(), |(i, _)| self.cursor_pos + i);
        }
    }

    /// Move cursor to the beginning of the line.
    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) {
        self.cursor_pos = self.content.len();
    }

    /// Clear from cursor to the beginning of the line (Ctrl+U).
    pub fn clear_to_start(&mut self) {
        self.content.drain(..self.cursor_pos);
        self.cursor_pos = 0;
    }

    /// Navigate to the previous history entry (Up arrow).
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(0) => return,
            Some(i) => i - 1,
            None => self.history.len() - 1,
        };
        self.history_index = Some(idx);
        self.content = self.history[idx].clone();
        self.cursor_pos = self.content.len();
    }

    /// Navigate to the next history entry (Down arrow).
    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };
        if idx + 1 < self.history.len() {
            let next = idx + 1;
            self.history_index = Some(next);
            self.content = self.history[next].clone();
            self.cursor_pos = self.content.len();
        } else {
            // Past the end → clear to empty
            self.history_index = None;
            self.content.clear();
            self.cursor_pos = 0;
        }
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

    /// Set whether the input box is active (focused).
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Whether the input box is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Handle a key event. Returns `true` if the event was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction {
        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.insert_newline();
                    InputAction::Consumed
                } else {
                    InputAction::Submit
                }
            }
            KeyCode::Backspace => {
                self.delete_char_before();
                InputAction::Consumed
            }
            KeyCode::Delete => {
                self.delete_char_at();
                InputAction::Consumed
            }
            KeyCode::Left => {
                self.move_left();
                InputAction::Consumed
            }
            KeyCode::Right => {
                self.move_right();
                InputAction::Consumed
            }
            KeyCode::Up => {
                self.history_prev();
                InputAction::Consumed
            }
            KeyCode::Down => {
                self.history_next();
                InputAction::Consumed
            }
            KeyCode::Home => {
                self.move_home();
                InputAction::Consumed
            }
            KeyCode::End => {
                self.move_end();
                InputAction::Consumed
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                InputAction::Quit
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_to_start();
                InputAction::Consumed
            }
            KeyCode::Char(ch) => {
                self.insert_char(ch);
                InputAction::Consumed
            }
            _ => InputAction::Ignored,
        }
    }

    /// Compute the cursor position within the rendered widget area.
    pub fn cursor_position(&self, area: Rect) -> (u16, u16) {
        // Account for the "> " prompt prefix and border
        let prompt_len = 2u16;
        let inner_x = area.x + 1 + prompt_len;
        let inner_y = area.y + 1;
        let cursor_offset = u16::try_from(self.cursor_pos).unwrap_or(0);
        let inner_width = area.width.saturating_sub(2 + prompt_len);
        if inner_width == 0 {
            return (inner_x, inner_y);
        }
        let x = inner_x + cursor_offset % inner_width;
        let y = inner_y + cursor_offset / inner_width;
        (x, y)
    }

    /// Render the input box as a ratatui widget into the given buffer.
    pub fn render_widget(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::input_border_style(self.active))
            .title(" Input ");

        let text = Line::from(vec![
            Span::styled("> ", theme::user_message_style()),
            Span::raw(&self.content),
        ]);

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }
}

/// The result of handling a key event in the input box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// The key was consumed (editing action).
    Consumed,
    /// The user pressed Enter to submit.
    Submit,
    /// The user pressed Ctrl+C to quit.
    Quit,
    /// The key was not handled.
    Ignored,
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

    #[test]
    fn test_backspace() {
        let mut input = InputBox::new();
        input.set_content("abc".to_string());
        input.delete_char_before();
        assert_eq!(input.content(), "ab");
    }

    #[test]
    fn test_delete() {
        let mut input = InputBox::new();
        input.set_content("abc".to_string());
        input.move_home();
        input.delete_char_at();
        assert_eq!(input.content(), "bc");
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = InputBox::new();
        input.set_content("abc".to_string());
        assert_eq!(input.cursor_pos, 3);
        input.move_left();
        assert_eq!(input.cursor_pos, 2);
        input.move_right();
        assert_eq!(input.cursor_pos, 3);
        input.move_home();
        assert_eq!(input.cursor_pos, 0);
        input.move_end();
        assert_eq!(input.cursor_pos, 3);
    }

    #[test]
    fn test_history_navigation() {
        let mut input = InputBox::new();
        input.set_content("first".to_string());
        input.submit();
        input.set_content("second".to_string());
        input.submit();

        input.history_prev();
        assert_eq!(input.content(), "second");
        input.history_prev();
        assert_eq!(input.content(), "first");
        input.history_next();
        assert_eq!(input.content(), "second");
        input.history_next();
        assert_eq!(input.content(), "");
    }

    #[test]
    fn test_clear_to_start() {
        let mut input = InputBox::new();
        input.set_content("hello".to_string());
        input.cursor_pos = 3;
        input.clear_to_start();
        assert_eq!(input.content(), "lo");
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_handle_key_char() {
        let mut input = InputBox::new();
        let action = input.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(action, InputAction::Consumed);
        assert_eq!(input.content(), "a");
    }

    #[test]
    fn test_handle_key_enter() {
        let mut input = InputBox::new();
        input.set_content("test".to_string());
        let action = input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(action, InputAction::Submit);
    }

    #[test]
    fn test_handle_key_ctrl_c() {
        let mut input = InputBox::new();
        let action = input.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(action, InputAction::Quit);
    }

    #[test]
    fn test_handle_key_shift_enter() {
        let mut input = InputBox::new();
        let action = input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
        assert_eq!(action, InputAction::Consumed);
        assert_eq!(input.content(), "\n");
    }
}
