/// Handle a slash command and return output text.
pub fn handle_command(cmd: &str, _args: &str) -> String {
    match cmd.trim().to_lowercase().as_str() {
        "help" | "h" => help_text().to_string(),
        "exit" | "quit" | "q" => "Goodbye!".to_string(),
        "clear" => "Conversation cleared.".to_string(),
        "config" => "Config display not yet implemented.".to_string(),
        "model" => "Model management not yet implemented.".to_string(),
        other => format!("Unknown command: /{other}. Type /help for available commands."),
    }
}

/// Returns `true` if the command should cause the application to exit.
pub fn is_exit_command(cmd: &str) -> bool {
    matches!(cmd.trim().to_lowercase().as_str(), "exit" | "quit" | "q")
}

fn help_text() -> &'static str {
    "\
Available commands:
  /help, /h       Show this help message
  /exit, /quit    Exit Kanata
  /clear          Clear conversation history
  /config         Show current configuration
  /model          Show or change model"
}
