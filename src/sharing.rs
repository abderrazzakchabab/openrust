use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

use crate::ai::types::{ContentBlock, Message, Role};

/// Export conversation to markdown format
pub fn export_markdown(messages: &[Message], path: &PathBuf) -> Result<String> {
    let mut markdown = String::new();
    markdown.push_str("# Conversation Export\n\n");

    for message in messages {
        let role_header = match message.role {
            Role::User => "## User",
            Role::Assistant => "## Assistant",
            Role::System => "## System",
        };

        markdown.push_str(role_header);
        markdown.push_str("\n\n");

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    markdown.push_str(text);
                    markdown.push_str("\n\n");
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    markdown.push_str(&format!("```\n[Tool: {}]\n{}\n```\n\n", name, input));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let prefix = if *is_error { "[Error] " } else { "" };
                    markdown.push_str(&format!("```\n{}[Result]\n{}\n```\n\n", prefix, content));
                }
                ContentBlock::Thinking { thinking } => {
                    markdown.push_str(&format!("```\n[Thinking]\n{}\n```\n\n", thinking));
                }
            }
        }
    }

    fs::write(path, &markdown)
        .with_context(|| format!("Failed to write markdown export to {}", path.display()))?;

    Ok(format!("Exported conversation to {}", path.display()))
}

/// Export conversation to JSON format
pub fn export_json(messages: &[Message], path: &PathBuf) -> Result<String> {
    let json_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|msg| {
            json!({
                "role": msg.role.to_string(),
                "content": msg.content.iter().map(|block| {
                    match block {
                        ContentBlock::Text { text } => json!({
                            "type": "text",
                            "text": text
                        }),
                        ContentBlock::ToolUse { id, name, input } => json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }),
                        ContentBlock::ToolResult { tool_use_id, content, is_error } => json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                            "is_error": is_error
                        }),
                        ContentBlock::Thinking { thinking } => json!({
                            "type": "thinking",
                            "thinking": thinking
                        }),
                    }
                }).collect::<Vec<_>>()
            })
        })
        .collect();

    let export = json!({
        "version": "1.0",
        "messages": json_messages
    });

    let json_str = serde_json::to_string_pretty(&export)
        .context("Failed to serialize conversation to JSON")?;

    fs::write(path, &json_str)
        .with_context(|| format!("Failed to write JSON export to {}", path.display()))?;

    Ok(format!("Exported conversation to {}", path.display()))
}

/// Export conversation to HTML format
pub fn export_html(messages: &[Message], path: &PathBuf) -> Result<String> {
    let mut html = String::new();
    html.push_str(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Conversation Export</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            line-height: 1.6;
            max-width: 900px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
            color: #333;
        }
        h1 {
            color: #2c3e50;
            border-bottom: 3px solid #3498db;
            padding-bottom: 10px;
        }
        .message {
            margin: 20px 0;
            padding: 15px;
            border-radius: 8px;
            background-color: white;
            border-left: 4px solid #3498db;
        }
        .message.user {
            border-left-color: #2ecc71;
            background-color: #f0f8f5;
        }
        .message.assistant {
            border-left-color: #3498db;
            background-color: #f0f4f8;
        }
        .message.system {
            border-left-color: #e74c3c;
            background-color: #fdf0f0;
        }
        .role {
            font-weight: bold;
            font-size: 0.9em;
            text-transform: uppercase;
            margin-bottom: 10px;
            color: #555;
        }
        .user .role { color: #27ae60; }
        .assistant .role { color: #2980b9; }
        .system .role { color: #c0392b; }
        .content {
            margin: 10px 0;
        }
        .block {
            margin: 10px 0;
        }
        code {
            background-color: #f4f4f4;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: "Courier New", monospace;
            font-size: 0.9em;
        }
        pre {
            background-color: #f4f4f4;
            padding: 12px;
            border-radius: 4px;
            overflow-x: auto;
            border: 1px solid #ddd;
        }
        .tool-use {
            background-color: #fff3cd;
            border: 1px solid #ffc107;
            padding: 10px;
            border-radius: 4px;
            margin: 10px 0;
        }
        .tool-result {
            background-color: #d4edda;
            border: 1px solid #28a745;
            padding: 10px;
            border-radius: 4px;
            margin: 10px 0;
        }
        .tool-result.error {
            background-color: #f8d7da;
            border: 1px solid #f5c6cb;
        }
        .thinking {
            background-color: #e7e7ff;
            border: 1px solid #b3b3ff;
            padding: 10px;
            border-radius: 4px;
            margin: 10px 0;
            font-style: italic;
        }
    </style>
</head>
<body>
    <h1>Conversation Export</h1>
"#,
    );

    for message in messages {
        let role_class = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        };

        html.push_str(&format!(
            r#"    <div class="message {}">
        <div class="role">{}</div>
        <div class="content">
"#,
            role_class,
            message.role.to_string()
        ));

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    html.push_str(&format!(
                        r#"            <div class="block">{}</div>
"#,
                        escape_html(text)
                    ));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    html.push_str(&format!(
                        r#"            <div class="block tool-use">
                <strong>Tool:</strong> {}<br>
                <strong>Input:</strong> <pre>{}</pre>
            </div>
"#,
                        escape_html(name),
                        escape_html(&input.to_string())
                    ));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let error_class = if *is_error { " error" } else { "" };
                    html.push_str(&format!(
                        r#"            <div class="block tool-result{}">
                <strong>Result:</strong> <pre>{}</pre>
            </div>
"#,
                        error_class,
                        escape_html(content)
                    ));
                }
                ContentBlock::Thinking { thinking } => {
                    html.push_str(&format!(
                        r#"            <div class="block thinking">
                <strong>Thinking:</strong> {}
            </div>
"#,
                        escape_html(thinking)
                    ));
                }
            }
        }

        html.push_str(
            r#"        </div>
    </div>
"#,
        );
    }

    html.push_str(
        r#"</body>
</html>
"#,
    );

    fs::write(path, &html)
        .with_context(|| format!("Failed to write HTML export to {}", path.display()))?;

    Ok(format!("Exported conversation to {}", path.display()))
}

