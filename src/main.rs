mod agent_loop;
mod agents;
mod ai;
mod app;
mod auth;
mod cli;
mod commands;
mod config;
mod formatters;
mod keybinds;
mod lsp;
mod mcp;
mod permissions;
mod plugins;
mod rules;
mod session;
mod sharing;
mod tools;
mod ui;
mod undo;
mod watcher;

#[cfg(test)]
mod test_helpers;

use std::io::{self, IsTerminal, Read};
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppEvent, AppMode};
use config::Config;
use keybinds::{KeyAction, KeybindManager};

#[derive(Parser, Debug)]
#[command(
    name = "openrust",
    about = "OpenRust - AI-powered terminal coding assistant",
    version = env!("CARGO_PKG_VERSION"),
    author = "OpenRust Contributors"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// AI provider to use (anthropic, openai)
    #[arg(short, long)]
    provider: Option<String>,

    /// Model to use
    #[arg(short, long)]
    model: Option<String>,

    /// Working directory
    #[arg(short, long)]
    dir: Option<String>,

    /// Send a single message without launching TUI
    #[arg(long, short = 'M')]
    message: Option<String>,

    /// Output format: text (default), json, markdown
    #[arg(long, default_value = "text")]
    format: String,

    /// Continue the last session instead of starting new
    #[arg(long, short = 'c')]
    r#continue: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List saved sessions
    Sessions,
    /// Show current configuration
    Config,
    /// Clear all saved sessions
    Clear,
    /// Log in to an AI provider via browser
    Login {
        /// Provider name: anthropic, openai
        #[arg(default_value = "anthropic")]
        provider: String,
    },
    /// Log out of an AI provider (removes saved API key)
    Logout {
        /// Provider name: anthropic, openai
        #[arg(default_value = "anthropic")]
        provider: String,
    },
    /// Set the default AI provider (must already be logged in)
    Default {
        /// Provider name: anthropic, openai
        provider: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("openrust=info".parse()?),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // Load config
    let mut config = Config::load()?;

    // Apply CLI overrides
    if let Some(provider) = cli.provider {
        config.provider.default = provider;
    }
    if let Some(model) = cli.model {
        match config.provider.default.as_str() {
            "anthropic" => config.provider.anthropic.model = model,
            "openai" => config.provider.openai.model = model,
            _ => {}
        }
    }
    if let Some(dir) = cli.dir {
        config.tools.working_directory = Some(dir);
    }

    let cli_format = cli.format.clone();
    let cli_continue = cli.r#continue;

    match cli.command {
        Some(Commands::Sessions) => {
            list_sessions();
            return Ok(());
        }
        Some(Commands::Config) => {
            show_config(&config);
            return Ok(());
        }
        Some(Commands::Clear) => {
            clear_sessions()?;
            return Ok(());
        }
        Some(Commands::Login { provider }) => {
            auth::login(&provider).await?;
            return Ok(());
        }
        Some(Commands::Logout { provider }) => {
            auth::logout(&provider)?;
            return Ok(());
        }
        Some(Commands::Default { provider }) => {
            auth::set_default(&provider)?;
            return Ok(());
        }
        None => {}
    }

    let message = if let Some(msg) = cli.message {
        Some(msg)
    } else if !io::stdin().is_terminal() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if !input.trim().is_empty() {
            Some(input.trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    if let Some(msg) = message {
        return cli::run_cli_mode(&config, &msg, &cli_format, cli_continue).await;
    }

    // Run TUI
    run_tui(config).await
}

fn list_sessions() {
    match session::Session::list_all() {
        Ok(sessions) if sessions.is_empty() => println!("No saved sessions."),
        Ok(sessions) => {
            println!(
                "{:<4} {:<50} {:<20} {:>8}",
                "No.", "Title", "Updated", "Messages"
            );
            println!("{}", "-".repeat(86));
            for (i, s) in sessions.iter().enumerate() {
                println!(
                    "{:<4} {:<50} {:<20} {:>8}",
                    i + 1,
                    &s.title[..s.title.len().min(49)],
                    s.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                    s.message_count
                );
            }
        }
        Err(e) => eprintln!("Error listing sessions: {}", e),
    }
}

fn show_config(config: &Config) {
    println!("OpenRust Configuration");
    println!("  Config file:    {}", Config::config_path().display());
    println!("  Provider:       {}", config.provider.default);
    println!("  Anthropic model: {}", config.provider.anthropic.model);
    println!(
        "  Anthropic key:  {}",
        if config.provider.anthropic.api_key.is_some() {
            "configured"
        } else {
            "not set (ANTHROPIC_API_KEY)"
        }
    );
    println!("  OpenAI model:   {}", config.provider.openai.model);
    println!(
        "  OpenAI key:     {}",
        if config.provider.openai.api_key.is_some() {
            "configured"
        } else {
            "not set (OPENAI_API_KEY)"
        }
    );
    println!("  Theme:          {}", config.ui.theme);
    println!(
        "  Sessions dir:   {}",
        session::Session::sessions_dir().display()
    );
}

fn clear_sessions() -> Result<()> {
    let dir = session::Session::sessions_dir();
    if dir.exists() {
        let count = std::fs::read_dir(&dir)?.count();
        std::fs::remove_dir_all(&dir)?;
        println!("Cleared {} sessions.", count);
    } else {
        println!("No sessions to clear.");
    }
    Ok(())
}

async fn run_tui(config: Config) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, config).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: Config,
) -> Result<()> {
    let mut app = App::new(config);
    let mut keybind_manager = KeybindManager::new();

    // Scroll to bottom initially
    app.scroll_to_bottom();

    loop {
        // Draw
        terminal.draw(|f| ui::draw(f, &app))?;

        // Process async events (non-blocking)
        while let Ok(event) = app.event_rx.try_recv() {
            match event {
                AppEvent::StreamChunk(chunk) => {
                    app.apply_stream_chunk(chunk);
                    app.scroll_to_bottom();
                }
                AppEvent::StreamComplete => {
                    app.finish_stream();
                    app.scroll_to_bottom();
                }
                AppEvent::StreamError(err) => {
                    app.handle_stream_error(err);
                }
                AppEvent::StatusMessage(msg) => {
                    app.status_message = Some(msg);
                }
                AppEvent::ToolCallStart {
                    name,
                    input: _input,
                } => {
                    app.status_message = Some(format!("Running tool: {name}..."));
                }
                AppEvent::ToolCallComplete {
                    name,
                    output: _output,
                    is_error,
                } => {
                    app.status_message = if is_error {
                        Some(format!("Tool {name} failed"))
                    } else {
                        Some(format!("Tool {name} completed"))
                    };
                }
                AppEvent::MessagesUpdated(messages) => {
                    app.session.messages = messages;
                    app.scroll_to_bottom();
                }
                AppEvent::PermissionRequest {
                    tool_name,
                    description,
                    input_json,
                    response_tx,
                } => {
                    app.pending_permission = Some(app::PendingPermission {
                        tool_name,
                        description,
                        input_json,
                    });
                    app.permission_response_tx = Some(response_tx);
                    app.mode = app::AppMode::PermissionPrompt;
                }
                AppEvent::QuestionPrompt {
                    question,
                    options,
                    response_tx,
                } => {
                    app.pending_question = Some(app::PendingQuestion {
                        question,
                        options,
                    });
                    app.question_response_tx = Some(response_tx);
                    app.selected_option = 0;
                    app.mode = app::AppMode::QuestionPrompt;
                }
            }
        }

        // Check for keyboard input (with short timeout to stay responsive)
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match app.mode {
                    app::AppMode::PermissionPrompt => {
                        handle_permission_prompt(&mut app, key.code);
                    }
                    app::AppMode::QuestionPrompt => {
                        handle_question_prompt(&mut app, key.code);
                    }
                    app::AppMode::Help => match key.code {
                        KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => {
                            app.mode = app::AppMode::Chat;
                        }
                        _ => {}
                    },
                    app::AppMode::SessionList => match key.code {
                        KeyCode::Esc | KeyCode::Char('l')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.mode = app::AppMode::Chat;
                        }
                        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let provider = &app.config.provider.default.clone();
                            let model = app.get_model();
                            app.session = session::Session::new(provider, &model);
                            app.mode = app::AppMode::Chat;
                        }
                        _ => {}
                    },
                    app::AppMode::Chat => {
                        app.leader_active = keybind_manager.is_leader_active();

                        // Check keybind manager first
                        if let Some(action) = keybind_manager.handle_key(key.code, key.modifiers) {
                            match action {
                                KeyAction::Quit => {
                                    let _ = app.session.save();
                                    app.mode = app::AppMode::Quit;
                                }
                                KeyAction::ShowHelp => {
                                    app.mode = app::AppMode::Help;
                                }
                                KeyAction::ShowSessions => {
                                    app.mode = app::AppMode::SessionList;
                                }
                                KeyAction::NewSession => {
                                    let _ = app.session.save();
                                    let provider = app.config.provider.default.clone();
                                    let model = app.get_model();
                                    app.session = session::Session::new(&provider, &model);
                                    app.scroll_to_bottom();
                                }
                                KeyAction::SaveSession => {
                                    let _ = app.session.save();
                                    app.status_message = Some("Session saved".to_string());
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let _ = app.session.save();
                                    app.mode = app::AppMode::Quit;
                                }
                                KeyCode::F(1) | KeyCode::Char('?') => {
                                    app.mode = app::AppMode::Help;
                                }
                                KeyCode::Char('l')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.mode = app::AppMode::SessionList;
                                }
                                KeyCode::Char('n')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let _ = app.session.save();
                                    let provider = app.config.provider.default.clone();
                                    let model = app.get_model();
                                    app.session = session::Session::new(&provider, &model);
                                    app.scroll_to_bottom();
                                }
                                KeyCode::Char('s')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let _ = app.session.save();
                                    app.status_message = Some("Session saved".to_string());
                                }
                                KeyCode::Enter
                                    if !app.is_loading
                                        && key.modifiers.contains(KeyModifiers::SHIFT) =>
                                {
                                    app.input_push('\n');
                                }
                                KeyCode::Enter if !app.is_loading => {
                                    let input = app.take_input();
                                    app.clear_tab_completions();
                                    if !input.trim().is_empty() {
                                        if input.trim().starts_with('/') {
                                            let registry = commands::CommandRegistry::new()
                                                .with_custom_commands(app.config.custom_commands.clone());
                                            if let Some((cmd_name, args)) = registry.parse(&input) {
                                                let result =
                                                    registry.execute(cmd_name, &args, &mut app);
                                                match result {
                                                    commands::CommandResult::Ok => {}
                                                    commands::CommandResult::Message(msg) => {
                                                        app.status_message = Some(msg);
                                                    }
                                                    commands::CommandResult::Error(err) => {
                                                        app.status_message = Some(err);
                                                    }
                                                    commands::CommandResult::ModeChange(mode) => {
                                                        app.mode = mode;
                                                    }
                                                    commands::CommandResult::Quit => {
                                                        app.mode = app::AppMode::Quit;
                                                    }
                                                }
                                            }
                                        } else if input.trim().starts_with('!') {
                                            let command =
                                                input.trim().trim_start_matches('!').trim();
                                            if !command.is_empty() {
                                                app.execute_bang_command(command).await?;
                                            }
                                        } else {
                                            let processed = app.process_input_references(&input);
                                            app.send_message(processed).await?;
                                        }
                                        app.scroll_to_bottom();
                                    }
                                }
                                KeyCode::Tab if !app.is_loading && app.input.starts_with('/') => {
                                    let registry = commands::CommandRegistry::new()
                                        .with_custom_commands(app.config.custom_commands.clone());
                                    if app.tab_completions.is_empty() {
                                        app.tab_completions = registry.completions(&app.input);
                                        if !app.tab_completions.is_empty() {
                                            app.tab_index = Some(0);
                                            app.input = app.tab_completions[0].clone();
                                            app.cursor_pos = app.input.len();
                                        }
                                    } else if let Some(idx) = app.tab_index {
                                        let next_idx = (idx + 1) % app.tab_completions.len();
                                        app.tab_index = Some(next_idx);
                                        app.input = app.tab_completions[next_idx].clone();
                                        app.cursor_pos = app.input.len();
                                    }
                                }
                                KeyCode::Char(c) if !app.is_loading => {
                                    app.input_push(c);
                                    app.clear_tab_completions();
                                }
                                KeyCode::Backspace if !app.is_loading => {
                                    app.input_backspace();
                                    app.clear_tab_completions();
                                }
                                KeyCode::Left => {
                                    app.input_move_left();
                                }
                                KeyCode::Right => {
                                    app.input_move_right();
                                }
                                KeyCode::Home => {
                                    app.input_home();
                                }
                                KeyCode::End => {
                                    app.input_end();
                                }
                                KeyCode::Up => {
                                    app.scroll_up();
                                }
                                KeyCode::Down => {
                                    let h = terminal.size()?.height as usize;
                                    app.scroll_down(h.saturating_sub(6));
                                }
                                KeyCode::PageUp => {
                                    let h = terminal.size()?.height as usize;
                                    for _ in 0..h.saturating_sub(6) {
                                        app.scroll_up();
                                    }
                                }
                                KeyCode::PageDown => {
                                    let h = terminal.size()?.height as usize;
                                    for _ in 0..h.saturating_sub(6) {
                                        app.scroll_down(h.saturating_sub(6));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    app::AppMode::Quit => break,
                }
            }
        }

        if app.mode == app::AppMode::Quit {
            break;
        }
    }

    Ok(())
}

