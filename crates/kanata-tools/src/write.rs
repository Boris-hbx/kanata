//! Write tool — writes content to a file on disk.

use async_trait::async_trait;
use serde_json::json;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

/// Writes content to a file, creating parent directories as needed.
pub struct WriteTool;

impl WriteTool {
    /// Creates a new `WriteTool`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Write".to_string(),
            description: "Writes content to a file on the local filesystem.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The absolute path to the file to write."
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file."
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    /// # Errors
    ///
    /// Returns `KanataError` if parameters are missing or the file cannot be written.
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let path =
            input.get("path").and_then(|v| v.as_str()).ok_or_else(|| KanataError::ToolError {
                tool_name: "Write".to_string(),
                reason: "Missing required parameter: path".to_string(),
            })?;
        let content = input.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
            KanataError::ToolError {
                tool_name: "Write".to_string(),
                reason: "Missing required parameter: content".to_string(),
            }
        })?;

        // Create parent directories if they don't exist.
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(path, content).await?;

        Ok(ToolResult {
            content: format!("Successfully wrote {} bytes to {path}", content.len()),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_creates_file() {
        let dir = std::env::temp_dir().join("kanata_test_write");
        let file = dir.join("output.txt");

        let tool = WriteTool::new();
        let result = tool
            .execute(json!({
                "path": file.to_str().unwrap(),
                "content": "hello from write tool"
            }))
            .await
            .expect("write");
        assert!(!result.is_error);

        let content = tokio::fs::read_to_string(&file).await.expect("read back");
        assert_eq!(content, "hello from write tool");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = std::env::temp_dir().join("kanata_test_write_nested/a/b/c");
        let file = dir.join("deep.txt");

        let tool = WriteTool::new();
        let result = tool
            .execute(json!({
                "path": file.to_str().unwrap(),
                "content": "deep content"
            }))
            .await
            .expect("write nested");
        assert!(!result.is_error);

        let _ =
            tokio::fs::remove_dir_all(std::env::temp_dir().join("kanata_test_write_nested")).await;
    }

    #[tokio::test]
    async fn test_write_missing_content_returns_error() {
        let tool = WriteTool::new();
        let result = tool.execute(json!({ "path": "/tmp/x.txt" })).await;
        assert!(result.is_err());
    }
}