/// Create a shareable file from the conversation
pub fn share_conversation(
    messages: &[Message],
    session_id: &str,
    working_dir: &PathBuf,
) -> Result<String> {
    let shared_dir = working_dir.join(".openrust").join("shared");
    fs::create_dir_all(&shared_dir).with_context(|| {
        format!(
            "Failed to create shared directory at {}",
            shared_dir.display()
        )
    })?;

    let shared_path = shared_dir.join(format!("{}.md", session_id));

    // Use markdown format for shared files
    let mut markdown = String::new();
    markdown.push_str("# Shared Conversation\n\n");

    for message in messages {
        let role_header = match message.role {
            Role::User => "## User",
            Role::Assistant => "## Assistant",
            Role::System => "## System",
        };

        markdown.push_str(role_header);
        markdown.push_str("\n\n");

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    markdown.push_str(text);
                    markdown.push_str("\n\n");
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    markdown.push_str(&format!("```\n[Tool: {}]\n{}\n```\n\n", name, input));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let prefix = if *is_error { "[Error] " } else { "" };
                    markdown.push_str(&format!("```\n{}[Result]\n{}\n```\n\n", prefix, content));
                }
                ContentBlock::Thinking { thinking } => {
                    markdown.push_str(&format!("```\n[Thinking]\n{}\n```\n\n", thinking));
                }
            }
        }
    }

    fs::write(&shared_path, &markdown).with_context(|| {
        format!(
            "Failed to write shared conversation to {}",
            shared_path.display()
        )
    })?;

    Ok(format!("Shared at: {}", shared_path.display()))
}

/// Remove shared conversation file
pub fn unshare_conversation(session_id: &str, working_dir: &PathBuf) -> Result<String> {
    let shared_path = working_dir
        .join(".openrust")
        .join("shared")
        .join(format!("{}.md", session_id));

    if shared_path.exists() {
        fs::remove_file(&shared_path).with_context(|| {
            format!("Failed to remove shared file at {}", shared_path.display())
        })?;
        Ok("Unshared".to_string())
    } else {
        Ok("No shared file found".to_string())
    }
}

/// Helper function to escape HTML special characters
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_messages() -> Vec<Message> {
        vec![
            Message::new_user("Hello, how are you?"),
            Message::new_assistant("I'm doing well, thank you for asking!"),
            Message::new_user("Can you help me with Rust?"),
            Message::new_assistant("Of course! I'd be happy to help with Rust."),
        ]
    }

    #[test]
    fn test_export_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");
        let messages = create_test_messages();

        let result = export_markdown(&messages, &path);
        assert!(result.is_ok());
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Conversation Export"));
        assert!(content.contains("## User"));
        assert!(content.contains("## Assistant"));
        assert!(content.contains("Hello, how are you?"));
        assert!(content.contains("I'm doing well"));
    }

    #[test]
    fn test_export_json() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let messages = create_test_messages();

        let result = export_json(&messages, &path);
        assert!(result.is_ok());
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["version"], "1.0");
        assert!(json["messages"].is_array());
        assert_eq!(json["messages"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_export_html() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.html");
        let messages = create_test_messages();

        let result = export_html(&messages, &path);
        assert!(result.is_ok());
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Conversation Export"));
        assert!(content.contains("class=\"message user\""));
        assert!(content.contains("class=\"message assistant\""));
    }

    #[test]
    fn test_export_empty_messages() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("empty.md");
        let messages: Vec<Message> = vec![];

        let result = export_markdown(&messages, &path);
        assert!(result.is_ok());
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Conversation Export"));
    }

    #[test]
    fn test_share_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let messages = create_test_messages();
        let session_id = "test-session-123";

        let result = share_conversation(&messages, session_id, &temp_dir.path().to_path_buf());
        assert!(result.is_ok());

        let shared_path = temp_dir
            .path()
            .join(".openrust")
            .join("shared")
            .join(format!("{}.md", session_id));
        assert!(shared_path.exists());

        let content = fs::read_to_string(&shared_path).unwrap();
        assert!(content.contains("# Shared Conversation"));
        assert!(content.contains("Hello, how are you?"));
    }

    #[test]
    fn test_unshare_removes_file() {
        let temp_dir = TempDir::new().unwrap();
        let messages = create_test_messages();
        let session_id = "test-session-456";

        // First share the conversation
        let _ = share_conversation(&messages, session_id, &temp_dir.path().to_path_buf());
        let shared_path = temp_dir
            .path()
            .join(".openrust")
            .join("shared")
            .join(format!("{}.md", session_id));
        assert!(shared_path.exists());

        // Then unshare it
        let result = unshare_conversation(session_id, &temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        assert!(!shared_path.exists());
    }

    #[test]
    fn test_unshare_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let session_id = "nonexistent-session";

        let result = unshare_conversation(session_id, &temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No shared file found");
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(escape_html("'single'"), "&#39;single&#39;");
    }
}
