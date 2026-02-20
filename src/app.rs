use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::ai::types::{CompletionRequest, Message, Role};
use crate::ai::{create_provider, Provider};
use crate::config::Config;
use crate::session::Session;
use crate::tools::ToolSet;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Chat,
    SessionList,
    Help,
    Quit,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    StreamChunk(String),
    StreamComplete,
    StreamError(String),
    StatusMessage(String),
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
        let provider_name = &self.config.provider.default;
        let (api_key, base_url) = match provider_name.as_str() {
            "anthropic" => {
                let key = self.config.provider.anthropic.api_key.clone()?;
                (key, None)
            }
            "openai" => {
                let key = self.config.provider.openai.api_key.clone()?;
                (key, self.config.provider.openai.base_url.clone())
            }
            _ => return None,
        };

        Some(create_provider(provider_name, api_key, base_url))
    }

    pub fn get_model(&self) -> String {
        match self.config.provider.default.as_str() {
            "anthropic" => self.config.provider.anthropic.model.clone(),
            "openai" => self.config.provider.openai.model.clone(),
            _ => self.config.provider.anthropic.model.clone(),
        }
    }

    pub fn get_max_tokens(&self) -> u32 {
        match self.config.provider.default.as_str() {
            "anthropic" => self.config.provider.anthropic.max_tokens,
            "openai" => self.config.provider.openai.max_tokens,
            _ => 8192,
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
                self.status_message = Some("Error: No API key configured. Set ANTHROPIC_API_KEY or OPENAI_API_KEY".to_string());
                return Ok(());
            }
        };

        let messages = self.session.messages.clone();
        let model = self.get_model();
        let max_tokens = self.get_max_tokens();
        let event_tx = self.event_tx.clone();

        let system_prompt = build_system_prompt(&self.working_dir);

        tokio::spawn(async move {
            let request = CompletionRequest {
                messages,
                model,
                max_tokens,
                system: Some(system_prompt),
                stream: true,
            };

            let (chunk_tx, mut chunk_rx) = mpsc::channel::<Result<String, crate::ai::types::AiError>>(100);

            let stream_task = tokio::spawn(async move {
                provider.complete_stream(request, chunk_tx).await
            });

            while let Some(chunk) = chunk_rx.recv().await {
                match chunk {
                    Ok(text) => {
                        let _ = event_tx.send(AppEvent::StreamChunk(text)).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::StreamError(e.to_string())).await;
                        return;
                    }
                }
            }

            match stream_task.await {
                Ok(Ok(())) => {
                    let _ = event_tx.send(AppEvent::StreamComplete).await;
                }
                Ok(Err(e)) => {
                    let _ = event_tx.send(AppEvent::StreamError(e.to_string())).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::StreamError(e.to_string())).await;
                }
            }
        });

        Ok(())
    }

    pub fn apply_stream_chunk(&mut self, chunk: String) {
        if let Some(last_msg) = self.session.messages.last_mut() {
            if last_msg.role == Role::Assistant {
                last_msg.content.push_str(&chunk);
                return;
            }
        }
        // First chunk - create assistant message
        self.session.messages.push(Message {
            role: Role::Assistant,
            content: chunk,
        });
    }

    pub fn finish_stream(&mut self) {
        self.is_loading = false;
        self.status_message = None;
        let _ = self.session.save();
    }

    pub fn handle_stream_error(&mut self, error: String) {
        self.is_loading = false;
        self.status_message = Some(format!("Error: {}", error));
        self.session.messages.push(Message {
            role: Role::Assistant,
            content: format!("Error: {}", error),
        });
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

    fn count_message_lines(&self) -> usize {
        self.session
            .messages
            .iter()
            .map(|m| m.content.lines().count() + 2)
            .sum()
    }
}

fn build_system_prompt(working_dir: &PathBuf) -> String {
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
