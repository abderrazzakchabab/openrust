# OpenRust

An AI-powered terminal coding assistant, written in Rust. A Rust fork of [OpenCode](https://github.com/sst/opencode).

## Features

- **TUI Interface**: Beautiful terminal UI built with [ratatui](https://github.com/ratatui-org/ratatui)
- **Multiple AI Providers**: Supports Anthropic (Claude) and OpenAI (GPT)
- **Streaming Responses**: Real-time streaming of AI responses
- **Session Management**: Save and resume chat sessions
- **Developer Tools**: File reading, shell execution, and git integration
- **Syntax Highlighting**: Markdown rendering with code block support
- **Configurable**: TOML-based configuration

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/openrust
```

## Configuration

Set your API key via environment variable:

```bash
export ANTHROPIC_API_KEY=your-key-here
# or
export OPENAI_API_KEY=your-key-here
```

Config file is at `~/.config/openrust/config.toml` (auto-created on first run).

To switch providers:

```bash
openrust --provider openai
openrust --provider anthropic
```

To use a specific model:

```bash
openrust --provider anthropic --model claude-opus-4-6
openrust --provider openai --model gpt-4o
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Ctrl+Q` | Quit |
| `Ctrl+N` | New session |
| `Ctrl+S` | Save session |
| `Ctrl+L` | View session list |
| `F1` / `?` | Toggle help |
| `Up/Down` | Scroll messages |
| `PgUp/PgDn` | Scroll page |
| `Left/Right` | Move cursor |
| `Home/End` | Jump cursor |

## CLI Commands

```bash
# List sessions
openrust sessions

# Show config
openrust config

# Clear all sessions
openrust clear

# Open with specific working directory
openrust --dir /path/to/project
```

## Architecture

```
src/
├── main.rs          # Entry point, TUI event loop
├── app.rs           # Application state
├── config.rs        # Configuration management
├── session.rs       # Session persistence
├── ai/
│   ├── mod.rs       # Provider trait
│   ├── anthropic.rs # Anthropic/Claude integration
│   ├── openai.rs    # OpenAI integration
│   └── types.rs     # Shared types
├── tools/
│   ├── mod.rs       # Tool aggregator
│   ├── file.rs      # File operations
│   ├── shell.rs     # Shell execution
│   └── git.rs       # Git operations
└── ui/
    ├── mod.rs        # UI router
    ├── chat.rs       # Main chat view
    ├── help.rs       # Help overlay
    ├── sessions.rs   # Session list view
    └── theme.rs      # Color themes
```

## License

MIT
