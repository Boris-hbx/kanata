/// The result of executing a slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Display text to the user.
    Display(String),
    /// Exit the application.
    Exit,
    /// Clear the conversation and output.
    Clear,
    /// Switch to a different model.
    SwitchModel(String),
    /// Show token usage statistics.
    ShowStats,
    /// Unknown command.
    Unknown(String),
}

/// Handle a slash command and return a structured result.
pub fn handle_command(cmd: &str, args: &str) -> CommandResult {
    match cmd.trim().to_lowercase().as_str() {
        "help" | "h" => CommandResult::Display(help_text().to_string()),
        "exit" | "quit" | "q" => CommandResult::Exit,
        "clear" => CommandResult::Clear,
        "config" => CommandResult::Display(config_text()),
        "model" => {
            let name = args.trim();
            if name.is_empty() {
                CommandResult::Display(
                    "Usage: /model <name>. Currently using the configured default model."
                        .to_string(),
                )
            } else {
                CommandResult::SwitchModel(name.to_string())
            }
        }
        "cost" | "stats" => CommandResult::ShowStats,
        "undo" => CommandResult::Display("Undo not yet implemented.".to_string()),
        "history" => CommandResult::Display("Session history not yet implemented.".to_string()),
        other => CommandResult::Unknown(other.to_string()),
    }
}

/// Returns `true` if the command should cause the application to exit.
pub fn is_exit_command(cmd: &str) -> bool {
    matches!(cmd.trim().to_lowercase().as_str(), "exit" | "quit" | "q")
}

fn help_text() -> &'static str {
    "\
Available commands:
  /help, /h         Show this help message
  /exit, /quit, /q  Exit Kanata
  /clear            Clear conversation history
  /model [name]     Show or change model
  /cost, /stats     Show token usage and cost
  /undo             Undo last file operation
  /history          Show session history
  /config           Show current configuration"
}

fn config_text() -> String {
    "Configuration:\n  Config path: ~/.config/kanata/config.yaml\n  Use /model to view or change the active model.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_help() {
        let result = handle_command("help", "");
        assert!(
            matches!(result, CommandResult::Display(text) if text.contains("Available commands"))
        );
    }

    #[test]
    fn test_command_exit() {
        assert_eq!(handle_command("exit", ""), CommandResult::Exit);
        assert_eq!(handle_command("quit", ""), CommandResult::Exit);
        assert_eq!(handle_command("q", ""), CommandResult::Exit);
    }

    #[test]
    fn test_command_clear() {
        assert_eq!(handle_command("clear", ""), CommandResult::Clear);
    }

    #[test]
    fn test_command_model_no_args() {
        let result = handle_command("model", "");
        assert!(matches!(result, CommandResult::Display(_)));
    }

    #[test]
    fn test_command_model_with_args() {
        let result = handle_command("model", "gpt-4");
        assert_eq!(result, CommandResult::SwitchModel("gpt-4".to_string()));
    }

    #[test]
    fn test_command_stats() {
        assert_eq!(handle_command("cost", ""), CommandResult::ShowStats);
        assert_eq!(handle_command("stats", ""), CommandResult::ShowStats);
    }

    #[test]
    fn test_command_unknown() {
        let result = handle_command("foobar", "");
        assert_eq!(result, CommandResult::Unknown("foobar".to_string()));
    }

    #[test]
    fn test_is_exit_command() {
        assert!(is_exit_command("exit"));
        assert!(is_exit_command("quit"));
        assert!(is_exit_command("q"));
        assert!(!is_exit_command("help"));
    }

    #[test]
    fn test_command_config() {
        let result = handle_command("config", "");
        assert!(matches!(result, CommandResult::Display(text) if text.contains("Configuration")));
    }

    #[test]
    fn test_command_undo() {
        let result = handle_command("undo", "");
        assert!(matches!(result, CommandResult::Display(_)));
    }

    #[test]
    fn test_command_history() {
        let result = handle_command("history", "");
        assert!(matches!(result, CommandResult::Display(_)));
    }
}
