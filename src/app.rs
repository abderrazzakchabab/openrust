use anyhow::Result;
use regex::Regex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, oneshot};

use crate::agent_loop;
use crate::agents::AgentDispatch;
use crate::ai::types::{ContentBlock, Message, Role};
use crate::ai::{create_provider, Provider};
use crate::config::Config;
use crate::mcp::McpManager;
use crate::permissions::PermissionChecker;
use crate::session::Session;
use crate::tools::ToolSet;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Chat,
    SessionList,
    Help,
    Quit,
    PermissionPrompt,
    QuestionPrompt,
}

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub tool_name: String,
    pub description: String,
    pub input_json: String,
}

#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Debug)]
pub enum AppEvent {
    StreamChunk(String),
    StreamComplete,
    StreamError(String),
    StatusMessage(String),
    ToolCallStart {
        name: String,
        input: String,
    },
    ToolCallComplete {
        name: String,
        output: String,
        is_error: bool,
    },
    MessagesUpdated(Vec<Message>),
    PermissionRequest {
        tool_name: String,
        description: String,
        input_json: String,
        response_tx: oneshot::Sender<PermissionResponse>,
    },
    QuestionPrompt {
        question: String,
        options: Vec<String>,
        response_tx: oneshot::Sender<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionResponse {
    AllowOnce,
    AllowSession,
    Deny,
}

pub struct App {
    pub config: Config,
    pub session: Session,
    pub mode: AppMode,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: usize,
    pub is_loading: bool,
    pub status_message: Option<String>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub tools: ToolSet,
    pub working_dir: PathBuf,
    pub agent_dispatch: AgentDispatch,
    pub current_agent: String,
    pub tab_completions: Vec<String>,
    pub tab_index: Option<usize>,
    pub leader_active: bool,
    pub permissions: Arc<Mutex<PermissionChecker>>,
    pub mcp_manager: Option<Arc<Mutex<McpManager>>>,
    pub undo_manager: crate::undo::UndoManager,
    pub pending_permission: Option<PendingPermission>,
    pub pending_question: Option<PendingQuestion>,
    pub permission_response_tx: Option<oneshot::Sender<PermissionResponse>>,
    pub question_response_tx: Option<oneshot::Sender<String>>,
    pub selected_option: usize,
}

impl App {
    pub fn new(config: Config) -> Self {
        let working_dir = config
            .tools
            .working_directory
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let provider_name = &config.provider.default;
        let model = match provider_name.as_str() {
            "anthropic" => config.provider.anthropic.model.clone(),
            "openai" => config.provider.openai.model.clone(),
            _ => config.provider.anthropic.model.clone(),
        };

        let session = Session::new(provider_name, &model);
        let tools = ToolSet::new(working_dir.clone());
        let (event_tx, event_rx) = mpsc::channel(1000);
        let agent_dispatch = AgentDispatch::new();
        let current_agent = "build".to_string();
        let permissions = Arc::new(Mutex::new(PermissionChecker::new(&config.permissions)));

        let mcp_manager = if !config.mcp_servers.is_empty() {
            Some(Arc::new(Mutex::new(McpManager::new())))
        } else {
            None
        };

        let undo_manager = crate::undo::UndoManager::new(working_dir.clone());

        App {
            config,
            session,
            mode: AppMode::Chat,
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            is_loading: false,
            status_message: None,
            event_tx,
            event_rx,
            tools,
            working_dir,
            agent_dispatch,
            current_agent,
            tab_completions: Vec::new(),
            tab_index: None,
            leader_active: false,
            permissions,
            mcp_manager,
            undo_manager,
            pending_permission: None,
            pending_question: None,
            permission_response_tx: None,
            question_response_tx: None,
            selected_option: 0,
        }
    }

    pub fn input_push(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn input_backspace(&mut self) {
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while pos > 0 && !self.input.is_char_boundary(pos) {
                pos -= 1;
            }
            self.input.remove(pos);
            self.cursor_pos = pos;
        }
    }

    pub fn input_move_left(&mut self) {
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while pos > 0 && !self.input.is_char_boundary(pos) {
                pos -= 1;
            }
            self.cursor_pos = pos;
        }
    }

    pub fn input_move_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let mut pos = self.cursor_pos + 1;
            while pos < self.input.len() && !self.input.is_char_boundary(pos) {
                pos += 1;
            }
            self.cursor_pos = pos;
        }
    }

    pub fn input_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn input_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    pub fn take_input(&mut self) -> String {
        let input = std::mem::take(&mut self.input);
        self.cursor_pos = 0;
        input
    }

    pub fn get_provider(&self) -> Option<Box<dyn Provider>> {
        use crate::ai::ProviderOptions;
        use std::collections::HashMap;

        let provider_name = &self.config.provider.default;

        match provider_name.as_str() {
            "anthropic" => {
                let key = self.config.provider.anthropic.api_key.clone()?;
                create_provider(provider_name, key, None, None, None).ok()
            }
            "openai" => {
                let key = self.config.provider.openai.api_key.clone()?;
                let base_url = self.config.provider.openai.base_url.clone();
                create_provider(provider_name, key, base_url, None, None).ok()
            }
            "google" | "gemini" => {
                let google_cfg = self.config.provider.google.as_ref()?;
                let key = google_cfg
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
                    .or_else(|| std::env::var("GEMINI_API_KEY").ok())?;
                create_provider(provider_name, key, None, None, None).ok()
            }
            "bedrock" | "aws-bedrock" => {
                let bedrock_cfg = self.config.provider.bedrock.as_ref()?;
                let access_key = bedrock_cfg
                    .access_key
                    .clone()
                    .or_else(|| std::env::var("AWS_ACCESS_KEY_ID").ok())?;
                let secret_key = bedrock_cfg
                    .secret_key
                    .clone()
                    .or_else(|| std::env::var("AWS_SECRET_ACCESS_KEY").ok())?;
                let session_token = bedrock_cfg
                    .session_token
                    .clone()
                    .or_else(|| std::env::var("AWS_SESSION_TOKEN").ok());
                let opts = ProviderOptions {
                    region: bedrock_cfg.region.clone(),
                    secret_key: Some(secret_key),
                    session_token,
                    model_id: Some(bedrock_cfg.model.clone()),
                    ..Default::default()
                };
                create_provider(provider_name, access_key, None, None, Some(opts)).ok()
            }
            "azure" | "azure-openai" => {
                let azure_cfg = self.config.provider.azure.as_ref()?;
                let key = azure_cfg
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("AZURE_OPENAI_API_KEY").ok())?;
                let opts = ProviderOptions {
                    endpoint: Some(azure_cfg.endpoint.clone()),
                    deployment: Some(azure_cfg.deployment.clone()),
                    api_version: azure_cfg.api_version.clone(),
                    ..Default::default()
                };
                create_provider(provider_name, key, None, None, Some(opts)).ok()
            }
            "openrouter" => {
                let openrouter_cfg = self.config.provider.openrouter.as_ref()?;
                let key = openrouter_cfg
                    .api_key
                    .clone()
                    .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())?;
                create_provider(provider_name, key, None, None, None).ok()
            }
            "ollama" => {
                let ollama_cfg = self.config.provider.ollama.as_ref()?;
                let base_url = ollama_cfg.base_url.clone();
                create_provider(provider_name, String::new(), base_url, None, None).ok()
            }
            "generic" | "openai-compatible" => {
                let custom_name = &self.config.provider.default;
                let custom_cfg = self.config.provider.custom.get(custom_name)?;
                let key = custom_cfg.api_key.clone().unwrap_or_default();
                let base_url = Some(custom_cfg.base_url.clone());
                let extra_headers = if custom_cfg.extra_headers.is_empty() {
                    None
                } else {
                    Some(custom_cfg.extra_headers.clone())
                };
                create_provider("generic", key, base_url, extra_headers, None).ok()
            }
            custom_name => {
                let custom_cfg = self.config.provider.custom.get(custom_name)?;
                let key = custom_cfg.api_key.clone().unwrap_or_default();
                let base_url = Some(custom_cfg.base_url.clone());
                let extra_headers = if custom_cfg.extra_headers.is_empty() {
                    None
                } else {
                    Some(custom_cfg.extra_headers.clone())
                };
                create_provider("generic", key, base_url, extra_headers, None).ok()
            }
        }
    }

    pub fn get_model(&self) -> String {
        match self.config.provider.default.as_str() {
            "anthropic" => self.config.provider.anthropic.model.clone(),
            "openai" => self.config.provider.openai.model.clone(),
            "google" | "gemini" => self
                .config
                .provider
                .google
                .as_ref()
                .map(|g| g.model.clone())
                .unwrap_or_else(|| "gemini-2.0-flash-exp".to_string()),
            "bedrock" | "aws-bedrock" => self
                .config
                .provider
                .bedrock
                .as_ref()
                .map(|b| b.model.clone())
                .unwrap_or_else(|| "anthropic.claude-v2".to_string()),
            "azure" | "azure-openai" => self
                .config
                .provider
                .azure
                .as_ref()
                .map(|a| a.model.clone())
                .unwrap_or_else(|| "gpt-4".to_string()),
            "openrouter" => self
                .config
                .provider
                .openrouter
                .as_ref()
                .map(|o| o.model.clone())
                .unwrap_or_else(|| "anthropic/claude-opus-4".to_string()),
            "ollama" => self
                .config
                .provider
                .ollama
                .as_ref()
                .map(|o| o.model.clone())
                .unwrap_or_else(|| "llama2".to_string()),
            custom_name => self
                .config
                .provider
                .custom
                .get(custom_name)
                .map(|c| c.model.clone())
                .unwrap_or_else(|| self.config.provider.anthropic.model.clone()),
        }
    }

    pub fn get_max_tokens(&self) -> u32 {
        match self.config.provider.default.as_str() {
            "anthropic" => self.config.provider.anthropic.max_tokens,
            "openai" => self.config.provider.openai.max_tokens,
            "google" | "gemini" => self
                .config
                .provider
                .google
                .as_ref()
                .map(|g| g.max_tokens)
                .unwrap_or(8192),
            "bedrock" | "aws-bedrock" => self
                .config
                .provider
                .bedrock
                .as_ref()
                .map(|b| b.max_tokens)
                .unwrap_or(8192),
            "azure" | "azure-openai" => self
                .config
                .provider
                .azure
                .as_ref()
                .map(|a| a.max_tokens)
                .unwrap_or(8192),
            "openrouter" => self
                .config
                .provider
                .openrouter
                .as_ref()
                .map(|o| o.max_tokens)
                .unwrap_or(8192),
            "ollama" => self
                .config
                .provider
                .ollama
                .as_ref()
                .map(|o| o.max_tokens)
                .unwrap_or(8192),
            custom_name => self
                .config
                .provider
                .custom
                .get(custom_name)
                .map(|c| c.max_tokens)
                .unwrap_or(8192),
        }
    }

    pub async fn send_message(&mut self, content: String) -> Result<()> {
        self.session.add_message(Role::User, content);
        self.is_loading = true;
        self.status_message = Some("Thinking...".to_string());

        let provider = match self.get_provider() {
            Some(p) => p,
            None => {
                self.is_loading = false;
                self.status_message = Some(
                    "Error: No API key configured. Set ANTHROPIC_API_KEY or OPENAI_API_KEY"
                        .to_string(),
                );
                return Ok(());
            }
        };

        let messages = self.session.messages.clone();
        let model = self.get_model();
        let max_tokens = self.get_max_tokens();
        let event_tx = self.event_tx.clone();

        let base_system_prompt = build_system_prompt(&self.working_dir);
        let merged_rules = crate::rules::load_merged_rules(&self.working_dir, &self.config.rules);
        let system_prompt = crate::rules::prepend_rules(base_system_prompt, merged_rules);

        let registry = Arc::new(crate::tools::create_tool_registry_with_custom(
            self.working_dir.clone(),
            &self.config.custom_tools,
        ));
        let tool_defs = {
            let defs = registry.to_definitions();
            if defs.is_empty() {
                None
            } else {
                Some(defs)
            }
        };

        let agent_config = self.agent_dispatch.get(&self.current_agent).cloned();
        let permissions = Arc::clone(&self.permissions);
        let mcp_manager = self.mcp_manager.clone();

        tokio::spawn(async move {
            let final_messages = agent_loop::run_agent_loop(
                provider,
                messages,
                model,
                max_tokens,
                Some(system_prompt),
                tool_defs,
                registry,
                event_tx.clone(),
                agent_config.as_ref(),
                permissions,
                mcp_manager,
            )
            .await;

            let _ = event_tx
                .send(AppEvent::MessagesUpdated(final_messages))
                .await;
            let _ = event_tx.send(AppEvent::StreamComplete).await;
        });

        Ok(())
    }

    pub fn apply_stream_chunk(&mut self, chunk: String) {
        if let Some(last_msg) = self.session.messages.last_mut() {
            if last_msg.role == Role::Assistant {
                last_msg.append_text(&chunk);
                return;
            }
        }
        // First chunk - create assistant message
        self.session.messages.push(Message::new_assistant(chunk));
    }

    pub fn finish_stream(&mut self) {
        self.is_loading = false;
        self.status_message = None;
        let _ = self.session.save();
    }

    pub fn handle_stream_error(&mut self, error: String) {
        self.is_loading = false;
        self.status_message = Some(format!("Error: {}", error));
        self.session
            .messages
            .push(Message::new_assistant(format!("Error: {}", error)));
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self, max_lines: usize) {
        let total_lines = self.count_message_lines();
        if self.scroll_offset + max_lines < total_lines {
            self.scroll_offset += 1;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX;
    }

    pub fn clear_tab_completions(&mut self) {
        self.tab_completions.clear();
        self.tab_index = None;
    }

    pub fn export_conversation(&self) -> Result<String> {
        let mut markdown = String::new();
        markdown.push_str(&format!("# Session: {}\n\n", self.session.title));

        for message in &self.session.messages {
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
                        markdown
                            .push_str(&format!("[tool_use: {} with input: {}]\n\n", name, input));
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        markdown.push_str(&format!("[tool_result: {}]\n\n", content));
                    }
                    ContentBlock::Thinking { thinking } => {
                        markdown.push_str(&format!("[thinking: {}]\n\n", thinking));
                    }
                }
            }
        }

        Ok(markdown)
    }

    pub fn process_input_references(&self, input: &str) -> String {
        let re = Regex::new(r"@([^\s]+)").unwrap();
        let mut result = input.to_string();

        for cap in re.captures_iter(input) {
            let path_str = &cap[1];
            let full_match = &cap[0];
            let resolved = if std::path::Path::new(path_str).is_absolute() {
                std::path::PathBuf::from(path_str)
            } else {
                self.working_dir.join(path_str)
            };

            let replacement = if !resolved.exists() {
                format!("[File not found: {}]", path_str)
            } else if resolved.is_dir() || path_str.ends_with('/') {
                match std::fs::read_dir(&resolved) {
                    Ok(entries) => {
                        let mut names: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let mut name = e.file_name().to_string_lossy().to_string();
                                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    name.push('/');
                                }
                                name
                            })
                            .collect();
                        names.sort();
                        format!("[Directory: {}]\n{}", path_str, names.join("\n"))
                    }
                    Err(e) => format!("[Error reading directory {}: {}]", path_str, e),
                }
            } else {
                match std::fs::metadata(&resolved) {
                    Ok(meta) if meta.len() > 100_000 => {
                        format!("[File too large: {} ({} bytes)]", path_str, meta.len())
                    }
                    Ok(_) => match std::fs::read_to_string(&resolved) {
                        Ok(content) => {
                            format!("[File: {}]\n```\n{}\n```", path_str, content)
                        }
                        Err(e) => format!("[Error reading file {}: {}]", path_str, e),
                    },
                    Err(e) => format!("[Error accessing file {}: {}]", path_str, e),
                }
            };

            result = result.replace(full_match, &replacement);
        }

        result
    }

    pub async fn execute_bang_command(&mut self, command: &str) -> Result<()> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&stderr);
        }

        let truncated = if result.len() > 500 {
            format!("{}...", &result[..500])
        } else {
            result
        };

        self.status_message = Some(format!("$ {}\n{}", command, truncated));
        Ok(())
    }

    fn count_message_lines(&self) -> usize {
        self.session
            .messages
            .iter()
            .map(|m| {
                m.content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.lines().count()),
                        _ => None,
                    })
                    .sum::<usize>()
                    + 2
            })
            .sum()
    }
}

