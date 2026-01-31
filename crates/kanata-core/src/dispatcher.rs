//! Tool dispatcher — resolves tool names and dispatches execution.

use std::collections::HashMap;

use kanata_types::error::KanataError;
use kanata_types::tool::{Tool, ToolDefinition, ToolResult};

/// Dispatches tool invocations by name.
pub struct ToolDispatcher {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolDispatcher {
    /// Creates a dispatcher from a list of tools.
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        let map = tools.into_iter().map(|t| (t.definition().name.clone(), t)).collect();
        Self { tools: map }
    }

    /// Returns definitions for all registered tools.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Dispatches a tool call by name.
    ///
    /// # Errors
    ///
    /// Returns `KanataError` if the tool is not found or execution fails.
    pub async fn dispatch(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<ToolResult, KanataError> {
        let tool = self.tools.get(name).ok_or_else(|| KanataError::ToolError {
            tool_name: name.to_string(),
            reason: "Unknown tool".to_string(),
        })?;
        tool.execute(input).await
    }

    /// Returns the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns true if no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Minimal mock tool for dispatcher tests.
    struct EchoTool;

    #[async_trait::async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "Echo".to_string(),
                description: "Echoes input".to_string(),
                input_schema: json!({}),
            }
        }

        async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, KanataError> {
            Ok(ToolResult { content: input.to_string(), is_error: false })
        }
    }

    #[tokio::test]
    async fn test_dispatcher_dispatches_tool() {
        let dispatcher = ToolDispatcher::new(vec![Box::new(EchoTool)]);
        let result = dispatcher.dispatch("Echo", json!({"msg": "hi"})).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hi"));
    }

    #[tokio::test]
    async fn test_dispatcher_unknown_tool_returns_error() {
        let dispatcher = ToolDispatcher::new(vec![]);
        let result = dispatcher.dispatch("Nonexistent", json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatcher_definitions() {
        let dispatcher = ToolDispatcher::new(vec![Box::new(EchoTool)]);
        let defs = dispatcher.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "Echo");
    }
}
