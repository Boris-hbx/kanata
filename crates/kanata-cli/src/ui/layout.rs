use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Three-zone layout: output area, status bar, input box.
pub struct AppLayout {
    /// Scrollable conversation output area.
    pub output: Rect,
    /// Single-line status bar.
    pub status: Rect,
    /// Multi-line input box.
    pub input: Rect,
}

impl AppLayout {
    /// Compute the three-zone layout for the given terminal area.
    ///
    /// ```text
    /// ┌─────── Output Area ──────┐  Min(5)
    /// ├─────── Status Bar ───────┤  Length(1)
    /// ├─────── Input Box  ───────┤  Length(3)
    /// └──────────────────────────┘
    /// ```
    pub fn new(area: Rect) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(1), Constraint::Length(3)])
            .split(area);

        Self { output: chunks[0], status: chunks[1], input: chunks[2] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_split() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = AppLayout::new(area);

        // Status bar should be exactly 1 row
        assert_eq!(layout.status.height, 1);
        // Input box should be exactly 3 rows
        assert_eq!(layout.input.height, 3);
        // Output takes the rest
        assert_eq!(layout.output.height, 20);
        // All widths match the terminal
        assert_eq!(layout.output.width, 80);
        assert_eq!(layout.status.width, 80);
        assert_eq!(layout.input.width, 80);
    }

    #[test]
    fn test_layout_small_terminal() {
        let area = Rect::new(0, 0, 40, 10);
        let layout = AppLayout::new(area);

        assert_eq!(layout.status.height, 1);
        assert_eq!(layout.input.height, 3);
        assert_eq!(layout.output.height, 6);
    }
}
