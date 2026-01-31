#![allow(dead_code)]

/// Renders streamed assistant output with syntax highlighting and markdown.
///
/// Will be backed by syntect and termimad for rich terminal rendering
/// in a later phase.
pub struct OutputView {
    lines: Vec<String>,
    scroll_offset: usize,
}

impl OutputView {
    /// Create an empty output view.
    pub fn new() -> Self {
        Self { lines: Vec::new(), scroll_offset: 0 }
    }

    /// Append text (possibly multi-line) to the output buffer.
    pub fn append_text(&mut self, text: &str) {
        for line in text.lines() {
            self.lines.push(line.to_string());
        }
    }

    /// Clear all output.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }

    /// Return the total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

impl Default for OutputView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_append_and_count() {
        let mut view = OutputView::new();
        assert_eq!(view.line_count(), 0);

        view.append_text("line one\nline two");
        assert_eq!(view.line_count(), 2);

        view.clear();
        assert_eq!(view.line_count(), 0);
    }
}