fn handle_permission_prompt(app: &mut App, key_code: crossterm::event::KeyCode) {
    use app::PermissionResponse;
    
    let response = match key_code {
        crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => Some(PermissionResponse::AllowOnce),
        crossterm::event::KeyCode::Char('a') | crossterm::event::KeyCode::Char('A') => Some(PermissionResponse::AllowSession),
        crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') | crossterm::event::KeyCode::Esc => Some(PermissionResponse::Deny),
        _ => None,
    };

    if let Some(resp) = response {
        if let Some(tx) = app.permission_response_tx.take() {
            let _ = tx.send(resp);
        }
        app.pending_permission = None;
        app.mode = app::AppMode::Chat;
    }
}

fn handle_question_prompt(app: &mut App, key_code: crossterm::event::KeyCode) {
    let pending = match &app.pending_question {
        Some(p) => p.clone(),
        None => {
            app.mode = app::AppMode::Chat;
            return;
        }
    };

    match key_code {
        crossterm::event::KeyCode::Up => {
            if app.selected_option > 0 {
                app.selected_option -= 1;
            }
        }
        crossterm::event::KeyCode::Down => {
            if !pending.options.is_empty() && app.selected_option < pending.options.len() - 1 {
                app.selected_option += 1;
            }
        }
        crossterm::event::KeyCode::Enter => {
            if let Some(tx) = app.question_response_tx.take() {
                if !pending.options.is_empty() && app.selected_option < pending.options.len() {
                    let _ = tx.send(pending.options[app.selected_option].clone());
                } else {
                    let _ = tx.send(String::new());
                }
            }
            app.pending_question = None;
            app.mode = app::AppMode::Chat;
        }
        crossterm::event::KeyCode::Esc => {
            if let Some(tx) = app.question_response_tx.take() {
                let _ = tx.send(String::new());
            }
            app.pending_question = None;
            app.mode = app::AppMode::Chat;
        }
        _ => {}
    }
}
