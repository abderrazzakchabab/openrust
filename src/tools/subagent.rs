use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::traits::{Tool, ToolOutput};

/// Tool that allows primary agents to spawn subagents for delegated tasks
pub struct SubagentTool;

impl SubagentTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubagentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "Spawn a subagent to perform a task. The subagent runs in its own conversation context with specialized capabilities."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Agent type: 'general' for general-purpose tasks, 'explore' for fast codebase exploration",
                    "enum": ["general", "explore"]
                },
                "task": {
                    "type": "string",
                    "description": "Task description for the subagent to execute"
                }
            },
            "required": ["agent", "task"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let agent = input
            .get("agent")
            .and_then(Value::as_str)
            .unwrap_or("general");
        let task = input.get("task").and_then(Value::as_str).unwrap_or("");

        if agent != "general" && agent != "explore" {
            return Ok(ToolOutput::error(format!(
                "Invalid agent type: '{}'. Must be 'general' or 'explore'.",
                agent
            )));
        }

        if task.is_empty() {
            return Ok(ToolOutput::error(
                "Task description is required and cannot be empty.",
            ));
        }

        // TODO: Implement actual subagent spawning with isolated conversation context.
        // This requires:
        // 1. Creating a new conversation context for the subagent
        // 2. Running the agent loop with the specified agent config
        // 3. Capturing and returning the subagent's results
        // 4. Handling errors and timeouts gracefully
        // For now, return a formatted message indicating the subagent request has been noted.

        Ok(ToolOutput::success(format!(
            "Subagent request registered:\n\
             Agent: {}\n\
             Task: {}\n\n\
             Note: Subagent execution is not yet fully implemented. \
             The task has been noted for manual execution or future implementation.",
            agent, task
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subagent_tool_general_agent() {
        let tool = SubagentTool::new();
        let input = json!({
            "agent": "general",
            "task": "Analyze the codebase structure"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Agent: general"));
        assert!(result.content.contains("Analyze the codebase structure"));
    }

    #[tokio::test]
    async fn test_subagent_tool_explore_agent() {
        let tool = SubagentTool::new();
        let input = json!({
            "agent": "explore",
            "task": "Find all Rust files in src/"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Agent: explore"));
        assert!(result.content.contains("Find all Rust files"));
    }

    #[tokio::test]
    async fn test_subagent_tool_invalid_agent() {
        let tool = SubagentTool::new();
        let input = json!({
            "agent": "invalid",
            "task": "Some task"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Invalid agent type"));
    }

    #[tokio::test]
    async fn test_subagent_tool_missing_task() {
        let tool = SubagentTool::new();
        let input = json!({
            "agent": "general",
            "task": ""
        });

        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Task description is required"));
    }

    #[tokio::test]
    async fn test_subagent_tool_missing_agent_defaults_to_general() {
        let tool = SubagentTool::new();
        let input = json!({
            "task": "Some task"
        });

        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Agent: general"));
    }

    #[test]
    fn test_subagent_tool_metadata() {
        let tool = SubagentTool::new();
        assert_eq!(tool.name(), "subagent");
        assert!(tool.description().contains("subagent"));
        assert!(tool.description().contains("specialized capabilities"));

        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["agent"]["enum"].is_array());
        assert!(params["required"].is_array());
    }
}
