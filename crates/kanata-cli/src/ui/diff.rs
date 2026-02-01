use ratatui::text::{Line, Span};
use similar::{ChangeTag, TextDiff};

use super::theme;

/// Render a unified diff between two strings as styled ratatui `Line`s.
pub fn render_diff(old: &str, new: &str) -> Vec<Line<'static>> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();

    for change in diff.iter_all_changes() {
        let (sign, style) = match change.tag() {
            ChangeTag::Delete => ("-", theme::diff_remove_style()),
            ChangeTag::Insert => ("+", theme::diff_add_style()),
            ChangeTag::Equal => (" ", theme::diff_context_style()),
        };
        let text = change.as_str().unwrap_or("");
        // Trim trailing newline for display
        let text = text.strip_suffix('\n').unwrap_or(text);
        lines.push(Line::from(Span::styled(format!("{sign} {text}"), style)));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_diff_no_changes() {
        let lines = render_diff("hello\n", "hello\n");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_render_diff_addition() {
        let lines = render_diff("line1\n", "line1\nline2\n");
        // Should have context line + added line
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_render_diff_deletion() {
        let lines = render_diff("line1\nline2\n", "line1\n");
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_render_diff_modification() {
        let lines = render_diff("old line\n", "new line\n");
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_render_diff_empty() {
        let lines = render_diff("", "");
        assert!(lines.is_empty());
    }
}
