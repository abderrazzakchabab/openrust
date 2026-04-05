use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};

use crate::agent_loop;
use crate::agents::AgentDispatch;
use crate::ai::types::{ContentBlock, Message, Role};
use crate::ai::{create_provider, Provider};
use crate::app::AppEvent;
use crate::config::Config;
use crate::permissions::PermissionChecker;
use crate::rules;
use crate::session::Session;
use crate::tools::create_tool_registry_with_custom;

pub async fn run_cli_mode(
    config: &Config,
    message: &str,
    format: &str,
    continue_session: bool,
) -> Result<()> {
    let working_dir = config
        .tools
        .working_directory
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let provider = create_provider_from_config(config)
        .context("Failed to create AI provider - check your API key configuration")?;

    let model = get_model_from_config(config);
    let max_tokens = get_max_tokens_from_config(config);
    let base_system_prompt = crate::app::build_system_prompt(&working_dir);
    let merged_rules = rules::load_merged_rules(&working_dir, &config.rules);
    let system_prompt = rules::prepend_rules(base_system_prompt, merged_rules);

    let messages = if continue_session {
        let mut sessions = Session::list_all().context("Failed to load sessions")?;
        if sessions.is_empty() {
            bail!("No sessions available to continue");
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let latest_session = Session::load(&sessions[0].id).context("Failed to load session")?;
        let mut msgs = latest_session.messages;
        msgs.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: message.to_string(),
            }],
        });
        msgs
    } else {
        vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: message.to_string(),
            }],
        }]
    };

    let registry = Arc::new(create_tool_registry_with_custom(
        working_dir.clone(),
        &config.custom_tools,
    ));
    let tools = Some(registry.to_definitions());

    let (event_tx, mut event_rx) = mpsc::channel(1000);

    let agent_dispatch = AgentDispatch::new();
    let agent_config = agent_dispatch.get("build").cloned();

    let permissions = Arc::new(Mutex::new(PermissionChecker::new_allow_all()));

    let loop_provider = provider;
    let loop_messages = messages.clone();
    let loop_event_tx = event_tx.clone();

    let handle = tokio::spawn(async move {
        agent_loop::run_agent_loop(
            loop_provider,
            loop_messages,
            model,
            max_tokens,
            Some(system_prompt),
            tools,
            registry,
            loop_event_tx,
            agent_config.as_ref(),
            permissions,
            None,
        )
        .await
    });

    let mut final_messages = messages;

    while let Some(event) = event_rx.recv().await {
        match event {
            AppEvent::MessagesUpdated(updated_messages) => {
                final_messages = updated_messages;
            }
            AppEvent::StreamError(err) => {
                eprintln!("Error: {}", err);
                std::process::exit(1);
            }
            _ => {}
        }
    }

    let result_messages = handle.await.context("Agent loop task failed")?;
    if !result_messages.is_empty() {
        final_messages = result_messages;
    }

    let output = match format {
        "json" => format_json(&final_messages)?,
        "markdown" => format_markdown(&final_messages),
        _ => format_text(&final_messages),
    };

    println!("{}", output);

    Ok(())
}

fn create_provider_from_config(config: &Config) -> Option<Box<dyn Provider>> {
    let provider_name = &config.provider.default;

    match provider_name.as_str() {
        "anthropic" => {
            let key = config.provider.anthropic.api_key.clone()?;
            create_provider(provider_name, key, None, None, None).ok()
        }
        "openai" => {
            let key = config.provider.openai.api_key.clone()?;
            let base_url = config.provider.openai.base_url.clone();
            create_provider(provider_name, key, base_url, None, None).ok()
        }
        "google" | "gemini" => {
            let google_cfg = config.provider.google.as_ref()?;
            let key = google_cfg
                .api_key
                .clone()
                .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())?;
            create_provider(provider_name, key, None, None, None).ok()
        }
        _ => None,
    }
}

fn get_model_from_config(config: &Config) -> String {
    match config.provider.default.as_str() {
        "anthropic" => config.provider.anthropic.model.clone(),
        "openai" => config.provider.openai.model.clone(),
        "google" | "gemini" => config
            .provider
            .google
            .as_ref()
            .map(|c| c.model.clone())
            .unwrap_or_else(|| "gemini-2.0-flash-exp".to_string()),
        _ => "claude-sonnet-4-20250514".to_string(),
    }
}

fn get_max_tokens_from_config(config: &Config) -> u32 {
    match config.provider.default.as_str() {
        "anthropic" => config.provider.anthropic.max_tokens,
        "openai" => config.provider.openai.max_tokens,
        "google" | "gemini" => config
            .provider
            .google
            .as_ref()
            .map(|c| c.max_tokens)
            .unwrap_or(8192),
        _ => 8192,
    }
}

