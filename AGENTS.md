# AGENTS.md — OpenRust

AI-powered terminal coding assistant written in Rust (fork of OpenCode).
Single binary crate (`openrust`), Rust 2021 edition, stable toolchain.

## Build & Run Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (LTO enabled, stripped)
cargo run                      # Run in debug mode
cargo check                    # Type-check without building
cargo clippy                   # Lint (currently 42 warnings, 7 auto-fixable)
cargo fmt                      # Format (no rustfmt.toml — uses defaults)
cargo fmt -- --check           # Check formatting without modifying
cargo test                     # Run all tests (none exist yet)
cargo test <test_name>         # Run a single test by name
cargo test -- --nocapture      # Run tests with stdout visible
```

There are no CI workflows, no Makefile, no custom scripts.
`dev-dependencies`: only `tokio-test`.

## Architecture

```
src/
  main.rs           Entry point, CLI (clap), TUI event loop (crossterm + ratatui)
  app.rs            Application state (App struct), message send/stream, input handling
  config.rs         TOML config (~/.config/openrust/config.toml), env var overrides
  session.rs        Session persistence (JSON files in data dir)
  ai/
    mod.rs          Provider trait (async_trait), factory fn create_provider()
    types.rs        Shared types: Message, Role, CompletionRequest/Response, AiError
    anthropic.rs    Anthropic Claude API — SSE streaming
    openai.rs       OpenAI API — SSE streaming
  tools/
    mod.rs          ToolSet aggregator, tools_description() for system prompt
    file.rs         File read/write/search/patch operations
    shell.rs        Shell command execution with timeout
    git.rs          Git operations via CLI subprocess
  ui/
    mod.rs          UI router (draw fn dispatches by AppMode)
    chat.rs         Main chat view — messages, status bar, input
    help.rs         Help overlay
    sessions.rs     Session list overlay
    theme.rs        Catppuccin-style dark/light color themes
  auth/
    mod.rs          Browser-based login/logout/set-default flow
    server.rs       Local HTTP callback server for API key capture
```

## Code Style

### Formatting
- Default `rustfmt` settings (no `rustfmt.toml`). Run `cargo fmt` before committing.
- No line length override — uses rustfmt default (100 chars).

### Imports
Group imports in this order, separated by blank lines:
1. `std` / standard library
2. External crates (alphabetical)
3. Internal (`crate::` and `super::`)

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::ai::types::{CompletionRequest, Message, Role};
use crate::config::Config;
```

- Use explicit imports — no glob imports (`use super::*`).
- Nested imports for multiple items from same crate: `use crossterm::{event::{...}, terminal::{...}};`

### Naming
- `snake_case`: functions, methods, variables, modules
- `PascalCase`: types, enums, traits, structs
- `SCREAMING_SNAKE_CASE`: constants (`const ANTHROPIC_API_URL: &str = ...`)
- Enum variants: `PascalCase` (e.g., `AppMode::SessionList`, `Role::Assistant`)

### Error Handling
- **Application errors**: `anyhow::Result<T>` with `.with_context(|| ...)` for rich context.
- **Domain errors**: `thiserror` derive macro for typed errors (see `AiError` in `ai/types.rs`).
- **No bare `unwrap()`** in production paths. Use `?`, `.ok()`, `.unwrap_or()`, `.unwrap_or_default()`, `.unwrap_or_else()`.
- **`anyhow::bail!(...)`** for early-return errors with message.

```rust
let content = std::fs::read_to_string(&path)
    .with_context(|| format!("Failed to read file: {}", path.display()))?;
```

### Types & Derive Macros
- Structs/enums use derives liberally: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Serde attributes for API types: `#[serde(rename_all = "lowercase")]`, `#[serde(rename = "type")]`, `#[serde(skip_serializing_if = "Option::is_none")]`
- Public fields on data structs (`pub` on struct fields), no getters unless logic needed.

### Async
- Runtime: `tokio` with `features = ["full"]`, entry point: `#[tokio::main]`
- Async traits: `#[async_trait]` from `async_trait` crate (see `Provider` trait).
- Channels: `tokio::sync::mpsc` for event passing between async tasks.
- Streaming: `futures::StreamExt` for byte stream iteration.

### Strings
- Prefer `.to_string()` over `String::from()` for converting `&str`.
- `format!()` for string interpolation.
- `String::from_utf8_lossy()` for byte-to-string conversion.
- Raw string literals `r#"..."#` for multi-line templates (system prompts, HTML).

### Module Organization
- Each module directory has `mod.rs` with `pub mod` declarations.
- Re-export key types from `mod.rs` when appropriate.
- Module-level constants for configuration values (API URLs, versions, timeouts).

### UI (ratatui)
- Each view is a standalone `draw_*` function taking `&mut Frame` and `&App`.
- Theme colors use `Color::Rgb(r, g, b)` — Catppuccin Mocha (dark) / Latte (light).
- Layout uses `Layout::default().direction(...).constraints([...])`.
- Widgets: `Paragraph`, `Block`, `List`, `Span`, `Line`.

### Comments
- `///` doc comments on public functions and types.
- `//` inline comments for non-obvious logic.
- Comments are sparse — code is expected to be self-documenting.

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` + `crossterm` | TUI framework |
| `tokio` | Async runtime |
| `reqwest` (rustls) | HTTP client for AI APIs |
| `serde` + `serde_json` + `toml` | Serialization |
| `clap` (derive) | CLI argument parsing |
| `anyhow` + `thiserror` | Error handling |
| `syntect` | Syntax highlighting |
| `tracing` | Structured logging (stderr, env filter) |
| `chrono` | Timestamps |
| `uuid` | Session IDs |
| `async-trait` | Async trait support |

## Known Issues
- 33 compiler warnings (dead code — tools/providers not yet wired to AI responses).
- Clippy suggests `strip_prefix` over manual slicing in `ui/chat.rs`.
- No tests exist. Test infrastructure (`tokio-test`) is in dev-dependencies but unused.
- `auth/server.rs` has an unused mutable variable (`chars` in `percent_decode`).
