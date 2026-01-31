use std::io::Write as _;

use anyhow::Result;

use crate::commands;
use crate::session::SessionManager;

/// The main application state machine.
///
/// In the skeleton phase this runs a simple REPL loop.
/// It will be replaced with a full ratatui TUI in later phases.
pub struct App {
    session: SessionManager,
    should_quit: bool,
}

impl App {
    /// Create a new `App` backed by the given session.
    pub fn new(session: SessionManager) -> Self {
        Self { session, should_quit: false }
    }

    /// Run the interactive REPL loop.
    pub async fn run(&mut self) -> Result<()> {
        println!("Kanata v{} — interactive mode", env!("CARGO_PKG_VERSION"));
        println!("Type /help for available commands, /exit to quit.\n");

        let stdin = std::io::stdin();
        loop {
            print!("> ");
            std::io::stdout().flush()?;

            let mut input = String::new();
            stdin.read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if let Some(stripped) = input.strip_prefix('/') {
                let (cmd, args) = stripped.split_once(' ').map_or((stripped, ""), |(c, a)| (c, a));

                if commands::is_exit_command(cmd) {
                    println!("Goodbye!");
                    self.should_quit = true;
                } else {
                    let output = commands::handle_command(cmd, args);
                    println!("{output}");
                }
            } else {
                self.session.send_once(input).await?;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }
}