fn format_text(messages: &[Message]) -> String {
    let mut output = String::new();

    for message in messages {
        match message.role {
            Role::User => {
                output.push_str("User:\n");
            }
            Role::Assistant => {
                output.push_str("\nAssistant:\n");
            }
            Role::System => {
                output.push_str("\nSystem:\n");
            }
        }

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    output.push_str(text);
                    output.push('\n');
                }
                ContentBlock::ToolUse { id, name, input } => {
                    output.push_str(&format!("[Tool Use: {} ({}): {}]\n", name, id, input));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let status = if *is_error { "ERROR" } else { "OK" };
                    output.push_str(&format!(
                        "[Tool Result ({}): {}]\n{}\n",
                        tool_use_id, status, content
                    ));
                }
                ContentBlock::Thinking { thinking } => {
                    output.push_str(&format!("[Thinking: {}]\n", thinking));
                }
            }
        }
    }

    output.trim_end().to_string()
}

fn format_json(messages: &[Message]) -> Result<String> {
    let json_messages: Vec<Value> = messages
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
                        })
                    }
                }).collect::<Vec<_>>()
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({ "messages": json_messages }))
        .context("Failed to serialize messages to JSON")
}

fn format_markdown(messages: &[Message]) -> String {
    let mut output = String::new();

    for message in messages {
        match message.role {
            Role::User => {
                output.push_str("## User\n\n");
            }
            Role::Assistant => {
                output.push_str("\n## Assistant\n\n");
            }
            Role::System => {
                output.push_str("\n## System\n\n");
            }
        }

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    output.push_str(text);
                    output.push_str("\n\n");
                }
                ContentBlock::ToolUse { id, name, input } => {
                    output.push_str(&format!(
                        "**[Tool Use: {}]**\n- ID: `{}`\n- Input: `{}`\n\n",
                        name, id, input
                    ));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let status = if *is_error { "❌ ERROR" } else { "✅ OK" };
                    output.push_str(&format!(
                        "**[Tool Result: {}]**\n```\n{}\n```\n\n",
                        status, content
                    ));
                }
                ContentBlock::Thinking { thinking } => {
                    output.push_str(&format!("*[Thinking: {}]*\n\n", thinking));
                }
            }
        }
    }

    output.trim_end().to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "markdown" | "md" => OutputFormat::Markdown,
            _ => OutputFormat::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CliResponse {
    pub content: String,
    pub tool_calls: Vec<CliToolCall>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliToolCall {
    pub name: String,
    pub input: serde_json::Value,
    pub output: String,
    pub is_error: bool,
}

fn extract_text_content(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_text_simple_message() {
        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "Hello".to_string(),
                }],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "Hi there!".to_string(),
                }],
            },
        ];

        let output = format_text(&messages);
        assert!(output.contains("User:"));
        assert!(output.contains("Hello"));
        assert!(output.contains("Assistant:"));
        assert!(output.contains("Hi there!"));
    }

    #[test]
    fn test_format_json_message() {
        let messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Test".to_string(),
            }],
        }];

        let output = format_json(&messages).unwrap();
        assert!(output.contains("\"role\": \"user\""));
        assert!(output.contains("\"type\": \"text\""));
        assert!(output.contains("\"text\": \"Test\""));
        assert!(output.contains("\"messages\""));
    }

    #[test]
    fn test_format_markdown_message() {
        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "Question?".to_string(),
                }],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "Answer.".to_string(),
                }],
            },
        ];

        let output = format_markdown(&messages);
        assert!(output.contains("## User"));
        assert!(output.contains("Question?"));
        assert!(output.contains("## Assistant"));
        assert!(output.contains("Answer."));
    }

    #[test]
    fn test_format_text_with_tool_calls() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me check that.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    input: json!({"command": "ls"}),
                },
            ],
        }];

        let output = format_text(&messages);
        assert!(output.contains("Let me check that."));
        assert!(output.contains("[Tool Use: bash"));
        assert!(output.contains("call_1"));
    }

    #[test]
    fn test_format_json_with_tool_calls() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_2".to_string(),
                name: "read".to_string(),
                input: json!({"file_path": "test.txt"}),
            }],
        }];

        let output = format_json(&messages).unwrap();
        assert!(output.contains("\"type\": \"tool_use\""));
        assert!(output.contains("\"name\": \"read\""));
        assert!(output.contains("call_2"));
    }

    #[test]
    fn test_empty_message_formatting() {
        let messages: Vec<Message> = vec![];

        assert_eq!(format_text(&messages), "");
        assert!(format_json(&messages).unwrap().contains("\"messages\": []"));
        assert_eq!(format_markdown(&messages), "");
    }
}
