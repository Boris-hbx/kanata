//! Glob tool — fast file pattern matching.

use std::path::Path;

use async_trait::async_trait;
use serde_json::json;
use walkdir::WalkDir;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

/// Finds files matching a glob pattern within a directory.
pub struct GlobTool;

impl GlobTool {
    /// Creates a new `GlobTool`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Glob".to_string(),
            description: "Fast file pattern matching using glob patterns.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match (e.g. \"**/*.rs\")."
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in. Defaults to current directory."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    /// # Errors
    ///
    /// Returns `KanataError` if the pattern parameter is missing or the glob is invalid.
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let pattern = input.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| {
            KanataError::ToolError {
                tool_name: "Glob".to_string(),
                reason: "Missing required parameter: pattern".to_string(),
            }
        })?;
        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| KanataError::ToolError {
                tool_name: "Glob".to_string(),
                reason: format!("Invalid glob pattern: {e}"),
            })?
            .compile_matcher();

        let base = Path::new(search_path);
        let mut matches = Vec::new();

        for entry in WalkDir::new(base).follow_links(false).into_iter().filter_map(Result::ok) {
            let rel = entry.path().strip_prefix(base).unwrap_or(entry.path());
            if glob.is_match(rel) {
                matches.push(entry.path().to_string_lossy().to_string());
            }
        }

        matches.sort();

        if matches.is_empty() {
            Ok(ToolResult {
                content: format!("No files matched pattern: {pattern}"),
                is_error: false,
            })
        } else {
            Ok(ToolResult { content: matches.join("\n"), is_error: false })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_glob_finds_rust_files() {
        // Create a temp dir with known files for a deterministic test.
        let dir = std::env::temp_dir().join("kanata_test_glob");
        let _ = tokio::fs::create_dir_all(dir.join("sub")).await;
        tokio::fs::write(dir.join("a.rs"), "fn main() {}").await.unwrap();
        tokio::fs::write(dir.join("sub/b.rs"), "fn helper() {}").await.unwrap();
        tokio::fs::write(dir.join("readme.md"), "# Readme").await.unwrap();

        let tool = GlobTool::new();
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "path": dir.to_str().unwrap()
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("a.rs"));
        assert!(result.content.contains("b.rs"));
        assert!(!result.content.contains("readme.md"));

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tool = GlobTool::new();
        let result = tool
            .execute(json!({
                "pattern": "**/*.nonexistent_extension",
                "path": "."
            }))
            .await
            .unwrap();
        assert!(result.content.contains("No files matched"));
    }
}
