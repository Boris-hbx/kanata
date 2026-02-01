use ratatui::text::{Line, Span};

use super::theme;

/// Parse a markdown string into styled ratatui `Line`s.
///
/// Supports: headings, bold, italic, inline code, code blocks, and lists.
pub fn parse_markdown(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();

    for line in text.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End code block — render accumulated lines
                for code_line in &code_block_lines {
                    lines.push(Line::from(Span::styled(
                        format!("  {code_line}"),
                        theme::code_inline_style(),
                    )));
                }
                code_block_lines.clear();
                in_code_block = false;
            } else {
                // Start code block
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_block_lines.push(line.to_string());
            continue;
        }

        // Heading
        if let Some(heading) = line.strip_prefix("### ") {
            lines.push(Line::from(Span::styled(format!("   {heading}"), theme::heading_style())));
        } else if let Some(heading) = line.strip_prefix("## ") {
            lines.push(Line::from(Span::styled(format!("  {heading}"), theme::heading_style())));
        } else if let Some(heading) = line.strip_prefix("# ") {
            lines.push(Line::from(Span::styled(heading.to_string(), theme::heading_style())));
        } else if line.starts_with("- ") || line.starts_with("* ") {
            // List item
            let content = &line[2..];
            lines.push(Line::from(vec![
                Span::styled("  • ", theme::accent_bullet_style()),
                Span::raw(content.to_string()),
            ]));
        } else {
            // Inline formatting
            lines.push(parse_inline(line));
        }
    }

    // Flush any unclosed code block
    for code_line in &code_block_lines {
        lines.push(Line::from(Span::styled(format!("  {code_line}"), theme::code_inline_style())));
    }

    lines
}

/// Parse inline markdown formatting (bold, italic, inline code).
fn parse_inline(text: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Inline code: `...`
        if chars[i] == '`' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            let start = i + 1;
            i = start;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            let code_text: String = chars[start..i].iter().collect();
            spans.push(Span::styled(code_text, theme::code_inline_style()));
            if i < len {
                i += 1; // skip closing `
            }
            continue;
        }

        // Bold: **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            let start = i + 2;
            i = start;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                i += 1;
            }
            let bold_text: String = chars[start..i].iter().collect();
            spans.push(Span::styled(bold_text, theme::bold_style()));
            if i + 1 < len {
                i += 2; // skip closing **
            }
            continue;
        }

        // Italic: *...*
        if chars[i] == '*' {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            let start = i + 1;
            i = start;
            while i < len && chars[i] != '*' {
                i += 1;
            }
            let italic_text: String = chars[start..i].iter().collect();
            spans.push(Span::styled(italic_text, theme::italic_style()));
            if i < len {
                i += 1; // skip closing *
            }
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let lines = parse_markdown("# Hello World");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_parse_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let lines = parse_markdown(input);
        assert_eq!(lines.len(), 1); // one code line
    }

    #[test]
    fn test_parse_list() {
        let lines = parse_markdown("- item one\n- item two");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_parse_inline_bold() {
        let line = parse_inline("hello **world**");
        assert!(line.spans.len() >= 2);
    }

    #[test]
    fn test_parse_inline_code() {
        let line = parse_inline("use `println!`");
        assert!(line.spans.len() >= 2);
    }

    #[test]
    fn test_parse_empty() {
        let lines = parse_markdown("");
        assert!(lines.is_empty());
    }

    #[test]
    fn test_parse_plain_text() {
        let lines = parse_markdown("just plain text");
        assert_eq!(lines.len(), 1);
    }
}
