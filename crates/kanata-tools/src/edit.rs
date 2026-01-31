//! Edit tool — performs precise string replacements in files.

use async_trait::async_trait;
use serde_json::json;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

/// Performs exact string replacement in a file.
pub struct EditTool;

impl EditTool {
    /// Creates a new `EditTool`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Edit".to_string(),
            description: "Performs exact string replacements in files.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The absolute path to the file to edit."
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact text to find and replace."
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement text."
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false)."
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }

    /// # Errors
    ///
    /// Returns `KanataError` if parameters are missing, the file is not found, or I/O fails.
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let path = require_str(&input, "path", "Edit")?;
        let old_string = require_str(&input, "old_string", "Edit")?;
        let new_string = require_str(&input, "new_string", "Edit")?;
        let replace_all =
            input.get("replace_all").and_then(serde_json::Value::as_bool).unwrap_or(false);

        let content = tokio::fs::read_to_string(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => KanataError::FileNotFound { path: path.to_string() },
            _ => KanataError::Io(e),
        })?;

        let count = content.matches(old_string).count();
        if count == 0 {
            return Ok(ToolResult {
                content: format!("old_string not found in {path}"),
                is_error: true,
            });
        }

        if !replace_all && count > 1 {
            return Ok(ToolResult {
                content: format!(
                    "old_string found {count} times in {path}. \
                     Provide more context to make it unique, or set replace_all: true."
                ),
                is_error: true,
            });
        }

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        tokio::fs::write(path, &new_content).await?;

        let replaced = if replace_all { count } else { 1 };
        Ok(ToolResult {
            content: format!("Replaced {replaced} occurrence(s) in {path}"),
            is_error: false,
        })
    }
}

/// Extracts a required string parameter or returns a `ToolError`.
fn require_str<'a>(
    input: &'a serde_json::Value,
    key: &str,
    tool_name: &str,
) -> Result<&'a str, KanataError> {
    input.get(key).and_then(|v| v.as_str()).ok_or_else(|| KanataError::ToolError {
        tool_name: tool_name.to_string(),
        reason: format!("Missing required parameter: {key}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_edit_replaces_string() {
        let dir = std::env::temp_dir().join("kanata_test_edit");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let file = dir.join("test.txt");
        tokio::fs::write(&file, "hello world").await.unwrap();

        let tool = EditTool::new();
        let result = tool
            .execute(json!({
                "path": file.to_str().unwrap(),
                "old_string": "world",
                "new_string": "rust"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);

        let content = tokio::fs::read_to_string(&file).await.unwrap();
        assert_eq!(content, "hello rust");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_edit_ambiguous_match_returns_error() {
        let dir = std::env::temp_dir().join("kanata_test_edit_ambig");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let file = dir.join("dup.txt");
        tokio::fs::write(&file, "aaa bbb aaa").await.unwrap();

        let tool = EditTool::new();
        let result = tool
            .execute(json!({
                "path": file.to_str().unwrap(),
                "old_string": "aaa",
                "new_string": "ccc"
            }))
            .await
            .unwrap();
        assert!(result.is_error);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let dir = std::env::temp_dir().join("kanata_test_edit_all");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let file = dir.join("rep.txt");
        tokio::fs::write(&file, "aaa bbb aaa").await.unwrap();

        let tool = EditTool::new();
        let result = tool
            .execute(json!({
                "path": file.to_str().unwrap(),
                "old_string": "aaa",
                "new_string": "ccc",
                "replace_all": true
            }))
            .await
            .unwrap();
        assert!(!result.is_error);

        let content = tokio::fs::read_to_string(&file).await.unwrap();
        assert_eq!(content, "ccc bbb ccc");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
