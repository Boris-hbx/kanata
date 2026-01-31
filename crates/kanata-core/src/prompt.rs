//! System prompt management — loads and assembles system prompts.

use std::path::Path;

use std::fmt::Write;

use kanata_types::tool::ToolDefinition;

/// Default system prompt used when no custom prompt file exists.
const DEFAULT_SYSTEM_PROMPT: &str = r"You are Kanata, an AI code assistant. You help users with software engineering tasks including reading, writing, and editing code, searching files, running commands, and more.

You have access to the following tools to interact with the user's codebase. Use them as needed to accomplish the task.";

/// Loads a system prompt from a file, falling back to the default.
pub fn load_system_prompt(prompt_dir: Option<&Path>) -> String {
    if let Some(dir) = prompt_dir {
        let system_file = dir.join("system.md");
        if system_file.exists()
            && let Ok(content) = std::fs::read_to_string(&system_file)
        {
            tracing::info!(path = %system_file.display(), "Loaded custom system prompt");
            return content;
        }
    }
    tracing::debug!("Using default system prompt");
    DEFAULT_SYSTEM_PROMPT.to_string()
}

/// Appends tool descriptions to a system prompt.
pub fn append_tool_descriptions(prompt: &str, tools: &[ToolDefinition]) -> String {
    if tools.is_empty() {
        return prompt.to_string();
    }

    let mut result = prompt.to_string();
    result.push_str("\n\n## Available Tools\n\n");

    for tool in tools {
        let _ = write!(result, "### {}\n{}\n\n", tool.name, tool.description);
    }

    result
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_load_default_prompt() {
        let prompt = load_system_prompt(None);
        assert!(prompt.contains("Kanata"));
    }

    #[test]
    fn test_load_custom_prompt() {
        let dir = std::env::temp_dir().join("kanata_test_prompt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("system.md"), "Custom prompt content").unwrap();

        let prompt = load_system_prompt(Some(&dir));
        assert_eq!(prompt, "Custom prompt content");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_append_tool_descriptions() {
        let prompt = "Base prompt.";
        let tools = vec![ToolDefinition {
            name: "Read".to_string(),
            description: "Reads a file.".to_string(),
            input_schema: json!({}),
        }];
        let result = append_tool_descriptions(prompt, &tools);
        assert!(result.contains("Base prompt."));
        assert!(result.contains("### Read"));
        assert!(result.contains("Reads a file."));
    }

    #[test]
    fn test_append_no_tools() {
        let prompt = "Base prompt.";
        let result = append_tool_descriptions(prompt, &[]);
        assert_eq!(result, "Base prompt.");
    }
}
