use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

/// Tool for asking questions to the user via the TUI
pub struct QuestionTool;

impl QuestionTool {
    pub fn new() -> Self {
        Self
    }

    fn format_question(question: &str, options: Option<&Vec<Value>>) -> String {
        let mut result = format!("Question: {question}");

        if let Some(opts) = options {
            if !opts.is_empty() {
                result.push_str("\nOptions:");
                for (idx, opt) in opts.iter().enumerate() {
                    let label = opt
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("(unlabeled)");
                    let description = opt.get("description").and_then(Value::as_str);

                    result.push_str(&format!("\n  {}. {label}", idx + 1));
                    if let Some(desc) = description {
                        result.push_str(&format!(" - {desc}"));
                    }
                }
            }
        }

        result
    }
}

#[async_trait]
impl Tool for QuestionTool {
    fn name(&self) -> &str {
        "question"
    }

    fn description(&self) -> &str {
        "Ask the user a question and wait for their response"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                },
                "options": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": {
                                "type": "string",
                                "description": "The option label"
                            },
                            "description": {
                                "type": "string",
                                "description": "Optional description of the option"
                            }
                        },
                        "required": ["label"]
                    },
                    "description": "Options for the user to choose from"
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let question = match input.get("question").and_then(Value::as_str) {
            Some(q) => q,
            None => return Ok(ToolOutput::error("Missing required field: question")),
        };

        let options = input.get("options").and_then(Value::as_array);

        // TODO: Wire to TUI popup in Wave 5 (Task 15 enhancement)
        // For now, return formatted question text as a prompt to the user
        // The full implementation will:
        // 1. Send AppEvent::QuestionPrompt to the app
        // 2. Switch to AppMode::QuestionPrompt
        // 3. Display a modal dialog with the question and options
        // 4. Wait for user input via oneshot channel
        // 5. Return the user's response

        let formatted = Self::format_question(question, options);
        Ok(ToolOutput::success(formatted))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_question_with_options() {
        let tool = QuestionTool::new();
        let input = json!({
            "question": "What is your favorite color?",
            "options": [
                {"label": "Red", "description": "The color of passion"},
                {"label": "Blue", "description": "The color of calm"},
                {"label": "Green"}
            ]
        });

        let result = tool.execute(input).await.expect("execute");

        assert!(!result.is_error);
        assert!(result
            .content
            .contains("Question: What is your favorite color?"));
        assert!(result.content.contains("Options:"));
        assert!(result.content.contains("1. Red - The color of passion"));
        assert!(result.content.contains("2. Blue - The color of calm"));
        assert!(result.content.contains("3. Green"));
    }

    #[tokio::test]
    async fn test_question_without_options() {
        let tool = QuestionTool::new();
        let input = json!({
            "question": "What is your name?"
        });

        let result = tool.execute(input).await.expect("execute");

        assert!(!result.is_error);
        assert!(result.content.contains("Question: What is your name?"));
        assert!(!result.content.contains("Options:"));
    }

    #[tokio::test]
    async fn test_question_with_empty_options() {
        let tool = QuestionTool::new();
        let input = json!({
            "question": "Do you agree?",
            "options": []
        });

        let result = tool.execute(input).await.expect("execute");

        assert!(!result.is_error);
        assert!(result.content.contains("Question: Do you agree?"));
        assert!(!result.content.contains("Options:"));
    }

    #[tokio::test]
    async fn test_question_missing_required_field() {
        let tool = QuestionTool::new();
        let input = json!({
            "options": [{"label": "Yes"}, {"label": "No"}]
        });

        let result = tool.execute(input).await.expect("execute");

        assert!(result.is_error);
        assert!(result.content.contains("Missing required field: question"));
    }
}
