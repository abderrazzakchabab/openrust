mod ai;
mod app;
mod auth;
mod config;
mod session;
mod tools;
mod ui;

use std::io;
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

    // Run TUI
    run_tui(config).await
}

fn list_sessions() {
    match session::Session::list_all() {
        Ok(sessions) if sessions.is_empty() => println!("No saved sessions."),
        Ok(sessions) => {
            println!("{:<4} {:<50} {:<20} {:>8}", "No.", "Title", "Updated", "Messages");
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
    println!("  Sessions dir:   {}", session::Session::sessions_dir().display());
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
            }
        }

        // Check for keyboard input (with short timeout to stay responsive)
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match app.mode {
                    AppMode::Help => match key.code {
                        KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => {
                            app.mode = AppMode::Chat;
                        }
                        _ => {}
                    },
                    AppMode::SessionList => match key.code {
                        KeyCode::Esc | KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.mode = AppMode::Chat;
                        }
                        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let provider = &app.config.provider.default.clone();
                            let model = app.get_model();
                            app.session = session::Session::new(provider, &model);
                            app.mode = AppMode::Chat;
                        }
                        _ => {}
                    },
                    AppMode::Chat => {
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                let _ = app.session.save();
                                app.mode = AppMode::Quit;
                            }
                            KeyCode::F(1) | KeyCode::Char('?') => {
                                app.mode = AppMode::Help;
                            }
                            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.mode = AppMode::SessionList;
                            }
                            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                let _ = app.session.save();
                                let provider = app.config.provider.default.clone();
                                let model = app.get_model();
                                app.session = session::Session::new(&provider, &model);
                                app.scroll_to_bottom();
                            }
                            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                let _ = app.session.save();
                                app.status_message = Some("Session saved".to_string());
                            }
                            KeyCode::Enter if !app.is_loading => {
                                let input = app.take_input();
                                if !input.trim().is_empty() {
                                    app.send_message(input).await?;
                                    app.scroll_to_bottom();
                                }
                            }
                            KeyCode::Char(c) if !app.is_loading => {
                                app.input_push(c);
                            }
                            KeyCode::Backspace if !app.is_loading => {
                                app.input_backspace();
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
                    AppMode::Quit => break,
                }
            }
        }

        if app.mode == AppMode::Quit {
            break;
        }
    }

    Ok(())
}
