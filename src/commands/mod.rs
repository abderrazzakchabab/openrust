use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::agents::hidden;
use crate::app::{App, AppEvent, AppMode};
use crate::ai::types::Message;
use crate::config::CustomCommandConfig;
use crate::session::Session;
use crate::sharing;

/// Result of executing a slash command
#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    /// Command executed successfully with no output
    Ok,
    /// Command executed successfully with a status message
    Message(String),
    /// Command failed with an error message
    Error(String),
    /// Command requests a mode change
    ModeChange(AppMode),
    /// Command requests application quit
    Quit,
}

/// A registered slash command
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub description: &'static str,
    pub handler: fn(&[&str], &mut App) -> CommandResult,
}

impl SlashCommand {
    fn new(
        name: &'static str,
        aliases: Vec<&'static str>,
        description: &'static str,
        handler: fn(&[&str], &mut App) -> CommandResult,
    ) -> Self {
        Self {
            name,
            aliases,
            description,
            handler,
        }
    }
}

/// Registry of all available slash commands
pub struct CommandRegistry {
    commands: Vec<SlashCommand>,
    custom_commands: HashMap<String, CustomCommandConfig>,
}

impl CommandRegistry {
    /// Create a new registry with all core commands
    pub fn new() -> Self {
        let mut registry = Self {
            commands: Vec::new(),
            custom_commands: HashMap::new(),
        };
        registry.register_core_commands();
        registry
    }

    /// Add custom commands from config
    pub fn with_custom_commands(mut self, commands: HashMap<String, CustomCommandConfig>) -> Self {
        self.custom_commands = commands;
        self
    }

    fn register_core_commands(&mut self) {
        self.commands.push(SlashCommand::new(
            "new",
            vec![],
            "Create a new session",
            cmd_new,
        ));

        self.commands.push(SlashCommand::new(
            "help",
            vec![],
            "Show help screen",
            cmd_help,
        ));

        self.commands.push(SlashCommand::new(
            "sessions",
            vec![],
            "Show session list",
            cmd_sessions,
        ));

        self.commands.push(SlashCommand::new(
            "exit",
            vec!["quit"],
            "Save session and quit",
            cmd_exit,
        ));

        self.commands.push(SlashCommand::new(
            "compact",
            vec![],
            "Trigger conversation compaction",
            cmd_compact,
        ));

        self.commands.push(SlashCommand::new(
            "details",
            vec![],
            "Show session details",
            cmd_details,
        ));

        self.commands.push(SlashCommand::new(
            "editor",
            vec![],
            "Open external editor",
            cmd_editor,
        ));

        self.commands.push(SlashCommand::new(
            "export",
            vec![],
            "Export conversation to markdown",
            cmd_export,
        ));

        self.commands.push(SlashCommand::new(
            "models",
            vec![],
            "List available models and show current model",
            cmd_models,
        ));

        self.commands.push(SlashCommand::new(
            "themes",
            vec![],
            "List available themes and show current theme",
            cmd_themes,
        ));

        self.commands.push(SlashCommand::new(
            "thinking",
            vec![],
            "Toggle extended thinking display",
            cmd_thinking,
        ));

        self.commands.push(SlashCommand::new(
            "connect",
            vec![],
            "Connect to MCP server",
            cmd_connect,
        ));

        self.commands.push(SlashCommand::new(
            "share",
            vec![],
            "Share conversation",
            cmd_share,
        ));

        self.commands.push(SlashCommand::new(
            "unshare",
            vec![],
            "Unshare conversation",
            cmd_unshare,
        ));

        self.commands.push(SlashCommand::new(
            "undo",
            vec![],
            "Undo last change",
            cmd_undo,
        ));

        self.commands.push(SlashCommand::new(
            "redo",
            vec![],
            "Redo last change",
            cmd_redo,
        ));

        self.commands.push(SlashCommand::new(
            "init",
            vec![],
            "Initialize project configuration",
            cmd_init,
        ));
    }

