use anyhow::Result;
use futures::StreamExt as _;
use kanata_types::{AgentEvent, AgentSession, KanataConfig, SessionTokenStats};

/// Manages the agent session lifecycle, bridging the CLI layer to core.
pub struct SessionManager {
    session: Box<dyn AgentSession>,
    #[allow(dead_code)]
    config: KanataConfig,
}

impl SessionManager {
    /// Create a new `SessionManager` backed by the mock agent session.
    ///
    /// # Errors
    ///
    /// Returns an error if session initialization fails.
    #[allow(clippy::unnecessary_wraps)]
    pub fn new(config: KanataConfig) -> Result<Self> {
        let session = kanata_core::MockAgentSession::new();
        Ok(Self { session: Box::new(session), config })
    }

    /// Send a single message and print the streamed response to stdout.
    pub async fn send_once(&self, content: &str) -> Result<()> {
        let stream = self.session.send_message(content).await?;
        futures::pin_mut!(stream);

        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::Thinking => {
                    println!("Thinking...");
                }
                AgentEvent::TextDelta(text) => {
                    print!("{text}");
                }
                AgentEvent::ToolStart { name, input_preview } => {
                    println!("[Tool: {name}] {input_preview}");
                }
                AgentEvent::ToolEnd { name, result_preview } => {
                    println!("[Tool: {name} done] {result_preview}");
                }
                AgentEvent::Done { usage } => {
                    println!();
                    println!(
                        "[Tokens: {} in / {} out | Cost: ${:.4}]",
                        usage.total_input_tokens, usage.total_output_tokens, usage.total_cost_usd
                    );
                }
                AgentEvent::Error(err) => {
                    eprintln!("Error: {err}");
                }
            }
        }

        Ok(())
    }

    /// Execute a slash command via the agent session.
    #[allow(dead_code)]
    pub async fn execute_command(&self, cmd: &str, args: &str) -> Result<String> {
        let output = self.session.execute_command(cmd, args).await?;
        Ok(output)
    }

    /// Get the current session token statistics.
    #[allow(dead_code)]
    pub fn stats(&self) -> SessionTokenStats {
        self.session.stats()
    }
}
