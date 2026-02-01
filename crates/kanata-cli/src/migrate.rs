use std::path::Path;

use anyhow::Result;

/// Detect and run any necessary data migrations.
///
/// Checks for:
/// - `.claude/` directory → suggest importing project rules
/// - `.cursorrules` file → suggest importing Cursor rules
///
/// # Errors
///
/// Returns an error if a migration step fails.
pub fn detect_and_migrate() -> Result<()> {
    let cwd = std::env::current_dir()?;

    detect_claude_dir(&cwd);
    detect_cursor_rules(&cwd);

    Ok(())
}

/// Check for `.claude/` directory and inform the user.
fn detect_claude_dir(cwd: &Path) {
    let claude_dir = cwd.join(".claude");
    if claude_dir.is_dir() {
        tracing::info!("Detected .claude/ directory. Consider importing project rules to .kanata/");
    }
}

/// Check for `.cursorrules` file and inform the user.
fn detect_cursor_rules(cwd: &Path) {
    let cursor_rules = cwd.join(".cursorrules");
    if cursor_rules.is_file() {
        tracing::info!("Detected .cursorrules file. Consider importing Cursor rules to .kanata/");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_and_migrate_succeeds() {
        // Should not fail even if no migration files are present
        let result = detect_and_migrate();
        assert!(result.is_ok());
    }
}
