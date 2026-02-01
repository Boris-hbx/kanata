use kanata_types::SessionTokenStats;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::theme;

/// Bottom status bar showing token usage, model info, and current state.
pub struct StatusBar {
    model_name: String,
    token_stats: SessionTokenStats,
    status_text: String,
    spinner_char: String,
}

impl StatusBar {
    /// Create a status bar for the given model.
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            token_stats: SessionTokenStats::default(),
            status_text: "Ready".to_string(),
            spinner_char: String::new(),
        }
    }

    /// Update the displayed token statistics.
    pub fn update_stats(&mut self, stats: SessionTokenStats) {
        self.token_stats = stats;
    }

    /// Set the current status text (e.g. "Thinking...", "Ready").
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status_text = status.into();
    }

    /// Set the spinner animation character.
    pub fn set_spinner_char(&mut self, ch: &str) {
        self.spinner_char = ch.to_string();
    }

    /// Return the model name.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Return the current status text.
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Return a reference to the current token statistics.
    pub fn token_stats(&self) -> &SessionTokenStats {
        &self.token_stats
    }

    /// Render the status bar into the given buffer area.
    pub fn render_widget(&self, area: Rect, buf: &mut Buffer) {
        let model = &self.model_name;
        let spinner_part = if self.spinner_char.is_empty() {
            String::new()
        } else {
            format!(" {} ", self.spinner_char)
        };
        let status = &self.status_text;

        let tokens = self.token_stats.total_input_tokens + self.token_stats.total_output_tokens;
        let cost = self.token_stats.total_cost_usd;

        let right = if tokens > 0 { format!("{tokens}tok/${cost:.4}") } else { String::new() };

        // Calculate padding
        let left_text = format!(" {model}{spinner_part}{status}");
        let right_text = format!("{right} ");
        let width = area.width as usize;
        let padding = width.saturating_sub(left_text.len()).saturating_sub(right_text.len());

        let line = Line::from(vec![
            Span::styled(left_text, theme::status_style()),
            Span::styled(" ".repeat(padding), theme::status_style()),
            Span::styled(right_text, theme::status_style()),
        ]);

        let paragraph = Paragraph::new(line);
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_defaults() {
        let bar = StatusBar::new("mock-model");
        assert_eq!(bar.model_name(), "mock-model");
        assert_eq!(bar.status_text(), "Ready");
        assert_eq!(bar.token_stats().turns, 0);
    }

    #[test]
    fn test_status_bar_update() {
        let mut bar = StatusBar::new("test");
        bar.set_status("Thinking...");
        assert_eq!(bar.status_text(), "Thinking...");

        bar.update_stats(SessionTokenStats {
            total_input_tokens: 100,
            total_output_tokens: 200,
            total_cost_usd: 0.005,
            turns: 3,
            model: "test".to_string(),
        });
        assert_eq!(bar.token_stats().turns, 3);
    }

    #[test]
    fn test_status_bar_spinner_char() {
        let mut bar = StatusBar::new("test");
        assert!(bar.spinner_char.is_empty());
        bar.set_spinner_char("⠋");
        assert_eq!(bar.spinner_char, "⠋");
    }
}
