//! Bash tool — executes shell commands with safety checks and output truncation.

use async_trait::async_trait;
use serde_json::json;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

use crate::safety;

/// Maximum output bytes before truncation.
const MAX_OUTPUT_BYTES: usize = 30_000;

/// Default timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Executes shell commands with safety checks.
pub struct BashTool;

impl BashTool {
    /// Creates a new `BashTool`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Bash".to_string(),
            description: "Executes a shell command and returns the output.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 120, max: 600)."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    /// # Errors
    ///
    /// Returns `KanataError` if the command parameter is missing or execution fails.
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let command = input.get("command").and_then(|v| v.as_str()).ok_or_else(|| {
            KanataError::ToolError {
                tool_name: "Bash".to_string(),
                reason: "Missing required parameter: command".to_string(),
            }
        })?;

        let timeout_secs = input
            .get("timeout")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(600);

        // Safety check.
        if let Some(reason) = safety::check_dangerous_command(command) {
            return Ok(ToolResult { content: reason, is_error: true });
        }

        // Execute the command.
        let shell = if cfg!(target_os = "windows") { ("cmd", "/C") } else { ("sh", "-c") };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tokio::process::Command::new(shell.0).arg(shell.1).arg(command).output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut combined = String::new();
                if !stdout.is_empty() {
                    combined.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str("STDERR:\n");
                    combined.push_str(&stderr);
                }

                // Truncate if too long (at a valid UTF-8 boundary).
                if combined.len() > MAX_OUTPUT_BYTES {
                    let mut end = MAX_OUTPUT_BYTES;
                    while end > 0 && !combined.is_char_boundary(end) {
                        end -= 1;
                    }
                    combined.truncate(end);
                    combined.push_str("\n... (output truncated)");
                }

                let is_error = !output.status.success();
                if combined.is_empty() {
                    combined = if is_error {
                        format!("Command exited with status: {}", output.status)
                    } else {
                        "Command completed successfully (no output).".to_string()
                    };
                }

                Ok(ToolResult { content: combined, is_error })
            }
            Ok(Err(e)) => Err(KanataError::ToolError {
                tool_name: "Bash".to_string(),
                reason: format!("Failed to execute command: {e}"),
            }),
            Err(_) => Ok(ToolResult {
                content: format!("Command timed out after {timeout_secs}s"),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_echo() {
        let tool = BashTool::new();
        let result = tool.execute(json!({ "command": "echo hello" })).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_blocks_dangerous_command() {
        let tool = BashTool::new();
        let result = tool.execute(json!({ "command": "rm -rf /" })).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Blocked"));
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool::new();
        let cmd = if cfg!(target_os = "windows") { "ping -n 10 127.0.0.1" } else { "sleep 10" };
        let result = tool.execute(json!({ "command": cmd, "timeout": 1 })).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("timed out"));
    }

    #[tokio::test]
    async fn test_bash_missing_command_returns_error() {
        let tool = BashTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }
}
