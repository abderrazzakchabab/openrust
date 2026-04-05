use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use crate::ai::types::ToolDefinition;

/// Output from a tool execution
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        ToolOutput {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        ToolOutput {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Trait for tools that can be called by the AI
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of the tool
    fn name(&self) -> &str;

    /// Returns a description of what the tool does
    fn description(&self) -> &str;

    /// Returns the JSON Schema for the tool's input parameters
    fn parameters(&self) -> Value;

    /// Executes the tool with the given input
    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput>;
}

/// Registry for managing tools
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Creates a new empty tool registry
    pub fn new() -> Self {
        ToolRegistry {
            tools: HashMap::new(),
        }
    }

    /// Registers a tool in the registry
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Gets a tool by name
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// Lists all registered tool names
    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(|k| k.as_str()).collect()
    }

    /// Converts all registered tools to ToolDefinition format for the AI API
    pub fn to_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.parameters(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Mock tool for testing
    struct MockTool {
        name: String,
        description: String,
        params: Value,
    }

    impl MockTool {
        fn new(name: &str, description: &str) -> Self {
            MockTool {
                name: name.to_string(),
                description: description.to_string(),
                params: json!({
                    "type": "object",
                    "properties": {
                        "input": {
                            "type": "string",
                            "description": "Test input"
                        }
                    },
                    "required": ["input"]
                }),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn parameters(&self) -> Value {
            self.params.clone()
        }

        async fn execute(&self, _input: Value) -> anyhow::Result<ToolOutput> {
            Ok(ToolOutput::success("mock result"))
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        let tool = Box::new(MockTool::new("test_tool", "A test tool"));

        registry.register(tool);

        let retrieved = registry.get("test_tool");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "test_tool");
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("tool1", "First tool")));
        registry.register(Box::new(MockTool::new("tool2", "Second tool")));

        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"tool1"));
        assert!(list.contains(&"tool2"));
    }

    #[test]
    fn test_registry_to_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("bash", "Execute bash commands")));
        registry.register(Box::new(MockTool::new("read", "Read files")));

        let definitions = registry.to_definitions();
        assert_eq!(definitions.len(), 2);

        let bash_def = definitions.iter().find(|d| d.name == "bash");
        assert!(bash_def.is_some());
        assert_eq!(bash_def.unwrap().description, "Execute bash commands");

        let read_def = definitions.iter().find(|d| d.name == "read");
        assert!(read_def.is_some());
        assert_eq!(read_def.unwrap().description, "Read files");
    }

    #[test]
    fn test_tool_output_success() {
        let output = ToolOutput::success("test content");
        assert_eq!(output.content, "test content");
        assert!(!output.is_error);
    }

    #[test]
    fn test_tool_output_error() {
        let output = ToolOutput::error("error message");
        assert_eq!(output.content, "error message");
        assert!(output.is_error);
    }

    #[tokio::test]
    async fn test_mock_tool_execute() {
        let tool = MockTool::new("test", "Test tool");
        let result = tool.execute(json!({"input": "test"})).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.content, "mock result");
        assert!(!output.is_error);
    }
}
