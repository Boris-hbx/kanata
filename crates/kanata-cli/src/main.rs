use anyhow::Result;
use clap::Parser;

mod app;
mod commands;
mod config;
mod migrate;
mod session;
mod ui;

/// Kanata — next-generation AI Code Agent.
#[derive(Parser, Debug)]
#[command(name = "kanata", version, about = "Next-generation AI Code Agent")]
struct Cli {
    /// Run a single prompt non-interactively.
    #[arg(short, long)]
    prompt: Option<String>,

    /// Working directory override.
    #[arg(short = 'C', long)]
    directory: Option<String>,

    /// Enable verbose logging (debug level).
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async_main())
}

async fn async_main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing.
    let filter = if cli.verbose {
        tracing_subscriber::EnvFilter::new("debug")
    } else {
        tracing_subscriber::EnvFilter::try_from_env("KANATA_LOG")
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!("Kanata v{} starting", env!("CARGO_PKG_VERSION"));

    // Load configuration.
    let cfg = config::load_config()?;

    // Set working directory if specified.
    if let Some(dir) = &cli.directory {
        std::env::set_current_dir(dir)?;
    }

    // Run migration detection.
    migrate::detect_and_migrate()?;

    // Create session backed by the mock agent.
    let session = session::SessionManager::new(cfg)?;

    if let Some(prompt) = cli.prompt {
        // Non-interactive single-shot mode.
        session.send_once(&prompt).await?;
    } else {
        // Interactive REPL mode.
        let mut application = app::App::new(session);
        application.run().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use futures::StreamExt as _;
    use kanata_core::MockAgentSession;
    use kanata_types::{AgentEvent, AgentSession};

    use crate::{commands, config};

    #[test]
    fn test_default_config_loads() {
        let cfg = config::load_config().expect("default config should load");
        assert_eq!(cfg.default_model, "claude-sonnet-4");
        assert_eq!(cfg.trust_level, 1);
    }

    #[test]
    fn test_command_parsing_help() {
        let output = commands::handle_command("help", "");
        assert!(output.contains("Available commands"));
    }

    #[test]
    fn test_command_parsing_exit() {
        let output = commands::handle_command("exit", "");
        assert_eq!(output, "Goodbye!");
    }

    #[test]
    fn test_command_parsing_unknown() {
        let output = commands::handle_command("foobar", "");
        assert!(output.contains("Unknown command"));
    }

    #[tokio::test]
    async fn test_mock_session_send_message() {
        let session = MockAgentSession::new();
        let stream = session.send_message("hello").await.expect("send_message should succeed");
        let events: Vec<_> = stream.collect().await;

        let mut got_thinking = false;
        let mut got_text = false;
        let mut got_done = false;

        for event in &events {
            match event {
                AgentEvent::Thinking => got_thinking = true,
                AgentEvent::TextDelta(text) => {
                    assert!(text.contains("hello"));
                    got_text = true;
                }
                AgentEvent::Done { usage } => {
                    assert!(usage.total_input_tokens > 0);
                    got_done = true;
                }
                _ => {}
            }
        }

        assert!(got_thinking, "should receive Thinking event");
        assert!(got_text, "should receive TextDelta event");
        assert!(got_done, "should receive Done event");
    }

    #[tokio::test]
    async fn test_mock_session_execute_command() {
        let session = MockAgentSession::new();
        let result =
            session.execute_command("help", "").await.expect("execute_command should succeed");
        assert!(result.contains("help"));
    }

    #[test]
    fn test_session_manager_creation() {
        let cfg = kanata_types::KanataConfig::default();
        let manager = crate::session::SessionManager::new(cfg);
        assert!(manager.is_ok(), "SessionManager should be created");
    }

    #[test]
    fn test_commands_is_exit() {
        assert!(commands::is_exit_command("exit"));
        assert!(commands::is_exit_command("quit"));
        assert!(commands::is_exit_command("q"));
        assert!(!commands::is_exit_command("help"));
    }
}