pub(crate) fn build_system_prompt(working_dir: &PathBuf) -> String {
    let cwd = working_dir.display();
    format!(
        r#"You are OpenRust, an AI coding assistant running in the terminal. You help developers write, debug, and understand code.

Current working directory: {cwd}

You have access to tools for reading files, executing commands, and working with git. When appropriate, use these tools to help the user.

Guidelines:
- Be concise and direct in your responses
- When showing code, use appropriate markdown code blocks with language identifiers
- When you need to read a file or run a command, say so explicitly
- Prefer making small, focused changes rather than rewriting everything
- Always explain what you're doing and why
- Ask for clarification when the request is ambiguous

{tools_desc}"#,
        cwd = cwd,
        tools_desc = crate::tools::tools_description()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time ok")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openrust-app-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn create_test_app(working_dir: PathBuf) -> App {
        let mut config = Config::default();
        config.tools.working_directory = Some(working_dir.to_string_lossy().to_string());
        App::new(config)
    }

    #[test]
    fn test_process_file_reference() {
        let dir = create_temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "Hello, world!").expect("write file");

        let app = create_test_app(dir);
        let result = app.process_input_references("Check @test.txt for content");

        assert!(result.contains("[File: test.txt]"));
        assert!(result.contains("Hello, world!"));
        assert!(result.contains("```"));
    }

    #[test]
    fn test_process_dir_reference() {
        let dir = create_temp_dir();
        fs::write(dir.join("a.txt"), "a").expect("write file");
        fs::write(dir.join("b.txt"), "b").expect("write file");
        fs::create_dir_all(dir.join("subdir")).expect("create subdir");

        let app = create_test_app(dir.clone());
        let result = app.process_input_references("List @./");

        assert!(result.contains("[Directory:"));
        assert!(result.contains("a.txt"));
        assert!(result.contains("b.txt"));
        assert!(result.contains("subdir/"));
    }

    #[test]
    fn test_process_missing_file() {
        let dir = create_temp_dir();
        let app = create_test_app(dir);
        let result = app.process_input_references("Check @nonexistent.txt");

        assert!(result.contains("[File not found: nonexistent.txt]"));
    }

    #[test]
    fn test_process_multiple_references() {
        let dir = create_temp_dir();
        fs::write(dir.join("first.txt"), "First").expect("write file");
        fs::write(dir.join("second.txt"), "Second").expect("write file");

        let app = create_test_app(dir);
        let result = app.process_input_references("Compare @first.txt and @second.txt");

        assert!(result.contains("[File: first.txt]"));
        assert!(result.contains("First"));
        assert!(result.contains("[File: second.txt]"));
        assert!(result.contains("Second"));
    }

    #[test]
    fn test_process_no_references() {
        let dir = create_temp_dir();
        let app = create_test_app(dir);
        let input = "This is plain text with no @ references";
        let result = app.process_input_references(input);

        assert_eq!(result, input);
    }

    #[test]
    fn test_process_large_file() {
        let dir = create_temp_dir();
        let file = dir.join("large.txt");
        let large_content = "x".repeat(150_000);
        fs::write(&file, large_content).expect("write file");

        let app = create_test_app(dir);
        let result = app.process_input_references("Check @large.txt");

        assert!(result.contains("[File too large: large.txt"));
        assert!(result.contains("150000 bytes"));
    }
}