    /// Parse input as a slash command, returns (command_name, args) if valid
    pub fn parse<'a>(&self, input: &'a str) -> Option<(&'a str, Vec<&'a str>)> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let without_slash = &trimmed[1..];
        let parts: Vec<&str> = without_slash.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let command_name = parts[0];
        let args = parts[1..].to_vec();
        Some((command_name, args))
    }

    /// Execute a command by name with arguments
    pub fn execute(&self, name: &str, args: &[&str], app: &mut App) -> CommandResult {
        let command = self
            .commands
            .iter()
            .find(|cmd| cmd.name == name || cmd.aliases.iter().any(|alias| *alias == name));

        match command {
            Some(cmd) => (cmd.handler)(args, app),
            None => {
                if let Some(custom_cmd) = self.custom_commands.get(name) {
                    execute_custom_command(custom_cmd, args, app)
                } else {
                    CommandResult::Error(format!(
                        "Unknown command: /{}. Type /help for available commands.",
                        name
                    ))
                }
            }
        }
    }

    /// Get command completions for a partial input
    pub fn completions(&self, partial: &str) -> Vec<String> {
        let trimmed = partial.trim();
        if !trimmed.starts_with('/') {
            return vec![];
        }

        let without_slash = &trimmed[1..];
        if without_slash.is_empty() {
            let mut completions: Vec<String> = self
                .commands
                .iter()
                .map(|cmd| format!("/{}", cmd.name))
                .collect();
            completions.extend(self.custom_commands.keys().map(|name| format!("/{}", name)));
            return completions;
        }

        let mut completions: Vec<String> = self
            .commands
            .iter()
            .filter(|cmd| cmd.name.starts_with(without_slash))
            .map(|cmd| format!("/{}", cmd.name))
            .collect();
        completions.extend(
            self.custom_commands
                .keys()
                .filter(|name| name.starts_with(without_slash))
                .map(|name| format!("/{}", name)),
        );
        completions
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn cmd_new(_args: &[&str], app: &mut App) -> CommandResult {
    let _ = app.session.save();

    let provider = app.config.provider.default.clone();
    let model = app.get_model();
    app.session = Session::new(&provider, &model);
    app.scroll_to_bottom();

    CommandResult::Message("Started new session".to_string())
}

fn cmd_help(_args: &[&str], _app: &mut App) -> CommandResult {
    CommandResult::ModeChange(AppMode::Help)
}

fn cmd_sessions(_args: &[&str], _app: &mut App) -> CommandResult {
    CommandResult::ModeChange(AppMode::SessionList)
}

fn cmd_exit(_args: &[&str], app: &mut App) -> CommandResult {
    let _ = app.session.save();
    CommandResult::Quit
}

fn cmd_compact(_args: &[&str], app: &mut App) -> CommandResult {
    if app.session.messages.is_empty() {
        return CommandResult::Message("No messages to compact".to_string());
    }

    let message_count = app.session.messages.len();
    let token_estimate = hidden::estimate_tokens(&app.session.messages);
    
    if message_count < 4 {
        return CommandResult::Message(format!(
            "Conversation too short to compact ({} messages, ~{} tokens). Need at least 4 messages.",
            message_count, token_estimate
        ));
    }

    let max_tokens = app.get_max_tokens() as usize;
    if !hidden::needs_compaction(&app.session.messages, max_tokens) {
        return CommandResult::Message(format!(
            "Conversation doesn't need compacting ({} messages, ~{} tokens, limit ~{})",
            message_count, token_estimate, max_tokens
        ));
    }

    let (system_prompt,user_message) = create_compact_message(&app.session.messages);
    
    app.session.messages = vec![
        Message::new_assistant("Compacting conversation...".to_string())
    ];
    app.scroll_to_bottom();

    let provider = match app.get_provider() {
        Some(p)=> p,
        None => return CommandResult::Error("No API key configured".to_string()),
    };

    let model = app.get_model();
    let max_tokens = app.get_max_tokens();
    let event_tx = app.event_tx.clone();

    tokio::spawn(async move {
        use crate::ai::types::CompletionRequest;
        let request = CompletionRequest {
            messages: vec![user_message],
            model,
            max_tokens,
            stream: false,
            system: system_prompt,
            tools: None,
        };

        match provider.complete(request).await {
            Ok(response) => {
                let summary_text = response.content.iter()
                    .filter_map(|block| match block {
                        crate::ai::types::ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let compacted = vec![
                    Message::new_assistant(format!("**Conversation Summary:**\n\n{}", summary_text))
                ];

                let _ = event_tx.send(AppEvent::MessagesUpdated(compacted)).await;
                let _ = event_tx.send(AppEvent::StatusMessage("Conversation compacted".to_string())).await;
            }
            Err(e) => {
                let _ = event_tx.send(AppEvent::StreamError(format!("Compaction failed: {}", e))).await;
            }
        }
    });

    CommandResult::Message(format!(
        "Compacting {} messages (~{} tokens)...",
        message_count, token_estimate
    ))
}

fn create_compact_message(messages: &[Message]) -> (Option<String>, Message) {
    let conversation_text = messages
        .iter()
        .map(|m| format!("{}: {}", m.role, m.text_content()))
        .collect::<Vec<_>>()
        .join("\n\n");

    let system_prompt = "You are creating a concise but comprehensive summary of a conversation. 
Preserve all key information including:
- Decisions made and why
- Code changes and their purpose
- Files created/modified
- Any errors encountered and solutions
- Unresolved questions or tasks

Format the summary clearly with sections if appropriate.";

    let user_msg = Message::new_user(format!(
        "Summarize this conversation concisely, preserving all important context for continuation:\n\n{}",
        conversation_text
    ));

    (Some(system_prompt.to_string()), user_msg)
}

fn cmd_details(_args: &[&str], app: &mut App) -> CommandResult {
    let message_count = app.session.messages.len();
    let token_estimate = hidden::estimate_tokens(&app.session.messages);
    let provider = &app.config.provider.default;
    let model = app.get_model();

    let details = format!(
        "Session: {} | Messages: {} | Tokens: ~{} | Provider: {} | Model: {}",
        app.session.title, message_count, token_estimate, provider, model
    );

    CommandResult::Message(details)
}

fn cmd_editor(_args: &[&str], _app: &mut App) -> CommandResult {
    // TODO: Implement editor spawning with raw mode handling in future task
    CommandResult::Message("Use Ctrl+E to open editor (coming soon)".to_string())
}

fn cmd_export(args: &[&str], app: &mut App) -> CommandResult {
    let format = args.first().copied().unwrap_or("markdown");
    let default_path = match format {
        "json" => app.working_dir.join("openrust_export.json"),
        "html" => app.working_dir.join("openrust_export.html"),
        _ => app.working_dir.join("openrust_export.md"),
    };
    let path = if args.len() > 1 {
        PathBuf::from(args[1])
    } else {
        default_path
    };

    let result = match format {
        "json" => sharing::export_json(&app.session.messages, &path),
        "html" => sharing::export_html(&app.session.messages, &path),
        _ => sharing::export_markdown(&app.session.messages, &path),
    };

    match result {
        Ok(msg) => CommandResult::Message(msg),
        Err(e) => CommandResult::Error(format!("Export failed: {}", e)),
    }
}

fn cmd_models(args: &[&str], app: &mut App) -> CommandResult {
    let anthropic_models = vec![
        "claude-sonnet-4-20250514",
        "claude-opus-4-20250514",
        "claude-3-haiku-20240307",
    ];
    let openai_models = vec!["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o1-preview"];
    let google_models = vec!["gemini-2.0-flash-exp", "gemini-1.5-pro", "gemini-1.5-flash"];

    if !args.is_empty() {
        let arg = args[0];
        let provider = &app.config.provider.default;

        if arg == "anthropic" || arg == "openai" || arg == "google" || arg == "gemini" {
            return list_models_for_provider(arg, app);
        }

        match provider.as_str() {
            "anthropic" => {
                app.config.provider.anthropic.model = arg.to_string();
                return CommandResult::Message(format!("Switched to model: {}", arg));
            }
            "openai" => {
                app.config.provider.openai.model = arg.to_string();
                return CommandResult::Message(format!("Switched to model: {}", arg));
            }
            "google" | "gemini" => {
                if let Some(ref mut google_cfg) = app.config.provider.google {
                    google_cfg.model = arg.to_string();
                    return CommandResult::Message(format!("Switched to model: {}", arg));
                } else {
                    return CommandResult::Error(
                        "Google provider not configured in config".to_string(),
                    );
                }
            }
            "bedrock" | "aws-bedrock" => {
                if let Some(ref mut bedrock_cfg) = app.config.provider.bedrock {
                    bedrock_cfg.model = arg.to_string();
                    return CommandResult::Message(format!("Switched to model: {}", arg));
                } else {
                    return CommandResult::Error(
                        "Bedrock provider not configured in config".to_string(),
                    );
                }
            }
            "azure" | "azure-openai" => {
                if let Some(ref mut azure_cfg) = app.config.provider.azure {
                    azure_cfg.model = arg.to_string();
                    return CommandResult::Message(format!("Switched to model: {}", arg));
                } else {
                    return CommandResult::Error(
                        "Azure provider not configured in config".to_string(),
                    );
                }
            }
            "openrouter" => {
                if let Some(ref mut openrouter_cfg) = app.config.provider.openrouter {
                    openrouter_cfg.model = arg.to_string();
                    return CommandResult::Message(format!("Switched to model: {}", arg));
                } else {
                    return CommandResult::Error(
                        "OpenRouter provider not configured in config".to_string(),
                    );
                }
            }
            "ollama" => {
                if let Some(ref mut ollama_cfg) = app.config.provider.ollama {
                    ollama_cfg.model = arg.to_string();
                    return CommandResult::Message(format!("Switched to model: {}", arg));
                } else {
                    return CommandResult::Error(
                        "Ollama provider not configured in config".to_string(),
                    );
                }
            }
            custom_name => {
                if let Some(custom_cfg) = app.config.provider.custom.get_mut(custom_name) {
                    custom_cfg.model = arg.to_string();
                    return CommandResult::Message(format!("Switched to model: {}", arg));
                } else {
                    return CommandResult::Error(format!("Unknown provider: {}", provider));
                }
            }
        }
    }

    let current_model = app.get_model();
    let provider = &app.config.provider.default;

    let mut output = format!(
        "Current provider: {}\nCurrent model: {}\n\n",
        provider, current_model
    );

    output.push_str("Available Anthropic models:\n");
    for model in &anthropic_models {
        if provider == "anthropic" && *model == current_model {
            output.push_str(&format!("  * {} (current)\n", model));
        } else {
            output.push_str(&format!("  - {}\n", model));
        }
    }

    output.push_str("\nAvailable OpenAI models:\n");
    for model in &openai_models {
        if provider == "openai" && *model == current_model {
            output.push_str(&format!("  * {} (current)\n", model));
        } else {
            output.push_str(&format!("  - {}\n", model));
        }
    }

    output.push_str("\nAvailable Google (Gemini) models:\n");
    for model in &google_models {
        if (provider == "google" || provider == "gemini") && *model == current_model {
            output.push_str(&format!("  * {} (current)\n", model));
        } else {
            output.push_str(&format!("  - {}\n", model));
        }
    }

    output.push_str("\nAvailable Bedrock models:\n");
    output.push_str("  - anthropic.claude-v2\n");
    output.push_str("  - anthropic.claude-v2:1\n");
    output.push_str("  - anthropic.claude-3-sonnet\n");
    output.push_str("  Note: Configure bedrock.model in your config file\n");

    output.push_str("\nAvailable Azure OpenAI models:\n");
    output.push_str(
        "  Note: Models depend on your deployment. Configure azure.model in your config file\n",
    );

    output.push_str("\nAvailable OpenRouter models:\n");
    output.push_str("  - anthropic/claude-opus-4\n");
    output.push_str("  - anthropic/claude-sonnet-4\n");
    output.push_str("  - openai/gpt-4o\n");
    output.push_str("  Note: OpenRouter supports many models. See https://openrouter.ai/models\n");

    output.push_str("\nAvailable Ollama models:\n");
    output.push_str("  Note: Run `ollama list` to see installed models. Configure ollama.model in your config file\n");

    output.push_str("\nUsage:\n");
    output.push_str("  /models <model_name>      - Switch to a specific model\n");
    output.push_str("  /models <provider>        - List models for a specific provider\n");

    CommandResult::Message(output)
}

fn list_models_for_provider(provider: &str, app: &App) -> CommandResult {
    let current_model = app.get_model();
    let current_provider = &app.config.provider.default;

    let mut output = String::new();

    match provider {
        "anthropic" => {
            output.push_str("Anthropic (Claude) models:\n");
            let models = vec![
                "claude-sonnet-4-20250514",
                "claude-opus-4-20250514",
                "claude-3-haiku-20240307",
            ];
            for model in &models {
                if current_provider == "anthropic" && *model == current_model {
                    output.push_str(&format!("  * {} (current)\n", model));
                } else {
                    output.push_str(&format!("  - {}\n", model));
                }
            }
        }
        "openai" => {
            output.push_str("OpenAI models:\n");
            let models = vec!["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o1-preview"];
            for model in &models {
                if current_provider == "openai" && *model == current_model {
                    output.push_str(&format!("  * {} (current)\n", model));
                } else {
                    output.push_str(&format!("  - {}\n", model));
                }
            }
        }
        "google" | "gemini" => {
            output.push_str("Google (Gemini) models:\n");
            let models = vec!["gemini-2.0-flash-exp", "gemini-1.5-pro", "gemini-1.5-flash"];
            for model in &models {
                if (current_provider == "google" || current_provider == "gemini")
                    && *model == current_model
                {
                    output.push_str(&format!("  * {} (current)\n", model));
                } else {
                    output.push_str(&format!("  - {}\n", model));
                }
            }
        }
        _ => {
            return CommandResult::Error(format!("Unknown provider: {}", provider));
        }
    }

    CommandResult::Message(output)
}

fn cmd_themes(args: &[&str], app: &mut App) -> CommandResult {
    let available_themes = crate::ui::theme::list_themes();
    let current_theme = &app.config.ui.theme;

    if !args.is_empty() {
        let new_theme = args[0];
        if available_themes.contains(&new_theme) || new_theme == "dark" || new_theme == "light" {
            app.config.ui.theme = new_theme.to_string();
            return CommandResult::Message(format!("Switched to theme: {}", new_theme));
        } else {
            return CommandResult::Error(format!("Unknown theme: {}", new_theme));
        }
    }

    let mut output = format!("Current theme: {}\n\nAvailable themes:\n", current_theme);
    for theme in &available_themes {
        if *theme == current_theme {
            output.push_str(&format!("  * {} (current)\n", theme));
        } else {
            output.push_str(&format!("  - {}\n", theme));
        }
    }

    CommandResult::Message(output)
}

fn cmd_thinking(_args: &[&str], _app: &mut App) -> CommandResult {
    CommandResult::Message("Extended thinking display toggle not yet implemented".to_string())
}

fn cmd_connect(_args: &[&str], _app: &mut App) -> CommandResult {
    CommandResult::Message("MCP server connection not yet implemented".to_string())
}

fn cmd_share(_args: &[&str], app: &mut App) -> CommandResult {
    match sharing::share_conversation(&app.session.messages, &app.session.id, &app.working_dir) {
        Ok(msg) => CommandResult::Message(msg),
        Err(e) => CommandResult::Error(format!("Share failed: {}", e)),
    }
}

fn cmd_unshare(_args: &[&str], app: &mut App) -> CommandResult {
    match sharing::unshare_conversation(&app.session.id, &app.working_dir) {
        Ok(msg) => CommandResult::Message(msg),
        Err(e) => CommandResult::Error(format!("Unshare failed: {}", e)),
    }
}

fn cmd_undo(_args: &[&str], app: &mut App) -> CommandResult {
    match app.undo_manager.undo() {
        Ok(Some(msg)) => CommandResult::Message(msg),
        Ok(None) => CommandResult::Message("Nothing to undo".to_string()),
        Err(e) => CommandResult::Error(format!("Undo failed: {}", e)),
    }
}

fn cmd_redo(_args: &[&str], app: &mut App) -> CommandResult {
    match app.undo_manager.redo() {
        Ok(Some(msg)) => CommandResult::Message(msg),
        Ok(None) => CommandResult::Message("Nothing to redo".to_string()),
        Err(e) => CommandResult::Error(format!("Redo failed: {}", e)),
    }
}

fn cmd_init(_args: &[&str], app: &mut App) -> CommandResult {
    let config_path = app.working_dir.join("openrust.json");

    if config_path.exists() {
        return CommandResult::Error("openrust.json already exists".to_string());
    }

    let default_config = crate::config::Config::default();
    match serde_json::to_string_pretty(&default_config) {
        Ok(json_content) => match fs::write(&config_path, json_content) {
            Ok(_) => CommandResult::Message(format!(
                "Created openrust.json at {}",
                config_path.display()
            )),
            Err(e) => CommandResult::Error(format!("Failed to write config file: {}", e)),
        },
        Err(e) => CommandResult::Error(format!("Failed to serialize config: {}", e)),
    }
}

fn execute_custom_command(
    custom_cmd: &CustomCommandConfig,
    args: &[&str],
    app: &mut App,
) -> CommandResult {
    let mut prompt = custom_cmd.prompt.clone();

    prompt = prompt.replace("$SELECTION", &args.join(" "));

    if prompt.contains("$GIT_DIFF") {
        let diff = std::process::Command::new("git")
            .args(["diff"])
            .current_dir(&app.working_dir)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        prompt = prompt.replace("$GIT_DIFF", &diff);
    }

    if prompt.contains("$FILE_LIST") {
        let file_list = std::process::Command::new("ls")
            .current_dir(&app.working_dir)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        prompt = prompt.replace("$FILE_LIST", &file_list);
    }

    if prompt.contains("$CURRENT_DIR") {
        let current_dir = app.working_dir.display().to_string();
        prompt = prompt.replace("$CURRENT_DIR", &current_dir);
    }

    if prompt.contains("$TIMESTAMP") {
        let timestamp = chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string();
        prompt = prompt.replace("$TIMESTAMP", &timestamp);
    }

    app.input = prompt;
    app.cursor_pos = app.input.len();
    CommandResult::Message("Custom command loaded into input. Press Enter to send.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::{Message, Role};
    use crate::config::Config;

    fn test_app() -> App {
        let config = Config::default();
        App::new(config)
    }

    #[test]
    fn test_parse_slash_command() {
        let registry = CommandRegistry::new();
        let result = registry.parse("/help");
        assert_eq!(result, Some(("help", vec![])));
    }

    #[test]
    fn test_parse_not_command() {
        let registry = CommandRegistry::new();
        let result = registry.parse("not a command");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_with_args() {
        let registry = CommandRegistry::new();
        let result = registry.parse("/export my-file.md");
        assert_eq!(result, Some(("export", vec!["my-file.md"])));
    }

    #[test]
    fn test_completions_partial() {
        let registry = CommandRegistry::new();
        let completions = registry.completions("/ex");
        assert_eq!(completions, vec!["/exit", "/export"]);
    }

    #[test]
    fn test_completions_empty() {
        let registry = CommandRegistry::new();
        let completions = registry.completions("/");
        assert_eq!(completions.len(), 17);
        assert!(completions.contains(&"/new".to_string()));
        assert!(completions.contains(&"/help".to_string()));
        assert!(completions.contains(&"/models".to_string()));
        assert!(completions.contains(&"/themes".to_string()));
    }

    #[test]
    fn test_completions_no_match() {
        let registry = CommandRegistry::new();
        let completions = registry.completions("/xyz");
        assert_eq!(completions, Vec::<String>::new());
    }

    #[test]
    fn test_unknown_command_error() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("unknown", &[], &mut app);
        match result {
            CommandResult::Error(msg) => {
                assert!(msg.contains("Unknown command"));
                assert!(msg.contains("/unknown"));
            }
            _ => panic!("Expected Error result"),
        }
    }

    #[test]
    fn test_export_format() {
        let mut app = test_app();
        app.session.add_message(Role::User, "Hello AI".to_string());
        app.session
            .add_message(Role::Assistant, "Hello human!".to_string());

        let markdown = app.export_conversation().unwrap();

        assert!(markdown.contains("# Session:"));
        assert!(markdown.contains("## User"));
        assert!(markdown.contains("Hello AI"));
        assert!(markdown.contains("## Assistant"));
        assert!(markdown.contains("Hello human!"));
    }

    #[test]
    fn test_new_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        app.session
            .add_message(Role::User, "Old message".to_string());

        let result = registry.execute("new", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("new session"));
            }
            _ => panic!("Expected Message result"),
        }

        assert_eq!(app.session.messages.len(), 0);
    }

    #[test]
    fn test_help_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("help", &[], &mut app);

        match result {
            CommandResult::ModeChange(mode) => {
                assert_eq!(mode, AppMode::Help);
            }
            _ => panic!("Expected ModeChange result"),
        }
    }

    #[test]
    fn test_sessions_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("sessions", &[], &mut app);

        match result {
            CommandResult::ModeChange(mode) => {
                assert_eq!(mode, AppMode::SessionList);
            }
            _ => panic!("Expected ModeChange result"),
        }
    }

    #[test]
    fn test_exit_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("exit", &[], &mut app);

        match result {
            CommandResult::Quit => {}
            _ => panic!("Expected Quit result"),
        }
    }

    #[test]
    fn test_quit_alias() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("quit", &[], &mut app);

        match result {
            CommandResult::Quit => {}
            _ => panic!("Expected Quit result"),
        }
    }

    #[test]
    fn test_details_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        app.session.add_message(Role::User, "Hello".to_string());

        let result = registry.execute("details", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Messages: 1"));
                assert!(msg.contains("Tokens:"));
                assert!(msg.contains("Provider:"));
                assert!(msg.contains("Model:"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_compact_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("compact", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("not yet implemented"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_editor_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("editor", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("coming soon"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_lists_models() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("models", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("claude-sonnet-4-20250514"));
                assert!(msg.contains("gpt-4o"));
                assert!(msg.contains("Current model:"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_switch() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("models", &["claude-3-haiku-20240307"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Switched to model: claude-3-haiku-20240307"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_themes_command_lists_themes() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("themes", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("dark"));
                assert!(msg.contains("light"));
                assert!(msg.contains("Current theme:"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_themes_command_switch() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("themes", &["light"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Switched to theme: light"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_init_command() {
        use std::fs;
        use tempfile::TempDir;

        let registry = CommandRegistry::new();
        let temp_dir = TempDir::new().unwrap();
        let mut app = test_app();
        app.working_dir = temp_dir.path().to_path_buf();

        let result = registry.execute("init", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Created openrust.json"));
                let config_path = app.working_dir.join("openrust.json");
                assert!(config_path.exists());
                let content = fs::read_to_string(&config_path).unwrap();
                assert!(content.contains("provider"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_thinking_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("thinking", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("not yet implemented"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_connect_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("connect", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("not yet implemented"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_share_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("share", &[], &mut app);

        match result {
            CommandResult::Message(_) | CommandResult::Error(_) => {}
            _ => panic!("Expected Message or Error result"),
        }
    }

    #[test]
    fn test_unshare_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("unshare", &[], &mut app);

        match result {
            CommandResult::Message(_) | CommandResult::Error(_) => {}
            _ => panic!("Expected Message or Error result"),
        }
    }

    #[test]
    fn test_undo_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("undo", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("not yet implemented"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_redo_command() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("redo", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("not yet implemented"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_completions_includes_new_commands() {
        let registry = CommandRegistry::new();
        let completions = registry.completions("/");
        assert!(completions.contains(&"/models".to_string()));
        assert!(completions.contains(&"/themes".to_string()));
        assert!(completions.contains(&"/thinking".to_string()));
        assert!(completions.contains(&"/connect".to_string()));
        assert!(completions.contains(&"/share".to_string()));
        assert!(completions.contains(&"/unshare".to_string()));
        assert!(completions.contains(&"/undo".to_string()));
        assert!(completions.contains(&"/redo".to_string()));
        assert!(completions.contains(&"/init".to_string()));
    }

    #[test]
    fn test_models_command_lists_all_providers() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("models", &[], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Current provider:"));
                assert!(msg.contains("Anthropic models"));
                assert!(msg.contains("OpenAI models"));
                assert!(msg.contains("Google (Gemini) models"));
                assert!(msg.contains("Bedrock models"));
                assert!(msg.contains("Azure OpenAI models"));
                assert!(msg.contains("OpenRouter models"));
                assert!(msg.contains("Ollama models"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_switch_anthropic() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        app.config.provider.default = "anthropic".to_string();
        let result = registry.execute("models", &["claude-3-haiku-20240307"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Switched to model"));
                assert_eq!(
                    app.config.provider.anthropic.model,
                    "claude-3-haiku-20240307"
                );
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_switch_openai() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        app.config.provider.default = "openai".to_string();
        let result = registry.execute("models", &["gpt-4o-mini"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Switched to model"));
                assert_eq!(app.config.provider.openai.model, "gpt-4o-mini");
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_filter_by_provider() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        let result = registry.execute("models", &["anthropic"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Anthropic (Claude) models"));
                assert!(msg.contains("claude-sonnet-4-20250514"));
                assert!(msg.contains("claude-opus-4-20250514"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_switch_google() {
        use crate::config::GoogleProviderConfig;
        let registry = CommandRegistry::new();
        let mut app = test_app();
        app.config.provider.default = "google".to_string();
        app.config.provider.google = Some(GoogleProviderConfig {
            api_key: Some("test_key".to_string()),
            model: "gemini-2.0-flash-exp".to_string(),
            max_tokens: 8192,
        });

        let result = registry.execute("models", &["gemini-1.5-pro"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Switched to model"));
                assert_eq!(
                    app.config.provider.google.as_ref().unwrap().model,
                    "gemini-1.5-pro"
                );
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_models_command_google_not_configured() {
        let registry = CommandRegistry::new();
        let mut app = test_app();
        app.config.provider.default = "google".to_string();
        app.config.provider.google = None;

        let result = registry.execute("models", &["gemini-1.5-pro"], &mut app);

        match result {
            CommandResult::Error(msg) => {
                assert!(msg.contains("Google provider not configured"));
            }
            _ => panic!("Expected Error result"),
        }
    }

    #[test]
    fn test_custom_command_config_parsing() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "test_cmd".to_string(),
            CustomCommandConfig {
                description: "Test command".to_string(),
                prompt: "This is a test prompt with $SELECTION".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        assert!(registry.custom_commands.contains_key("test_cmd"));
        assert_eq!(
            registry.custom_commands["test_cmd"].description,
            "Test command"
        );
    }

    #[test]
    fn test_custom_command_template_substitution() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "review".to_string(),
            CustomCommandConfig {
                description: "Review with args".to_string(),
                prompt: "Please review with these notes: $SELECTION".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        let mut app = test_app();

        let result = registry.execute("review", &["check", "error", "handling"], &mut app);

        match result {
            CommandResult::Message(msg) => {
                assert!(msg.contains("Custom command loaded"));
                assert_eq!(
                    app.input,
                    "Please review with these notes: check error handling"
                );
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_custom_commands_in_completions() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "review".to_string(),
            CustomCommandConfig {
                description: "Review a file".to_string(),
                prompt: "Review $SELECTION".to_string(),
                hidden: false,
            },
        );
        custom_commands.insert(
            "refactor".to_string(),
            CustomCommandConfig {
                description: "Refactor code".to_string(),
                prompt: "Refactor $SELECTION".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);

        let completions = registry.completions("/r");
        assert!(completions.contains(&"/review".to_string()));
        assert!(completions.contains(&"/refactor".to_string()));

        let all_completions = registry.completions("/");
        assert!(all_completions.contains(&"/review".to_string()));
        assert!(all_completions.contains(&"/refactor".to_string()));
        assert!(all_completions.contains(&"/help".to_string()));
    }

    #[test]
    fn test_custom_command_with_selection_substitution() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "analyze".to_string(),
            CustomCommandConfig {
                description: "Analyze file".to_string(),
                prompt: "Analyze $SELECTION".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        let mut app = test_app();

        let result = registry.execute("analyze", &["test.py"], &mut app);

        match result {
            CommandResult::Message(_) => {
                assert_eq!(app.input, "Analyze test.py");
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_custom_command_unknown() {
        let registry = CommandRegistry::new();
        let mut app = test_app();

        let result = registry.execute("nonexistent", &[], &mut app);

        match result {
            CommandResult::Error(msg) => {
                assert!(msg.contains("Unknown command"));
            }
            _ => panic!("Expected Error result"),
        }
    }

    #[test]
    fn test_custom_command_current_dir_substitution() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "showdir".to_string(),
            CustomCommandConfig {
                description: "Show current directory".to_string(),
                prompt: "Working in: $CURRENT_DIR".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        let mut app = test_app();

        let result = registry.execute("showdir", &[], &mut app);

        match result {
            CommandResult::Message(_) => {
                assert!(app.input.starts_with("Working in: "));
                assert!(app.input.contains(&app.working_dir.display().to_string()));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_custom_command_timestamp_substitution() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "log".to_string(),
            CustomCommandConfig {
                description: "Log with timestamp".to_string(),
                prompt: "Log at $TIMESTAMP: $SELECTION".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        let mut app = test_app();

        let result = registry.execute("log", &["test", "message"], &mut app);

        match result {
            CommandResult::Message(_) => {
                assert!(app.input.contains("Log at"));
                assert!(app.input.contains("UTC"));
                assert!(app.input.contains("test message"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_custom_command_git_diff_substitution() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "review_diff".to_string(),
            CustomCommandConfig {
                description: "Review git diff".to_string(),
                prompt: "Review these changes:\n$GIT_DIFF".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        let mut app = test_app();

        let result = registry.execute("review_diff", &[], &mut app);

        match result {
            CommandResult::Message(_) => {
                assert!(app.input.starts_with("Review these changes:\n"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_custom_command_file_list_substitution() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "list".to_string(),
            CustomCommandConfig {
                description: "List files".to_string(),
                prompt: "Files:\n$FILE_LIST".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);
        let mut app = test_app();

        let result = registry.execute("list", &[], &mut app);

        match result {
            CommandResult::Message(_) => {
                assert!(app.input.starts_with("Files:\n"));
            }
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_custom_command_hidden() {
        use std::collections::HashMap;
        let mut custom_commands = HashMap::new();
        custom_commands.insert(
            "hidden_cmd".to_string(),
            CustomCommandConfig {
                description: "Hidden command".to_string(),
                prompt: "This is hidden".to_string(),
                hidden: true,
            },
        );
        custom_commands.insert(
            "visible_cmd".to_string(),
            CustomCommandConfig {
                description: "Visible command".to_string(),
                prompt: "This is visible".to_string(),
                hidden: false,
            },
        );

        let registry = CommandRegistry::new().with_custom_commands(custom_commands);

        let mut app = test_app();
        let result = registry.execute("hidden_cmd", &[], &mut app);
        match result {
            CommandResult::Message(_) => {
                assert_eq!(app.input, "This is hidden");
            }
            _ => panic!("Expected Message result for hidden command execution"),
        }
    }
}
