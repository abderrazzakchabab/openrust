use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::traits::{Tool, ToolOutput};

/// Represents a single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub priority: String,
}

/// Tool for writing/updating the todo list
pub struct TodoWriteTool {
    todos: Arc<Mutex<Vec<TodoItem>>>,
}

impl TodoWriteTool {
    pub fn new(todos: Arc<Mutex<Vec<TodoItem>>>) -> Self {
        Self { todos }
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todowrite"
    }

    fn description(&self) -> &str {
        "Write or replace the entire todo list with new todos"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The todo item content"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "cancelled"],
                                "description": "The status of the todo"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["high", "medium", "low"],
                                "description": "The priority level"
                            }
                        },
                        "required": ["content", "status", "priority"]
                    },
                    "description": "Array of todo items to write"
                }
            },
            "required": ["todos"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let todos_input = match input.get("todos").and_then(Value::as_array) {
            Some(todos) => todos,
            None => return Ok(ToolOutput::error("Missing required field: todos")),
        };

        let mut new_todos = Vec::new();

        for todo_value in todos_input {
            let content = match todo_value.get("content").and_then(Value::as_str) {
                Some(c) => c.to_string(),
                None => return Ok(ToolOutput::error("Todo item missing 'content' field")),
            };

            let status = match todo_value.get("status").and_then(Value::as_str) {
                Some(s) => s.to_string(),
                None => return Ok(ToolOutput::error("Todo item missing 'status' field")),
            };

            let priority = match todo_value.get("priority").and_then(Value::as_str) {
                Some(p) => p.to_string(),
                None => return Ok(ToolOutput::error("Todo item missing 'priority' field")),
            };

            // Validate status
            if !["pending", "in_progress", "completed", "cancelled"].contains(&status.as_str()) {
                return Ok(ToolOutput::error(
                    "Invalid status. Must be one of: pending, in_progress, completed, cancelled",
                ));
            }

            // Validate priority
            if !["high", "medium", "low"].contains(&priority.as_str()) {
                return Ok(ToolOutput::error(
                    "Invalid priority. Must be one of: high, medium, low",
                ));
            }

            new_todos.push(TodoItem {
                content,
                status,
                priority,
            });
        }

        let mut todos = self.todos.lock().await;
        *todos = new_todos;
        let count = todos.len();

        Ok(ToolOutput::success(format!(
            "Todo list updated with {} items",
            count
        )))
    }
}

/// Tool for reading the current todo list
pub struct TodoReadTool {
    todos: Arc<Mutex<Vec<TodoItem>>>,
}

impl TodoReadTool {
    pub fn new(todos: Arc<Mutex<Vec<TodoItem>>>) -> Self {
        Self { todos }
    }
}

#[async_trait]
impl Tool for TodoReadTool {
    fn name(&self) -> &str {
        "todoread"
    }

    fn description(&self) -> &str {
        "Read and display the current todo list"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> anyhow::Result<ToolOutput> {
        let todos = self.todos.lock().await;

        if todos.is_empty() {
            return Ok(ToolOutput::success("No todos"));
        }

        let formatted = todos
            .iter()
            .enumerate()
            .map(|(idx, todo)| {
                format!(
                    "{}. [{}] ({}) {}",
                    idx + 1,
                    todo.status,
                    todo.priority,
                    todo.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::success(formatted))
    }
}

/// Factory function to create both todo tools with shared state
pub fn create_todo_tools() -> (TodoWriteTool, TodoReadTool) {
    let todos = Arc::new(Mutex::new(Vec::new()));
    (TodoWriteTool::new(todos.clone()), TodoReadTool::new(todos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_and_read_todos() {
        let (write_tool, read_tool) = create_todo_tools();

        let input = json!({
            "todos": [
                {
                    "content": "Implement feature X",
                    "status": "in_progress",
                    "priority": "high"
                },
                {
                    "content": "Fix bug Y",
                    "status": "pending",
                    "priority": "medium"
                }
            ]
        });

        let write_result = write_tool.execute(input).await;
        assert!(write_result.is_ok());
        let output = write_result.unwrap();
        assert!(!output.is_error);
        assert_eq!(output.content, "Todo list updated with 2 items");

        let read_result = read_tool.execute(json!({})).await;
        assert!(read_result.is_ok());
        let output = read_result.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("Implement feature X"));
        assert!(output.content.contains("Fix bug Y"));
        assert!(output.content.contains("[in_progress]"));
        assert!(output.content.contains("[pending]"));
    }

    #[tokio::test]
    async fn test_write_replaces_todos() {
        let (write_tool, read_tool) = create_todo_tools();

        // Write first set of todos
        let input1 = json!({
            "todos": [
                {
                    "content": "First todo",
                    "status": "pending",
                    "priority": "high"
                }
            ]
        });

        write_tool.execute(input1).await.unwrap();

        // Write second set of todos (should replace, not append)
        let input2 = json!({
            "todos": [
                {
                    "content": "Second todo",
                    "status": "completed",
                    "priority": "low"
                },
                {
                    "content": "Third todo",
                    "status": "in_progress",
                    "priority": "medium"
                }
            ]
        });

        write_tool.execute(input2).await.unwrap();

        let read_result = read_tool.execute(json!({})).await.unwrap();
        assert!(!read_result.content.contains("First todo"));
        assert!(read_result.content.contains("Second todo"));
        assert!(read_result.content.contains("Third todo"));
        assert_eq!(
            read_result.content.matches('\n').count(),
            1,
            "Should have exactly 2 todos (1 newline)"
        );
    }

    #[tokio::test]
    async fn test_read_empty_todo_list() {
        let (_write_tool, read_tool) = create_todo_tools();

        let read_result = read_tool.execute(json!({})).await;
        assert!(read_result.is_ok());
        let output = read_result.unwrap();
        assert!(!output.is_error);
        assert_eq!(output.content, "No todos");
    }

    #[tokio::test]
    async fn test_invalid_status() {
        let (write_tool, _read_tool) = create_todo_tools();

        let input = json!({
            "todos": [
                {
                    "content": "Test todo",
                    "status": "invalid_status",
                    "priority": "high"
                }
            ]
        });

        let result = write_tool.execute(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("Invalid status"));
    }

    #[tokio::test]
    async fn test_invalid_priority() {
        let (write_tool, _read_tool) = create_todo_tools();

        let input = json!({
            "todos": [
                {
                    "content": "Test todo",
                    "status": "pending",
                    "priority": "invalid_priority"
                }
            ]
        });

        let result = write_tool.execute(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("Invalid priority"));
    }

    #[tokio::test]
    async fn test_missing_required_field() {
        let (write_tool, _read_tool) = create_todo_tools();

        let input = json!({
            "todos": [
                {
                    "content": "Test todo",
                    "status": "pending"
                    // missing priority
                }
            ]
        });

        let result = write_tool.execute(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("missing 'priority'"));
    }
}
