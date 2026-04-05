use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

pub struct WebSearchTool;

impl WebSearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "websearch"
    }

    fn description(&self) -> &str {
        "Search the web for a query and return structured results"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5)",
                    "minimum": 1,
                    "maximum": 20
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let query = match input.get("query").and_then(Value::as_str) {
            Some(query) => query,
            None => return Ok(ToolOutput::error("Missing required field: query")),
        };

        // TODO: Implement actual search API integration (Google Custom Search, Brave Search, etc.)
        let message = format!(
            "Web search for: '{}'\n\n\
            Note: Web search requires an API key to be configured. \
            Set `websearch_api_key` in your config to enable this feature.\n\n\
            Alternatively, use the `webfetch` tool to fetch a specific URL.",
            query
        );

        Ok(ToolOutput::success(message))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_websearch_missing_query() {
        let tool = WebSearchTool::new();
        let output = tool.execute(json!({})).await.expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("Missing required field: query"));
    }

    #[tokio::test]
    async fn test_websearch_returns_informational_message() {
        let tool = WebSearchTool::new();
        let output = tool
            .execute(json!({"query": "rust programming"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output
            .content
            .contains("Web search for: 'rust programming'"));
        assert!(output.content.contains("API key"));
        assert!(output.content.contains("webfetch"));
    }

    #[tokio::test]
    async fn test_websearch_with_num_results() {
        let tool = WebSearchTool::new();
        let output = tool
            .execute(json!({"query": "test query", "num_results": 10}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("Web search for: 'test query'"));
    }
}
