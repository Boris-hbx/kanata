//! Read tool — reads file contents and returns them to the agent.

use async_trait::async_trait;
use serde_json::json;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

/// Reads a file from disk and returns its contents.
pub struct ReadTool;

impl ReadTool {
    /// Creates a new `ReadTool`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Read".to_string(),
            description: "Reads a file from the local filesystem.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The absolute path to the file to read."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    /// # Errors
    ///
    /// Returns `KanataError` if the path parameter is missing or the file cannot be read.
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let path =
            input.get("path").and_then(|v| v.as_str()).ok_or_else(|| KanataError::ToolError {
                tool_name: "Read".to_string(),
                reason: "Missing required parameter: path".to_string(),
            })?;

        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(ToolResult { content, is_error: false }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(KanataError::FileNotFound { path: path.to_string() })
            }
            Err(e) => Err(KanataError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_existing_file_returns_content() {
        let dir = std::env::temp_dir().join("kanata_test_read");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let file = dir.join("hello.txt");
        tokio::fs::write(&file, "hello world").await.expect("write");

        let tool = ReadTool::new();
        let result = tool.execute(json!({ "path": file.to_str().unwrap() })).await.expect("read");
        assert!(!result.is_error);
        assert_eq!(result.content, "hello world");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_read_missing_file_returns_error() {
        let tool = ReadTool::new();
        let result = tool.execute(json!({ "path": "/nonexistent/path/to/file.txt" })).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_missing_path_param_returns_error() {
        let tool = ReadTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }
}
