use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use super::theme;

/// The kind of tool status to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStatusKind {
    /// Tool is currently executing.
    Running,
    /// Tool completed successfully.
    Done,
    /// Tool failed.
    Failed,
}

/// A single block of output in the conversation.
#[derive(Debug, Clone)]
pub enum OutputBlock {
    /// A user message.
    UserMessage(String),
    /// Assistant text response.
    AssistantText(String),
    /// Tool execution status.
    ToolStatus {
        /// Tool name.
        name: String,
        /// Current status.
        status: ToolStatusKind,
        /// Detail text (preview of input/output).
        detail: String,
    },
    /// An error message.
    Error(String),
    /// A system/informational message.
    System(String),
}

/// Renders streamed assistant output with support for multiple block types.
pub struct OutputView {
    blocks: Vec<OutputBlock>,
    scroll_offset: usize,
    streaming_buffer: String,
}

impl OutputView {
    /// Create an empty output view.
    pub fn new() -> Self {
        Self { blocks: Vec::new(), scroll_offset: 0, streaming_buffer: String::new() }
    }

    /// Append a complete output block.
    pub fn push_block(&mut self, block: OutputBlock) {
        self.blocks.push(block);
    }

    /// Append text to the streaming buffer (for incremental `TextDelta` events).
    pub fn append_streaming_text(&mut self, text: &str) {
        self.streaming_buffer.push_str(text);
    }

    /// Flush the streaming buffer into a finalized `AssistantText` block.
    pub fn flush_streaming(&mut self) {
        if !self.streaming_buffer.is_empty() {
            let text = std::mem::take(&mut self.streaming_buffer);
            self.blocks.push(OutputBlock::AssistantText(text));
        }
    }

    /// Append text (possibly multi-line) as a system message — backwards compat.
    pub fn append_text(&mut self, text: &str) {
        self.blocks.push(OutputBlock::System(text.to_string()));
    }

    /// Append an error message.
    pub fn append_error(&mut self, text: &str) {
        self.blocks.push(OutputBlock::Error(text.to_string()));
    }

    /// Clear all output.
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.streaming_buffer.clear();
        self.scroll_offset = 0;
    }

    /// Return the total number of output blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Return the total number of rendered lines (for scrolling).
    pub fn line_count(&self) -> usize {
        let mut count = 0;
        for block in &self.blocks {
            count += self.block_line_count(block);
        }
        if !self.streaming_buffer.is_empty() {
            count += self.streaming_buffer.lines().count().max(1);
        }
        count
    }

    /// Scroll up by the given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    /// Scroll down by the given number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Auto-scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Whether we're at the bottom (auto-scroll position).
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset == 0
    }

    /// Render the output view into the given buffer area.
    pub fn render_widget(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default().borders(Borders::NONE);

        let mut lines: Vec<Line<'_>> = Vec::new();

        for output_block in &self.blocks {
            self.render_block_to_lines(output_block, &mut lines);
            // Add a blank line between blocks
            lines.push(Line::from(""));
        }

        // Render the streaming buffer
        if !self.streaming_buffer.is_empty() {
            for line in self.streaming_buffer.lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    theme::assistant_text_style(),
                )));
            }
        }

        // Apply scroll offset (from bottom)
        let visible_height = area.height as usize;
        let total = lines.len();
        let scroll = if total > visible_height {
            (total - visible_height).saturating_sub(self.scroll_offset)
        } else {
            0
        };

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));

        paragraph.render(area, buf);
    }

    #[allow(clippy::unused_self)]
    fn block_line_count(&self, block: &OutputBlock) -> usize {
        match block {
            OutputBlock::UserMessage(text)
            | OutputBlock::AssistantText(text)
            | OutputBlock::Error(text)
            | OutputBlock::System(text) => text.lines().count().max(1) + 1, // +1 for spacing
            OutputBlock::ToolStatus { .. } => 2, // status line + spacing
        }
    }

    #[allow(clippy::unused_self)]
    fn render_block_to_lines<'a>(&self, block: &'a OutputBlock, lines: &mut Vec<Line<'a>>) {
        match block {
            OutputBlock::UserMessage(text) => {
                lines.push(Line::from(Span::styled(
                    format!("You: {text}"),
                    theme::user_message_style(),
                )));
            }
            OutputBlock::AssistantText(text) => {
                for line in text.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        theme::assistant_text_style(),
                    )));
                }
            }
            OutputBlock::ToolStatus { name, status, detail } => {
                let status_icon = match status {
                    ToolStatusKind::Running => "⟳",
                    ToolStatusKind::Done => "✓",
                    ToolStatusKind::Failed => "✗",
                };
                lines.push(Line::from(Span::styled(
                    format!("[{status_icon} {name}] {detail}"),
                    theme::tool_style(),
                )));
            }
            OutputBlock::Error(text) => {
                lines
                    .push(Line::from(Span::styled(format!("Error: {text}"), theme::error_style())));
            }
            OutputBlock::System(text) => {
                for line in text.lines() {
                    lines.push(Line::from(Span::styled(line.to_string(), theme::system_style())));
                }
            }
        }
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
        assert_eq!(view.block_count(), 0);

        view.append_text("line one\nline two");
        assert_eq!(view.block_count(), 1);

        view.clear();
        assert_eq!(view.block_count(), 0);
    }

    #[test]
    fn test_output_blocks() {
        let mut view = OutputView::new();
        view.push_block(OutputBlock::UserMessage("hello".to_string()));
        view.push_block(OutputBlock::AssistantText("world".to_string()));
        assert_eq!(view.block_count(), 2);
    }

    #[test]
    fn test_streaming_buffer() {
        let mut view = OutputView::new();
        view.append_streaming_text("hello ");
        view.append_streaming_text("world");
        assert_eq!(view.block_count(), 0);

        view.flush_streaming();
        assert_eq!(view.block_count(), 1);
    }

    #[test]
    fn test_scroll() {
        let mut view = OutputView::new();
        assert!(view.is_at_bottom());

        view.scroll_up(5);
        assert!(!view.is_at_bottom());
        assert_eq!(view.scroll_offset, 5);

        view.scroll_down(3);
        assert_eq!(view.scroll_offset, 2);

        view.scroll_to_bottom();
        assert!(view.is_at_bottom());
    }

    #[test]
    fn test_append_error() {
        let mut view = OutputView::new();
        view.append_error("something went wrong");
        assert_eq!(view.block_count(), 1);
    }
}
