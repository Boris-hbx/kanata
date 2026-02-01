use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::Stream;
use futures::StreamExt as _;
use kanata_types::{AgentEvent, AgentSession, KanataConfig, SessionTokenStats};
use serde::{Deserialize, Serialize};

/// Manages the agent session lifecycle, bridging the CLI layer to core.
pub struct SessionManager {
    session: Arc<dyn AgentSession>,
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
        Ok(Self { session: Arc::new(session), config })
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

    /// Send a message and return a raw stream of `AgentEvent`s for TUI consumption.
    pub async fn send_message_stream(
        &self,
        content: String,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>> {
        let stream = self.session.send_message(&content).await?;
        Ok(stream)
    }

    /// Return a cloneable handle to the underlying agent session.
    ///
    /// Useful for spawning async tasks that need to call `send_message()`.
    pub fn session_handle(&self) -> Arc<dyn AgentSession> {
        Arc::clone(&self.session)
    }

    /// Execute a slash command via the agent session.
    #[allow(dead_code)]
    pub async fn execute_command(&self, cmd: &str, args: &str) -> Result<String> {
        let output = self.session.execute_command(cmd, args).await?;
        Ok(output)
    }

    /// Get the current session token statistics.
    pub fn stats(&self) -> SessionTokenStats {
        self.session.stats()
    }

    /// Return the configured default model name.
    pub fn config_model(&self) -> &str {
        &self.config.default_model
    }

    /// Return a reference to the config.
    pub fn config(&self) -> &KanataConfig {
        &self.config
    }
}

/// Persisted session record for session history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Unique session identifier.
    pub id: String,
    /// Timestamp of session creation (ISO 8601).
    pub created_at: String,
    /// Last model used.
    pub model: String,
    /// Number of turns in the session.
    pub turns: u32,
    /// Total tokens used.
    pub total_tokens: u32,
    /// Total cost in USD.
    pub cost_usd: f64,
}

/// Return the directory for storing session records.
pub fn sessions_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("kanata").join("sessions")
}

/// Save a session record to disk.
pub fn save_session(record: &SessionRecord) -> Result<()> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", record.id));
    let content = serde_json::to_string_pretty(record)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// List all saved session records, sorted by creation date (newest first).
pub fn list_sessions() -> Result<Vec<SessionRecord>> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(record) = serde_json::from_str::<SessionRecord>(&content) {
                sessions.push(record);
            }
        }
    }
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_creation() {
        let cfg = KanataConfig::default();
        let manager = SessionManager::new(cfg).unwrap();
        assert_eq!(manager.config_model(), "claude-sonnet-4");
    }

    #[test]
    fn test_session_record_serialization() {
        let record = SessionRecord {
            id: "test-id".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            model: "claude-sonnet-4".to_string(),
            turns: 5,
            total_tokens: 1000,
            cost_usd: 0.05,
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: SessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.turns, 5);
    }

    #[test]
    fn test_sessions_dir() {
        let dir = sessions_dir();
        assert!(dir.to_str().unwrap().contains("kanata"));
    }
}
