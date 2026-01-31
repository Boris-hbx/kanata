//! Grep tool — regex content search across files.

use std::path::Path;

use async_trait::async_trait;
use regex::Regex;
use serde_json::json;
use walkdir::WalkDir;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

/// Maximum number of matching lines to return.
const MAX_RESULTS: usize = 200;

/// Searches file contents using regular expressions.
pub struct GrepTool;

impl GrepTool {
    /// Creates a new `GrepTool`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Grep".to_string(),
            description: "Searches file contents using regular expressions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for."
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search in. Defaults to current directory."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Optional glob filter for files (e.g. \"*.rs\")."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    /// # Errors
    ///
    /// Returns `KanataError` if the pattern parameter is missing or the regex/glob is invalid.
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
        let pattern_str = input.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| {
            KanataError::ToolError {
                tool_name: "Grep".to_string(),
                reason: "Missing required parameter: pattern".to_string(),
            }
        })?;
        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let file_glob = input.get("glob").and_then(|v| v.as_str());

        let re = Regex::new(pattern_str).map_err(|e| KanataError::ToolError {
            tool_name: "Grep".to_string(),
            reason: format!("Invalid regex: {e}"),
        })?;

        let glob_matcher = file_glob
            .map(|g| {
                globset::GlobBuilder::new(g)
                    .literal_separator(false)
                    .build()
                    .map(|gb| gb.compile_matcher())
            })
            .transpose()
            .map_err(|e| KanataError::ToolError {
                tool_name: "Grep".to_string(),
                reason: format!("Invalid glob filter: {e}"),
            })?;

        let base = Path::new(search_path);
        let mut results = Vec::new();

        let entries: Vec<_> = WalkDir::new(base)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .collect();

        for entry in entries {
            if let Some(ref gm) = glob_matcher {
                let name = entry.file_name().to_string_lossy();
                if !gm.is_match(name.as_ref()) {
                    continue;
                }
            }

            // Skip binary files by checking first bytes.
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };

            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    let path_str = entry.path().to_string_lossy();
                    results.push(format!("{path_str}:{}: {line}", line_num + 1));
                    if results.len() >= MAX_RESULTS {
                        results.push(format!("... (truncated at {MAX_RESULTS} matches)"));
                        let output = results.join("\n");
                        return Ok(ToolResult { content: output, is_error: false });
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(ToolResult {
                content: format!("No matches found for pattern: {pattern_str}"),
                is_error: false,
            })
        } else {
            Ok(ToolResult { content: results.join("\n"), is_error: false })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grep_finds_pattern_in_source() {
        let tool = GrepTool::new();
        let result = tool
            .execute(json!({
                "pattern": "pub struct GrepTool",
                "path": "crates/kanata-tools/src"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("GrepTool"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let tool = GrepTool::new();
        let result = tool
            .execute(json!({
                "pattern": "ZZZZZ_NONEXISTENT_PATTERN_12345",
                "path": "crates/kanata-tools/src"
            }))
            .await
            .unwrap();
        assert!(result.content.contains("No matches"));
    }

    #[tokio::test]
    async fn test_grep_with_glob_filter() {
        let tool = GrepTool::new();
        let result = tool
            .execute(json!({
                "pattern": "fn definition",
                "path": "crates/kanata-tools/src",
                "glob": "*.rs"
            }))
            .await
            .unwrap();
        assert!(result.content.contains("fn definition"));
    }
}
