use ratatui::style::{Color, Modifier, Style};

// ── Base colors ─────────────────────────────────────────────────────────────

pub const BG: Color = Color::Reset;
pub const FG: Color = Color::White;
pub const DIM: Color = Color::DarkGray;
pub const ACCENT: Color = Color::Cyan;
pub const SUCCESS: Color = Color::Green;
pub const WARNING: Color = Color::Yellow;
pub const ERROR: Color = Color::Red;

// ── Status bar ──────────────────────────────────────────────────────────────

pub const STATUS_BG: Color = Color::DarkGray;
pub const STATUS_FG: Color = Color::White;

pub fn status_style() -> Style {
    Style::default().fg(STATUS_FG).bg(STATUS_BG)
}

// ── Input box ───────────────────────────────────────────────────────────────

pub const INPUT_BORDER: Color = Color::Cyan;
pub const INPUT_BORDER_INACTIVE: Color = Color::DarkGray;

pub fn input_border_style(active: bool) -> Style {
    if active {
        Style::default().fg(INPUT_BORDER)
    } else {
        Style::default().fg(INPUT_BORDER_INACTIVE)
    }
}

// ── Output area ─────────────────────────────────────────────────────────────

pub fn user_message_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn assistant_text_style() -> Style {
    Style::default().fg(FG)
}

pub fn error_style() -> Style {
    Style::default().fg(ERROR).add_modifier(Modifier::BOLD)
}

pub fn system_style() -> Style {
    Style::default().fg(DIM).add_modifier(Modifier::ITALIC)
}

pub fn tool_style() -> Style {
    Style::default().fg(WARNING)
}

// ── Markdown ────────────────────────────────────────────────────────────────

pub fn heading_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn bold_style() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn italic_style() -> Style {
    Style::default().add_modifier(Modifier::ITALIC)
}

pub fn code_inline_style() -> Style {
    Style::default().fg(Color::LightYellow)
}

// ── Diff ────────────────────────────────────────────────────────────────────

pub fn diff_add_style() -> Style {
    Style::default().fg(SUCCESS)
}

pub fn diff_remove_style() -> Style {
    Style::default().fg(ERROR)
}

pub fn accent_bullet_style() -> Style {
    Style::default().fg(ACCENT)
}

pub fn diff_context_style() -> Style {
    Style::default().fg(DIM)
}

// ── Confirm dialog ──────────────────────────────────────────────────────────

pub fn confirm_border_style() -> Style {
    Style::default().fg(WARNING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_styles_are_distinct() {
        assert_ne!(user_message_style(), assistant_text_style());
        assert_ne!(error_style(), system_style());
    }

    #[test]
    fn test_input_border_active_vs_inactive() {
        let active = input_border_style(true);
        let inactive = input_border_style(false);
        assert_ne!(active, inactive);
    }
}
