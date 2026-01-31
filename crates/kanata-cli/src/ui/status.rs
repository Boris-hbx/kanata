#![allow(dead_code)]

use kanata_types::SessionTokenStats;

/// Bottom status bar showing token usage, model info, and current state.
///
/// Will be rendered as a ratatui widget in a later phase.
pub struct StatusBar {
    model_name: String,
    token_stats: SessionTokenStats,
    status_text: String,
}

impl StatusBar {
    /// Create a status bar for the given model.
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            token_stats: SessionTokenStats::default(),
            status_text: "Ready".to_string(),
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
}
