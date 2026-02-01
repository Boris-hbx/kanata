use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use super::theme;

/// A confirmation dialog for dangerous operations.
pub struct ConfirmDialog {
    message: String,
    visible: bool,
}

impl ConfirmDialog {
    /// Create a new hidden confirm dialog.
    pub fn new() -> Self {
        Self { message: String::new(), visible: false }
    }

    /// Show the dialog with the given message.
    pub fn show(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.visible = true;
    }

    /// Hide the dialog.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Whether the dialog is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Handle a key event. Returns `Some(true)` for confirm, `Some(false)` for cancel,
    /// `None` if the dialog is not visible or the key was not relevant.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.visible {
            return None;
        }
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                self.hide();
                Some(true)
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                self.hide();
                Some(false)
            }
            _ => None,
        }
    }

    /// Render the confirm dialog centered in the given area.
    pub fn render_widget(&self, area: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }

        // Calculate centered popup area
        let width = 50.min(area.width.saturating_sub(4));
        let height = 5.min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        // Clear the background
        Clear.render(popup_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::confirm_border_style())
            .title(" Confirm ");

        let text = vec![
            Line::from(Span::raw(&self.message)),
            Line::from(""),
            Line::from(Span::styled("[Y]es / [N]o", theme::bold_style())),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        paragraph.render(popup_area, buf);
    }
}

impl Default for ConfirmDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    #[test]
    fn test_confirm_dialog_lifecycle() {
        let mut dialog = ConfirmDialog::new();
        assert!(!dialog.is_visible());

        dialog.show("Delete file?");
        assert!(dialog.is_visible());

        dialog.hide();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn test_confirm_dialog_yes() {
        let mut dialog = ConfirmDialog::new();
        dialog.show("Proceed?");

        let result = dialog.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert_eq!(result, Some(true));
        assert!(!dialog.is_visible());
    }

    #[test]
    fn test_confirm_dialog_no() {
        let mut dialog = ConfirmDialog::new();
        dialog.show("Proceed?");

        let result = dialog.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert_eq!(result, Some(false));
        assert!(!dialog.is_visible());
    }

    #[test]
    fn test_confirm_dialog_escape() {
        let mut dialog = ConfirmDialog::new();
        dialog.show("Proceed?");

        let result = dialog.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_confirm_dialog_not_visible() {
        let mut dialog = ConfirmDialog::new();
        let result = dialog.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert_eq!(result, None);
    }
}
