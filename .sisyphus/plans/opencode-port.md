# OpenCode → OpenRust: Complete Rust Port

## TL;DR

> **Quick Summary**: Port ALL OpenCode (TypeScript AI coding assistant) functionality into OpenRust — a single Rust binary. Starting from a ~3,000 LOC skeleton with basic chat/streaming, build out the complete tool calling protocol, 15 built-in tools, 7 agents, MCP/LSP integration, permission system, and full TUI with slash commands, themes, and keybinds.
> 
> **Deliverables**:
> - Complete tool calling protocol (Anthropic + OpenAI)
> - Agent orchestration loop (call → tool_use → execute → tool_result → repeat)
> - 15 built-in tools (bash, edit, write, read, grep, glob, list, lsp, patch, skill, todowrite, todoread, webfetch, websearch, question)
> - 7 agents (build, plan, general, explore, compaction, title, summary)
> - MCP client (stdio + HTTP/SSE + OAuth)
> - LSP client (diagnostics for 30+ languages)
> - Permission system (allow/ask/deny, glob patterns)
> - JSON/JSONC config with variable substitution
> - SQLite session storage with undo/redo
> - Full TUI: 17 slash commands, 60+ keybinds, themes, file refs (@), bash prefix (!)
> - CLI mode, conversation sharing, custom commands, formatters, plugins
> - Provider expansion: Google, Bedrock, Azure, Ollama, OpenRouter, 75+ configs
> 
> **Estimated Effort**: XL (50 tasks, ~25,000-35,000 LOC final binary)
> **Parallel Execution**: YES — 10 waves, many tasks parallelizable within waves
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 5 → Task 6 → Task 10

---

## Context

### Original Request
"Make a rust copy of opencode, implement all opencode functionality but in rust" — ULTRAWORK mode, no scope reduction.

### Interview Summary
**Key Discussions (from prior session)**:
- Full OpenCode feature inventory was gathered from docs and source
- Current OpenRust has only basic streaming with no tool calling
- All 19 source files analyzed (~3,000 LOC total)
- Gap analysis identified tool calling protocol as #1 blocker

**Research Findings**:
- OpenCode is TypeScript (not Go), uses Vercel AI SDK + Drizzle ORM + SolidJS TUI
- OpenCode has 15 built-in tools, 7 agents, 75+ provider configs
- Current OpenRust has Message { role, content: String } — needs ContentBlock expansion
- MCP Rust crate `rmcp` exists but needs evaluation
- OpenCode uses JSON/JSONC config, OpenRust uses TOML (needs migration)

### Metis Review
**Key Corrections Applied**:
- Reordered waves: vertical slice first (Anthropic + bash + agent loop + TUI rendering)
- Deferred SQLite to after tools work (JSON sessions adequate for early waves)
- ContentBlock and StreamEvent are load-bearing architectural types — prioritized in Wave 0
- Agent loop elevated to co-equal blocker with tool calling parsing
- Dropped portable-pty (tokio::process::Command sufficient), axum (client not server), nucleo (premature)
- LOC estimate raised to 25,000-35,000

---

## Work Objectives

### Core Objective
Port 100% of OpenCode's functionality to a single Rust binary (`openrust`) using the existing tech stack (ratatui, tokio, reqwest) plus carefully selected additional crates.

### Concrete Deliverables
- Modified `src/ai/types.rs` with ContentBlock, StreamEvent, tool types
- New/modified provider files with tool calling support
- `src/tools/` expanded to 15 tool implementations
- `src/agents/` new module with 7 agent implementations
- `src/mcp/` new module for MCP client
- `src/lsp/` new module for LSP client
- `src/permissions/` new module for permission system
- Modified `src/config.rs` for JSON/JSONC
- Modified `src/session.rs` for SQLite
- Modified `src/ui/` for full TUI features
- Modified `src/main.rs` for CLI mode + slash commands

### Definition of Done
- [ ] `cargo build --release` succeeds with zero errors
- [ ] `cargo clippy` passes with no errors (warnings acceptable for dead code during phased build)
- [ ] `cargo test` passes all tests
- [ ] All 15 built-in tools executable via AI tool calling
- [ ] Agent loop correctly handles multi-step tool chains
- [ ] MCP servers connectable (at least stdio)
- [ ] LSP diagnostics displayable
- [ ] JSON/JSONC config loads and merges correctly
- [ ] SQLite sessions persist and load correctly

### Must Have
- Tool calling protocol for Anthropic Messages API (tool_use/tool_result content blocks)
- Tool calling protocol for OpenAI Chat Completions API (tool_calls/tool role)
- Agent orchestration loop with max iteration guard
- All 15 built-in tools
- Permission system with allow/ask/deny
- JSON/JSONC config with {env:VAR} substitution
- Slash commands, keybinds, themes
- CLI mode (non-interactive)

### Must NOT Have (Guardrails)
- **No Vercel AI SDK equivalent** — implement tool calling natively per provider API
- **No ORM layer** — use rusqlite directly with raw SQL
- **No SolidJS/React paradigm** — keep ratatui's immediate-mode rendering
- **No web server mode** (OpenCode has this but it's a stretch goal; MCP client is priority)
- **No portable-pty** — tokio::process::Command with proper pipe handling is sufficient
- **No axum/web framework** — we're an MCP client, not server (auth callback uses raw TcpListener)
- **No premature optimization** — get features working first, optimize later
- **No glob imports** — maintain explicit imports per AGENTS.md conventions

---

## Verification Strategy

> **UNIVERSAL RULE: ZERO HUMAN INTERVENTION**
> ALL tasks verified by agent using tools. No human testing required.

### Test Decision
- **Infrastructure exists**: NO (tokio-test in dev-deps, 0 tests)
- **Automated tests**: YES (tests-after) — core modules get unit tests
- **Framework**: `cargo test` with tokio-test for async tests
- **Test infrastructure setup**: Included in Task 50

### Agent-Executed QA Scenarios (MANDATORY — ALL tasks)

**Verification Tool by Deliverable Type:**

| Type | Tool | How Agent Verifies |
|------|------|-------------------|
| **Type system changes** | Bash (cargo check/clippy) | Compile, zero errors, verify type usage |
| **Provider modifications** | Bash (cargo test + manual API call) | Unit test + real API streaming test |
| **Tool implementations** | Bash (cargo test) + interactive_bash | Unit tests + invoke tool directly |
| **TUI changes** | interactive_bash (tmux) | Launch app, interact, verify rendering |
| **Agent system** | Bash (cargo test) + interactive_bash | Unit tests + multi-step conversation |
| **Config/Session** | Bash (cargo test) | Load/save/merge tests |
| **Integration** | interactive_bash (tmux) + Bash | Full conversation with tool calls |

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 0 — Architecture Foundation (sequential, everything depends on this):
├── Task 1:  ContentBlock + StreamEvent type system
├── Task 2:  Tool trait + ToolRegistry
├── Task 3:  Anthropic provider tool calling
├── Task 4:  OpenAI provider tool calling
└── Task 5:  Agent orchestration loop

Wave 1 — Vertical Slice (after Wave 0):
├── Task 6:  Bash tool ──────────────────┐
├── Task 7:  Read tool ──────────────────┤ (parallel)
├── Task 8:  Write + Edit tools ─────────┤
└── Task 10: TUI tool call rendering ────┘ (after 6-8)

Wave 2 — Complete Tool Suite (after Wave 1):
├── Task 9:  Grep tool ─────────────────┐
├── Task 11: Glob tool ─────────────────┤
├── Task 12: List tool ─────────────────┤ (parallel)
├── Task 13: Patch tool ────────────────┤
├── Task 14: TodoWrite + TodoRead ──────┤
├── Task 15: Question tool ─────────────┤
├── Task 16: WebFetch + WebSearch ──────┤
└── Task 17: Skill tool ────────────────┘

Wave 3 — Config & Sessions (after Wave 1, parallel with Wave 2):
├── Task 18: JSON/JSONC config system ──┐
├── Task 19: Config var substitution ───┘ (sequential)
└── Task 20: SQLite session storage

Wave 4 — Agent System (after Wave 2):
├── Task 21: Agent trait + dispatch ────┐
├── Task 22: Build + Plan agents ───────┤ (sequential)
├── Task 23: General + Explore subagents┤
└── Task 24: Hidden agents + compaction ┘

Wave 5 — TUI Enhancement (after Wave 3, parallel with Wave 4):
├── Task 25: Slash command system ──────┐
├── Task 26: Slash commands part 2 ─────┤ (sequential)
├── Task 27: File refs + bash prefix ───┤
├── Task 28: Multi-line input + markdown┤
├── Task 29: Theme system ─────────────┤ (parallel with 28)
└── Task 30: Keybind system ────────────┘ (parallel with 29)

Wave 6 — Permissions (after Wave 4):
├── Task 31: Permission system ─────────┐ (sequential)
└── Task 32: Agent overrides + doom loop┘

Wave 7 — MCP (after Wave 6, parallel with Wave 8):
├── Task 33: MCP local client (stdio) ─┐
├── Task 34: MCP remote client (HTTP) ──┤ (sequential)
└── Task 35: MCP OAuth + registration ──┘

Wave 8 — LSP (after Wave 2, parallel with Wave 7):
├── Task 36: LSP client + diagnostics ─┐ (sequential)
└── Task 37: LSP server configs ────────┘

Wave 9 — Provider Expansion (after Wave 0, parallel with Wave 3+):
├── Task 38: Google Gemini provider ────┐
├── Task 39: Bedrock + Azure providers ─┤ (parallel)
├── Task 40: OpenRouter + Ollama ───────┤
└── Task 41: Provider config system ────┘

Wave 10 — Polish (after Wave 5-8):
├── Task 42: CLI mode ─────────────────┐
├── Task 43: Undo/redo (git-based) ────┤
├── Task 44: Sharing + export ──────────┤ (parallel)
├── Task 45: Custom commands ───────────┤
├── Task 46: Code formatters ───────────┤
├── Task 47: Plugins + file watcher ────┤
└── Task 48: Custom tools + server mode ┘

Wave 11 — Testing (can start any time after Wave 1):
├── Task 49: Test infrastructure ───────┐ (sequential)
└── Task 50: Integration tests ─────────┘

Critical Path: 1 → 2 → 3 → 5 → 6 → 10 → 21 → 22 → 31 → 33
Parallel Speedup: ~60% faster than sequential
```

### Dependency Matrix

| Task | Depends On | Blocks | Parallel With |
|------|------------|--------|---------------|
| 1 | None | 2, 3, 4 | None (first) |
| 2 | 1 | 3, 4, 5, 6-17 | None |
| 3 | 1, 2 | 5 | 4 |
| 4 | 1, 2 | 5 | 3 |
| 5 | 3, 4 | 6-10 | None |
| 6 | 5 | 10 | 7, 8 |
| 7 | 5 | 10 | 6, 8 |
| 8 | 5 | 10 | 6, 7 |
| 9-17 | 5 | 21 | Each other |
| 10 | 6, 7, 8 | 25 | 9-17 |
| 18-19 | 5 | 25-30 | 9-17 |
| 20 | 18 | 43 | 9-17 |
| 21-24 | 9-17 | 31 | 25-30 |
| 25-30 | 10, 18 | 42 | 21-24 |
| 31-32 | 21-24 | 33-35 | 36-37 |
| 33-35 | 31 | 47 | 36-37 |
| 36-37 | 5 | 47 | 33-35 |
| 38-41 | 3, 4 | None | Most waves |
| 42-48 | 25-30, 31-37 | None | Each other |
| 49-50 | 5 | None | Any wave |

---

## TODOs

### Wave 0 — Architecture Foundation

- [x] 1. ContentBlock + StreamEvent Type System

  **What to do**:
  - Replace `Message { role, content: String }` with `Message { role, content: Vec<ContentBlock> }`
  - Define `ContentBlock` enum: `Text { text }`, `ToolUse { id, name, input: Value }`, `ToolResult { tool_use_id, content, is_error }`, `Thinking { thinking }`, `Image { source }`
  - Define `StreamEvent` enum: `ContentStart { index, content_type }`, `ContentDelta { index, delta }`, `ContentStop { index }`, `MessageStart { message }`, `MessageDelta { stop_reason, usage }`, `MessageStop`, `Error { message }`
  - Update `CompletionRequest` to include `tools: Option<Vec<ToolDefinition>>`
  - Define `ToolDefinition { name, description, input_schema: Value }`
  - Update `CompletionResponse` to use `Vec<ContentBlock>` for content
  - Add `InputSchema` helper type for JSON Schema parameter definitions
  - Update all code that constructs/reads `Message` to use new format
  - Ensure `Display` impls extract text content for backward compat

  **Must NOT do**:
  - Don't change the Provider trait yet (Task 3-4 does that)
  - Don't implement actual tool execution (Task 5 does that)
  - Don't add tools themselves (Wave 1-2)

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: Core type system design requires deep understanding of how Anthropic/OpenAI APIs structure tool calls; wrong decisions here cascade to everything
  - **Skills**: []
    - No specific skills needed — this is pure Rust type design

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 0 — Sequential start
  - **Blocks**: Tasks 2, 3, 4, 5 (everything depends on these types)
  - **Blocked By**: None (first task)

  **References**:

  **Pattern References**:
  - `src/ai/types.rs` — Current Message, Role, CompletionRequest/Response definitions. REPLACE content: String with content: Vec<ContentBlock>
  - `src/ai/anthropic.rs:AnthropicRequest` — Current request struct. Shows how messages are serialized for Anthropic API
  - `src/ai/openai.rs:OpenAIRequest` — Current request struct. Shows how messages are serialized for OpenAI API
  - `src/app.rs:send_message()` + `apply_stream_chunk()` — All places that construct/consume Messages. Must be updated to use ContentBlock

  **API References**:
  - Anthropic Messages API: `POST /v1/messages` — `content` field is `Array<ContentBlock>` where each block has `type: "text"|"tool_use"|"tool_result"`. Tool use blocks: `{ type: "tool_use", id: "toolu_xxx", name: "tool_name", input: {} }`. Tool result: `{ type: "tool_result", tool_use_id: "toolu_xxx", content: "result" }`
  - OpenAI Chat Completions API: `tool_calls` array in assistant messages: `{ id: "call_xxx", type: "function", function: { name, arguments } }`. Tool results sent as messages with `role: "tool"`, `tool_call_id`, `content`

  **WHY Each Reference Matters**:
  - `types.rs` is the foundation — every other file imports from here
  - Provider request structs show the serialization shape we need to match
  - `app.rs` shows all consumers that need migration to new types

  **Acceptance Criteria**:
  - [ ] `cargo check` passes — zero compilation errors after type changes
  - [ ] `cargo clippy` passes (no new warnings from these changes)
  - [ ] ContentBlock enum has at minimum: Text, ToolUse, ToolResult variants
  - [ ] Message.content is Vec<ContentBlock>, not String
  - [ ] CompletionRequest has tools: Option<Vec<ToolDefinition>> field
  - [ ] StreamEvent enum covers all streaming event types
  - [ ] All existing code in app.rs, ui/chat.rs that reads message.content is updated

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Codebase compiles after type migration
    Tool: Bash
    Preconditions: None
    Steps:
      1. cargo check 2>&1
      2. Assert: exit code 0
      3. Assert: no "error[E" in output
      4. cargo clippy 2>&1
      5. Assert: no "error" level diagnostics
    Expected Result: Clean compilation
    Evidence: Build output captured

  Scenario: ContentBlock serialization roundtrips correctly
    Tool: Bash (cargo test)
    Preconditions: Test written for ContentBlock serde
    Steps:
      1. Create test: serialize Text block → JSON → deserialize → compare
      2. Create test: serialize ToolUse block → JSON → deserialize → compare
      3. Create test: serialize ToolResult block → JSON → deserialize → compare
      4. cargo test content_block 2>&1
      5. Assert: all tests pass
    Expected Result: All block types serialize/deserialize correctly
    Evidence: Test output captured
  ```

  **Commit**: YES
  - Message: `refactor(types): replace String content with ContentBlock type system for tool calling support`
  - Files: `src/ai/types.rs`, `src/app.rs`, `src/ui/chat.rs`, `src/session.rs`
  - Pre-commit: `cargo check`

---

- [ ] 2. Tool Trait + ToolRegistry

  **What to do**:
  - Create `src/tools/traits.rs` with `Tool` trait: `fn name(&self)`, `fn description(&self)`, `fn parameters(&self) -> Value` (JSON Schema), `async fn execute(&self, input: Value) -> Result<ToolOutput>`
  - Define `ToolOutput { content: String, is_error: bool }`
  - Create `ToolRegistry` struct: `HashMap<String, Box<dyn Tool>>` with `register()`, `get()`, `list()`, `to_definitions() -> Vec<ToolDefinition>`
  - `to_definitions()` converts registered tools to API-ready ToolDefinition format
  - Update `src/tools/mod.rs` to expose new trait and registry
  - Remove old `ToolSet` struct and `tools_description()` function (replaced by registry)
  - Do NOT migrate existing file/shell/git tools yet — just define the trait

  **Must NOT do**:
  - Don't implement any new tools (Wave 1-2)
  - Don't wire registry into providers yet (Task 3-4)
  - Don't modify provider code

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Well-defined trait design, small scope, clear pattern
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 0 — after Task 1
  - **Blocks**: Tasks 3, 4, 5, 6-17
  - **Blocked By**: Task 1 (needs ToolDefinition type)

  **References**:

  **Pattern References**:
  - `src/tools/mod.rs:ToolSet` — Current tool aggregator (REPLACE with ToolRegistry)
  - `src/tools/mod.rs:tools_description()` — Current text-based tool description (REPLACE with JSON Schema)
  - `src/ai/types.rs:ToolDefinition` — The type from Task 1 that registry must produce

  **API References**:
  - Anthropic tool format: `{ name: "bash", description: "...", input_schema: { type: "object", properties: {...}, required: [...] } }`
  - OpenAI tool format: `{ type: "function", function: { name: "bash", description: "...", parameters: { type: "object", properties: {...}, required: [...] } } }`

  **WHY Each Reference Matters**:
  - `ToolSet` shows current pattern to replace — registry is the new central hub
  - `ToolDefinition` from Task 1 defines the output format — registry converts tools to this
  - API formats show what JSON Schema shape the parameters() method must produce

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] Tool trait defined with name/description/parameters/execute methods
  - [ ] ToolRegistry can register, get, list, and export definitions
  - [ ] Old ToolSet removed or deprecated
  - [ ] `to_definitions()` produces valid ToolDefinition vec

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: ToolRegistry CRUD operations work
    Tool: Bash (cargo test)
    Preconditions: Test written with mock tool
    Steps:
      1. Create MockTool implementing Tool trait
      2. Register in ToolRegistry
      3. Assert: registry.get("mock") returns Some
      4. Assert: registry.list() contains "mock"
      5. Assert: registry.to_definitions() produces valid JSON Schema
      6. cargo test tool_registry 2>&1
      7. Assert: all tests pass
    Expected Result: Registry correctly manages tools
    Evidence: Test output captured
  ```

  **Commit**: YES
  - Message: `feat(tools): add Tool trait and ToolRegistry for dynamic tool management`
  - Files: `src/tools/traits.rs`, `src/tools/mod.rs`
  - Pre-commit: `cargo check`

---

- [ ] 3. Anthropic Provider: Tool Calling Support

  **What to do**:
  - Modify `AnthropicProvider::complete()` to accept tools in CompletionRequest and include them in API request body
  - Modify `AnthropicRequest` to include `tools: Option<Vec<AnthropicToolDef>>` field
  - Parse response content blocks: handle `type: "tool_use"` blocks alongside `type: "text"` blocks
  - Modify `AnthropicProvider::complete_stream()` to emit StreamEvent types instead of raw strings
  - Handle streaming tool use: `content_block_start` with `type: "tool_use"`, accumulate `input_json_delta`, `content_block_stop`
  - Parse `message_delta` for stop_reason: "end_turn" vs "tool_use"
  - Return Vec<ContentBlock> in CompletionResponse instead of single string
  - Update event_tx channel type from `Result<String, AiError>` to `Result<StreamEvent, AiError>`

  **Must NOT do**:
  - Don't implement tool execution (Task 5)
  - Don't modify OpenAI provider (Task 4)
  - Don't add caching or prompt caching headers yet

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: SSE parsing for tool calls is intricate — partial JSON accumulation, multiple content block types, stop_reason handling
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 0 — parallel with Task 4
  - **Blocks**: Task 5
  - **Blocked By**: Tasks 1, 2

  **References**:

  **Pattern References**:
  - `src/ai/anthropic.rs` — ENTIRE FILE. Current SSE streaming, request/response structs, event parsing. This is the base to modify.
  - `src/ai/types.rs` — Updated types from Task 1 (ContentBlock, StreamEvent, ToolDefinition)
  - `src/app.rs:send_message()` — The caller that spawns provider tasks. Channel type changes here.

  **API References**:
  - Anthropic Messages API tool calling: Request includes `tools: [{ name, description, input_schema }]`. Response content array includes `{ type: "tool_use", id: "toolu_01A...", name: "bash", input: { command: "ls" } }`. Stop reason is `"tool_use"` when tools need execution.
  - Anthropic SSE streaming with tools: Events include `content_block_start` (index + type), `content_block_delta` (type: "input_json_delta" with partial_json), `content_block_stop`. Tool use input arrives as incremental JSON fragments that must be accumulated.

  **WHY Each Reference Matters**:
  - `anthropic.rs` is the exact file being modified — SSE parser needs extension for tool events
  - Types from Task 1 define the output format
  - `app.rs` shows channel consumer that must handle StreamEvent instead of String

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] AnthropicRequest serializes tools field when present
  - [ ] Non-streaming: Response correctly parses mixed text + tool_use content blocks
  - [ ] Streaming: StreamEvent emitted for content_block_start/delta/stop
  - [ ] Streaming: Tool use input JSON accumulated from delta fragments
  - [ ] Stop reason correctly identifies "tool_use" vs "end_turn"
  - [ ] Channel type updated to StreamEvent

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Anthropic request includes tool definitions
    Tool: Bash (cargo test)
    Preconditions: Unit test with mock ToolDefinition
    Steps:
      1. Create CompletionRequest with tools = Some(vec![tool_def])
      2. Serialize to AnthropicRequest
      3. Assert: JSON body contains "tools" array
      4. Assert: tool has "name", "description", "input_schema" fields
      5. cargo test anthropic_tools 2>&1
    Expected Result: Tools correctly serialized in request
    Evidence: Test output captured

  Scenario: Anthropic streaming parses tool_use events
    Tool: Bash (cargo test)
    Preconditions: Mock SSE response with tool_use content blocks
    Steps:
      1. Create mock SSE stream with: content_block_start (tool_use), input_json_delta fragments, content_block_stop
      2. Feed through stream parser
      3. Assert: StreamEvent::ContentStart emitted with tool_use type
      4. Assert: Accumulated input JSON is valid
      5. Assert: Final ContentBlock::ToolUse has correct id, name, input
    Expected Result: Tool use blocks correctly parsed from stream
    Evidence: Test output captured
  ```

  **Commit**: YES
  - Message: `feat(anthropic): add tool calling support with streaming tool_use parsing`
  - Files: `src/ai/anthropic.rs`, `src/ai/types.rs` (if minor tweaks needed)
  - Pre-commit: `cargo check`

---

- [ ] 4. OpenAI Provider: Tool Calling Support

  **What to do**:
  - Modify `OpenAIProvider::complete()` to include `tools` in API request (OpenAI format: `{ type: "function", function: { name, description, parameters } }`)
  - Parse `tool_calls` array in assistant response choices: `{ id, type: "function", function: { name, arguments } }`
  - Convert OpenAI tool_calls to ContentBlock::ToolUse for unified internal format
  - Modify `OpenAIProvider::complete_stream()` to emit StreamEvent types
  - Handle streaming tool calls: `delta.tool_calls` with incremental function argument fragments
  - Handle `finish_reason: "tool_calls"` to signal tool execution needed
  - Convert tool results to OpenAI format: messages with `role: "tool"`, `tool_call_id`, `content`
  - Ensure bidirectional conversion: internal ContentBlock ↔ OpenAI message format

  **Must NOT do**:
  - Don't implement tool execution (Task 5)
  - Don't modify Anthropic provider (Task 3)

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: OpenAI's tool calling format differs significantly from Anthropic's — need careful mapping between formats
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 0 — parallel with Task 3
  - **Blocks**: Task 5
  - **Blocked By**: Tasks 1, 2

  **References**:

  **Pattern References**:
  - `src/ai/openai.rs` — ENTIRE FILE. Current SSE streaming, request/response structs. Base to modify.
  - `src/ai/types.rs` — Updated types from Task 1
  - `src/ai/anthropic.rs` (Task 3 output) — Reference for how Anthropic provider was updated — follow similar patterns

  **API References**:
  - OpenAI Chat Completions tool calling: Request includes `tools: [{ type: "function", function: { name, description, parameters } }]`. Response: `choices[0].message.tool_calls: [{ id, type: "function", function: { name, arguments: "JSON string" } }]`. Finish reason: `"tool_calls"`. Tool results: `{ role: "tool", tool_call_id: "call_xxx", content: "result" }`
  - OpenAI SSE streaming with tools: `delta.tool_calls` array with incremental `function.arguments` strings

  **WHY Each Reference Matters**:
  - `openai.rs` is the file being modified — same scope as Task 3 but for OpenAI format
  - Types ensure consistent internal representation regardless of provider

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] OpenAIRequest serializes tools in OpenAI format (type: "function")
  - [ ] Non-streaming: tool_calls parsed and converted to ContentBlock::ToolUse
  - [ ] Streaming: StreamEvent emitted for delta.tool_calls
  - [ ] Streaming: Function argument fragments accumulated correctly
  - [ ] Tool results convertible to role: "tool" messages
  - [ ] finish_reason "tool_calls" correctly detected

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: OpenAI tool calling roundtrip
    Tool: Bash (cargo test)
    Steps:
      1. Create CompletionRequest with tools
      2. Serialize to OpenAI format, assert "type": "function" wrapper
      3. Create mock response with tool_calls array
      4. Parse to ContentBlock::ToolUse, assert id/name/input correct
      5. Convert ToolResult back to OpenAI tool message format
      6. Assert: role is "tool", tool_call_id matches, content present
      7. cargo test openai_tools 2>&1
    Expected Result: Bidirectional conversion works
    Evidence: Test output captured
  ```

  **Commit**: YES
  - Message: `feat(openai): add tool calling support with streaming tool_calls parsing`
  - Files: `src/ai/openai.rs`
  - Pre-commit: `cargo check`

---

- [ ] 5. Agent Orchestration Loop

  **What to do**:
  - Create `src/agent_loop.rs` (or `src/loop.rs`) with the core orchestration engine
  - Implement loop: send request → receive response → if stop_reason == "tool_use" → extract ToolUse blocks → execute each tool via ToolRegistry → create ToolResult messages → append to conversation → send again → repeat until "end_turn" or max iterations
  - Add `MAX_ITERATIONS` constant (default: 50, configurable) to prevent infinite loops
  - Handle multiple tool calls in single response (parallel tool execution with tokio::join!)
  - Handle streaming: accumulate content blocks during stream, then process tool calls after stream completes
  - Integrate with App: modify `send_message()` to use agent loop instead of direct provider call
  - Emit AppEvents for each phase: StreamChunk, ToolCallStart { name, input }, ToolCallComplete { name, output }, StreamComplete
  - Add new AppEvent variants for tool call lifecycle

  **Must NOT do**:
  - Don't implement actual tools yet (Wave 1)
  - Don't implement agent-specific configuration (Wave 4)
  - Don't implement permission checks (Wave 6)

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: This is the most architecturally critical task — the core loop that drives all AI-tool interaction. Must handle streaming, parallel tool execution, error recovery, and event emission correctly
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 0 — after Tasks 3, 4
  - **Blocks**: Tasks 6-17 (all tools need the loop to work)
  - **Blocked By**: Tasks 3, 4 (needs provider tool calling support)

  **References**:

  **Pattern References**:
  - `src/app.rs:send_message()` — Current message sending flow. This gets replaced/wrapped by the agent loop.
  - `src/app.rs:AppEvent` — Current events. Extend with ToolCallStart, ToolCallComplete.
  - `src/tools/mod.rs:ToolRegistry` (from Task 2) — How to look up and execute tools
  - `src/ai/types.rs:ContentBlock, StreamEvent` (from Task 1) — Types flowing through the loop
  - `src/ai/anthropic.rs` + `src/ai/openai.rs` (from Tasks 3-4) — Providers that feed the loop

  **API References**:
  - Anthropic multi-turn tool use: User msg → Assistant (tool_use) → User (tool_result) → Assistant (text/more tool_use) → ...
  - OpenAI multi-turn: User msg → Assistant (tool_calls) → Tool msgs → Assistant (text/more tool_calls) → ...
  - Both APIs: tool results go in the next request as part of the conversation history

  **WHY Each Reference Matters**:
  - `send_message()` is the integration point — loop wraps or replaces this
  - AppEvent extensions needed so TUI can render tool call progress
  - ToolRegistry is how the loop finds and executes tools
  - Provider APIs define the conversation format for tool calling turns

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] Agent loop correctly chains: request → tool_use → execute → tool_result → request
  - [ ] Max iteration guard prevents infinite loops
  - [ ] Multiple tool calls in single response handled (parallel execution)
  - [ ] Streaming chunks forwarded to TUI during each iteration
  - [ ] AppEvent::ToolCallStart and ToolCallComplete emitted
  - [ ] Loop terminates on "end_turn" / "stop" stop reason
  - [ ] Error in tool execution produces ToolResult with is_error: true, doesn't crash loop

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Agent loop handles tool_use → tool_result → end_turn
    Tool: Bash (cargo test)
    Preconditions: Mock provider that returns tool_use then end_turn, mock tool
    Steps:
      1. Create mock provider: first call returns ContentBlock::ToolUse, second returns ContentBlock::Text with stop_reason "end_turn"
      2. Create mock tool that returns "tool output"
      3. Run agent loop
      4. Assert: tool was called with correct input
      5. Assert: second request includes ToolResult message
      6. Assert: loop terminated after 2 iterations
      7. Assert: final response contains text content
    Expected Result: Full tool calling cycle works
    Evidence: Test output captured

  Scenario: Max iteration guard triggers
    Tool: Bash (cargo test)
    Steps:
      1. Create mock provider that always returns tool_use (never end_turn)
      2. Set MAX_ITERATIONS = 3
      3. Run agent loop
      4. Assert: loop stops after 3 iterations
      5. Assert: error or warning emitted about max iterations
    Expected Result: Loop doesn't run forever
    Evidence: Test output captured
  ```

  **Commit**: YES
  - Message: `feat(core): implement agent orchestration loop for tool calling`
  - Files: `src/agent_loop.rs`, `src/app.rs`, `src/main.rs`
  - Pre-commit: `cargo check`

---

### Wave 1 — Vertical Slice: First Tools + TUI

- [ ] 6. Bash Tool

  **What to do**:
  - Create `src/tools/bash.rs` implementing Tool trait
  - Parameters: `command: String` (required), `timeout: Option<u64>` (seconds, default 120)
  - Execute via `tokio::process::Command` with `sh -c` (or appropriate shell)
  - Capture stdout + stderr, enforce timeout via `tokio::time::timeout`
  - Return combined output (stdout + stderr) truncated to reasonable limit (100KB)
  - Set working directory from App context
  - Register in ToolRegistry
  - Refactor existing `ShellTool` to implement new `Tool` trait or replace entirely

  **Must NOT do**:
  - Don't use portable-pty — tokio::process::Command is sufficient
  - Don't implement permission checks yet (Wave 6)
  - Don't implement PTY emulation for interactive commands

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Straightforward tool implementation following established patterns
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 — parallel with Tasks 7, 8
  - **Blocks**: Task 10 (TUI needs a working tool to render)
  - **Blocked By**: Task 5 (needs agent loop)

  **References**:

  **Pattern References**:
  - `src/tools/shell.rs:ShellTool` — Existing shell tool. Refactor to implement Tool trait or replace.
  - `src/tools/traits.rs:Tool` (from Task 2) — The trait to implement

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] Bash tool registered in ToolRegistry with correct JSON Schema parameters
  - [ ] `execute({ "command": "echo hello" })` returns "hello\n"
  - [ ] Timeout enforced (command killed after timeout)
  - [ ] Working directory correctly set
  - [ ] Stderr captured alongside stdout
  - [ ] Output truncated if > 100KB

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Bash tool executes commands
    Tool: Bash (cargo test)
    Steps:
      1. Create bash tool with working_dir = /tmp
      2. Execute: { "command": "echo hello && pwd" }
      3. Assert: output contains "hello"
      4. Assert: output contains "/tmp"
      5. Execute: { "command": "sleep 5", "timeout": 1 }
      6. Assert: returns error/timeout message
    Expected Result: Commands execute correctly with timeout
    Evidence: Test output captured
  ```

  **Commit**: YES (groups with 7, 8)
  - Message: `feat(tools): implement bash tool with timeout and output capture`
  - Files: `src/tools/bash.rs`, `src/tools/mod.rs`
  - Pre-commit: `cargo check`

---

- [ ] 7. Read Tool

  **What to do**:
  - Create `src/tools/read.rs` implementing Tool trait
  - Parameters: `file_path: String` (required), `offset: Option<u32>` (1-indexed line), `limit: Option<u32>` (max lines, default 2000)
  - Read file, prefix each line with line number: `{N}: {content}`
  - Support reading directories (list entries with trailing `/` for dirs)
  - Truncate lines > 2000 chars
  - Handle binary file detection (return error message)
  - Refactor existing `FileTool::read_file()` into this tool

  **Must NOT do**:
  - Don't implement image/PDF reading (stretch goal)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 — parallel with Tasks 6, 8
  - **Blocks**: Task 10
  - **Blocked By**: Task 5

  **References**:
  - `src/tools/file.rs:FileTool::read_file()` — Existing read implementation. Migrate to Tool trait.
  - `src/tools/file.rs:FileTool::list_directory()` — Directory listing. Read tool handles dirs too.

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] Reading file returns numbered lines: "1: first line\n2: second line\n..."
  - [ ] Offset and limit work correctly (offset=5, limit=10 → lines 5-14)
  - [ ] Directories return entry list with trailing /
  - [ ] Lines > 2000 chars truncated
  - [ ] Binary files return descriptive error

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Read tool reads files with line numbers
    Tool: Bash (cargo test)
    Steps:
      1. Create temp file with 10 lines
      2. Read with default params → assert all 10 lines numbered
      3. Read with offset=3, limit=2 → assert lines 3-4 only
      4. Read directory → assert entries listed
    Expected Result: File reading with proper line numbering
    Evidence: Test output captured
  ```

  **Commit**: YES (groups with 6, 8)
  - Message: `feat(tools): implement read tool with line numbers and offset/limit`
  - Files: `src/tools/read.rs`, `src/tools/mod.rs`
  - Pre-commit: `cargo check`

---

- [ ] 8. Write + Edit Tools

  **What to do**:
  - Create `src/tools/write.rs` implementing Tool trait
    - Parameters: `file_path: String`, `content: String`
    - Create parent directories, write file, return confirmation
  - Create `src/tools/edit.rs` implementing Tool trait
    - Parameters: `file_path: String`, `old_string: String`, `new_string: String`, `replace_all: Option<bool>`
    - Read file, find old_string, replace (first occurrence or all), write back
    - Error if old_string not found
    - Error if multiple matches and replace_all not set
  - Register both in ToolRegistry
  - Refactor existing `FileTool::write_file()` and `FileTool::apply_patch()`

  **Must NOT do**:
  - Don't implement file permission checks yet

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 — parallel with Tasks 6, 7
  - **Blocks**: Task 10
  - **Blocked By**: Task 5

  **References**:
  - `src/tools/file.rs:FileTool::write_file()` — Existing write. Migrate pattern.
  - `src/tools/file.rs:FileTool::apply_patch()` — Existing patch (find+replace). Edit tool replaces this.

  **Acceptance Criteria**:
  - [ ] Write tool creates files, creates parent dirs
  - [ ] Edit tool replaces first occurrence of old_string
  - [ ] Edit tool with replace_all replaces all occurrences
  - [ ] Edit tool errors on "not found" and "multiple matches" (when not replace_all)
  - [ ] Both registered in ToolRegistry

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Write and Edit tools work correctly
    Tool: Bash (cargo test)
    Steps:
      1. Write tool: create file in nested dir → assert file exists with correct content
      2. Edit tool: replace "foo" with "bar" → assert content updated
      3. Edit tool: old_string not found → assert error returned
      4. Edit tool: multiple matches without replace_all → assert error
      5. Edit tool: multiple matches with replace_all → assert all replaced
    Expected Result: File write and edit operations correct
    Evidence: Test output captured
  ```

  **Commit**: YES (groups with 6, 7)
  - Message: `feat(tools): implement write and edit tools with string replacement`
  - Files: `src/tools/write.rs`, `src/tools/edit.rs`, `src/tools/mod.rs`
  - Pre-commit: `cargo check`

---

- [ ] 10. TUI: Tool Call Rendering

  **What to do**:
  - Modify `src/ui/chat.rs` to render ContentBlock types beyond Text:
    - `ToolUse`: Show tool name, collapsible parameter display, loading indicator during execution
    - `ToolResult`: Show tool output, truncated with "show more" indicator, color-code errors
    - `Thinking`: Show thinking content in dimmed/italic style (if model supports)
  - Add new AppEvent handling: `ToolCallStart { name, input }` shows "⚡ Running {name}..." status
  - Add `ToolCallComplete { name, output, is_error }` updates the tool result display
  - Add collapsible/expandable sections for long tool outputs (toggle with keybind)
  - Update status bar to show "Running tool: {name}" during tool execution
  - Update message rendering to handle Vec<ContentBlock> instead of plain text

  **Must NOT do**:
  - Don't implement fancy animations or progress bars
  - Don't implement image rendering in terminal

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: TUI rendering requires visual design sense — layout of tool calls, color coding, collapsible sections
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 — after Tasks 6, 7, 8 (needs working tools to render)
  - **Blocks**: Tasks 25-30 (TUI enhancements build on this)
  - **Blocked By**: Tasks 6, 7, 8 (needs tools to demonstrate rendering)

  **References**:

  **Pattern References**:
  - `src/ui/chat.rs:draw_chat()` — Current chat rendering. This is the file to modify.
  - `src/ui/chat.rs` lines with message rendering — Current text-only rendering. Must handle ContentBlock types.
  - `src/ui/theme.rs:Theme` — Color definitions for consistent styling
  - `src/app.rs:AppEvent` — Extended with ToolCallStart/Complete from Task 5

  **Acceptance Criteria**:
  - [ ] Text content blocks render as before (no regression)
  - [ ] ToolUse blocks show: tool name, parameters (formatted JSON), execution indicator
  - [ ] ToolResult blocks show: output text, red color for errors
  - [ ] Long outputs truncated with indicator
  - [ ] Status bar shows "Running tool: {name}" during execution
  - [ ] All ContentBlock types handled (no panics on unknown types)

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Tool calls render in TUI
    Tool: interactive_bash (tmux)
    Preconditions: openrust built with Tasks 1-8 completed, ANTHROPIC_API_KEY set
    Steps:
      1. tmux new-session: cargo run
      2. Wait for input prompt
      3. Type: "List the files in the current directory"
      4. Press Enter
      5. Wait for response (timeout 30s)
      6. Assert: Tool call visible (bash or list tool invocation shown)
      7. Assert: Tool result visible (directory listing)
      8. Assert: Final text response visible
      9. Press Ctrl+Q to exit
    Expected Result: Full tool calling cycle visible in TUI
    Evidence: Terminal output captured via tmux capture-pane
  ```

  **Commit**: YES
  - Message: `feat(ui): render tool calls and results in chat view`
  - Files: `src/ui/chat.rs`, `src/app.rs`
  - Pre-commit: `cargo check`

---

### Wave 2 — Complete Tool Suite

- [ ] 9. Grep Tool

  **What to do**:
  - Create `src/tools/grep.rs` implementing Tool trait
  - Parameters: `pattern: String` (regex), `path: Option<String>` (search root, default "."), `include: Option<String>` (file glob filter like "*.rs")
  - Walk directory tree, match regex against file contents, return matches with file:line:content
  - Skip hidden dirs, .git, target, node_modules, binary files
  - Limit results (max 100 files, configurable)
  - Use `regex` crate (already in deps)

  **Must NOT do**:
  - Don't depend on external ripgrep binary
  - Don't implement AST-aware search

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 — parallel with Tasks 11-17
  - **Blocks**: Task 21 (agents need all tools)
  - **Blocked By**: Task 5

  **References**:
  - `src/tools/file.rs:FileTool::search_files()` — Existing regex search. Migrate and enhance.

  **Acceptance Criteria**:
  - [ ] Regex pattern matching works across files
  - [ ] Include filter works (e.g., "*.rs" only searches Rust files)
  - [ ] Hidden dirs and common excludes skipped
  - [ ] Results formatted as "file:line: content"
  - [ ] Result count limited

  **Commit**: YES (groups with 11-17)
  - Message: `feat(tools): implement grep tool with regex search and file filtering`
  - Files: `src/tools/grep.rs`, `src/tools/mod.rs`

---

- [ ] 11. Glob Tool

  **What to do**:
  - Create `src/tools/glob.rs` implementing Tool trait
  - Parameters: `pattern: String` (glob like "**/*.rs"), `path: Option<String>` (root)
  - Use `glob` crate or implement with `std::fs` + pattern matching
  - Return matching file paths sorted by modification time
  - Limit to 100 results
  - Add `glob` crate to Cargo.toml

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel with 9, 12-17

  **Acceptance Criteria**:
  - [ ] Glob patterns match files correctly
  - [ ] Results sorted by modification time
  - [ ] Limited to 100 results

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement glob tool for file pattern matching`
  - Files: `src/tools/glob.rs`, `src/tools/mod.rs`, `Cargo.toml`

---

- [ ] 12. List Tool

  **What to do**:
  - Create `src/tools/list.rs` implementing Tool trait
  - Parameters: `path: Option<String>` (default ".")
  - List directory entries with metadata: name, type (file/dir/symlink), size
  - Dirs first, then files, alphabetical within each group
  - Refactor existing `FileTool::list_directory()`

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel

  **Acceptance Criteria**:
  - [ ] Lists directory with type indicators
  - [ ] Sorted: dirs first, then files
  - [ ] Size shown for files

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement list tool for directory listing`
  - Files: `src/tools/list.rs`, `src/tools/mod.rs`

---

- [ ] 13. Patch Tool

  **What to do**:
  - Create `src/tools/patch.rs` implementing Tool trait
  - Parameters: `file_path: String`, `diff: String` (unified diff format)
  - Parse unified diff, apply hunks to file
  - Use `similar` crate or manual unified diff parser
  - Handle context lines for accurate hunk placement
  - Return success/failure with details
  - Add `similar` crate to Cargo.toml

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel

  **Acceptance Criteria**:
  - [ ] Unified diff parsed correctly
  - [ ] Hunks applied to correct locations
  - [ ] Error on hunk mismatch (context doesn't match)

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement patch tool for unified diff application`
  - Files: `src/tools/patch.rs`, `src/tools/mod.rs`, `Cargo.toml`

---

- [ ] 14. TodoWrite + TodoRead Tools

  **What to do**:
  - Create `src/tools/todo.rs` with two Tool implementations: TodoWriteTool, TodoReadTool
  - TodoWrite parameters: `todos: Vec<{ content, status, priority }>` — replaces entire todo list
  - TodoRead parameters: none — returns current todo list
  - Store todos in App state (in-memory per session, serialized with session)
  - Status values: "pending", "in_progress", "completed", "cancelled"

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel

  **Acceptance Criteria**:
  - [ ] TodoWrite replaces entire todo list
  - [ ] TodoRead returns current list formatted
  - [ ] Todos persist with session

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement todowrite and todoread tools`
  - Files: `src/tools/todo.rs`, `src/tools/mod.rs`, `src/app.rs` (state)

---

- [ ] 15. Question Tool

  **What to do**:
  - Create `src/tools/question.rs` implementing Tool trait
  - Parameters: `questions: Vec<{ question, header, options: Vec<{ label, description }>, multiple: Option<bool> }>`
  - When executed, display question UI in TUI (popup/overlay)
  - Wait for user selection, return selected option labels as array
  - Integrate with TUI event loop for user input capture
  - Add new AppMode::QuestionPrompt for handling question display

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: Requires TUI popup design for option selection
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel

  **Acceptance Criteria**:
  - [ ] Questions displayed as TUI overlay/popup
  - [ ] Options selectable with arrow keys + Enter
  - [ ] Multiple selection supported when enabled
  - [ ] Selected labels returned to tool caller

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement question tool with TUI option selection`
  - Files: `src/tools/question.rs`, `src/tools/mod.rs`, `src/ui/question.rs`, `src/app.rs`

---

- [ ] 16. WebFetch + WebSearch Tools

  **What to do**:
  - Create `src/tools/webfetch.rs` implementing Tool trait
    - Parameters: `url: String`, `format: Option<String>` ("markdown"|"text"|"html", default "markdown")
    - HTTP GET the URL via reqwest, extract text content
    - Convert HTML to markdown (basic: strip tags, convert links/headers)
    - Truncate to reasonable limit (50KB)
  - Create `src/tools/websearch.rs` implementing Tool trait
    - Parameters: `query: String`, `num_results: Option<u32>` (default 5)
    - Use a search API (configurable: Google Custom Search, or built-in scraping)
    - Return results with title, URL, snippet
  - Both use existing reqwest dependency

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel

  **Acceptance Criteria**:
  - [ ] WebFetch retrieves URL content
  - [ ] HTML to markdown conversion (basic)
  - [ ] WebSearch returns structured results
  - [ ] Content truncated to limit

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement webfetch and websearch tools`
  - Files: `src/tools/webfetch.rs`, `src/tools/websearch.rs`, `src/tools/mod.rs`

---

- [ ] 17. Skill Tool

  **What to do**:
  - Create `src/tools/skill.rs` implementing Tool trait
  - Parameters: `name: String` — skill name to load
  - Look up skill files from `.openrust/skills/` or configured skill directory
  - Read skill markdown file, return content to AI as context
  - Support listing available skills when no name given

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 2, parallel

  **Acceptance Criteria**:
  - [ ] Skill files loaded from configured directory
  - [ ] Skill content returned as text
  - [ ] Missing skill returns helpful error

  **Commit**: YES (groups with Wave 2 tools)
  - Message: `feat(tools): implement skill tool for loading skill files`
  - Files: `src/tools/skill.rs`, `src/tools/mod.rs`

---

### Wave 3 — Config & Session Overhaul

- [ ] 18. JSON/JSONC Config System

  **What to do**:
  - Replace TOML config with JSON/JSONC format
  - Add JSONC parser (strip comments before serde_json parse, or use `json_comments` crate)
  - Redesign Config struct to match OpenCode schema:
    - `provider: { default, anthropic: { api_key, model, max_tokens }, openai: {...}, google: {...}, ... }`
    - `theme: String`
    - `keybinds: HashMap<String, String>`
    - `permissions: { allow: [], ask: [], deny: [] }`
    - `mcp_servers: HashMap<String, McpServerConfig>`
    - `lsp: { enabled, servers: HashMap<String, LspConfig> }`
  - Config file path: `openrust.json` (project root) or `~/.config/openrust/openrust.json` (global)
  - Migration: detect old `config.toml`, convert to `openrust.json`, warn user
  - Support `$schema` field (ignored during parse)
  - Add `json_comments` or `serde_jsonc` crate to Cargo.toml, remove `toml` crate

  **Must NOT do**:
  - Don't implement variable substitution yet (Task 19)
  - Don't implement remote config (.well-known) yet

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Large config struct redesign affecting many files
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (within Wave 3)
  - **Parallel Group**: Wave 3 — parallel with Wave 2
  - **Blocks**: Task 19, 20, 25-30
  - **Blocked By**: Task 5 (basic app working)

  **References**:
  - `src/config.rs` — ENTIRE FILE. Current TOML config. Replace with JSON/JSONC.
  - OpenCode config docs: https://opencode.ai/docs/config/ — Full schema reference

  **Acceptance Criteria**:
  - [ ] JSONC config loads (with // comments stripped)
  - [ ] All OpenCode config fields represented in Rust structs
  - [ ] Global + project config merging (project overrides global)
  - [ ] Migration from config.toml → openrust.json works
  - [ ] Missing config creates sensible defaults

  **Commit**: YES
  - Message: `feat(config): replace TOML with JSON/JSONC config matching OpenCode schema`
  - Files: `src/config.rs`, `Cargo.toml`

---

- [ ] 19. Config Variable Substitution + Schema

  **What to do**:
  - Implement `{env:VAR_NAME}` substitution in config string values
  - Implement `{file:path/to/file}` substitution (read file content)
  - Apply substitution after loading, before parsing typed config
  - Implement config validation: required fields, valid provider names, valid theme names
  - Support config inheritance: global → project → CLI overrides

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: NO — after Task 18

  **Acceptance Criteria**:
  - [ ] `{env:ANTHROPIC_API_KEY}` resolves to env var value
  - [ ] `{file:./api-key.txt}` resolves to file content
  - [ ] Missing env var → helpful error message
  - [ ] Config validation catches invalid values

  **Commit**: YES
  - Message: `feat(config): add variable substitution and config validation`
  - Files: `src/config.rs`

---

- [ ] 20. SQLite Session Storage

  **What to do**:
  - Replace JSON file sessions with SQLite database
  - Add `rusqlite` crate to Cargo.toml (with `bundled` feature for self-contained binary)
  - Database path: `~/.local/share/openrust/sessions.db`
  - Schema: `sessions` table (id, title, created_at, updated_at, provider, model, working_directory), `messages` table (id, session_id, role, content_json, created_at, ordering)
  - content_json stores `Vec<ContentBlock>` as JSON blob
  - Implement: create_session, load_session, save_message, list_sessions, delete_session
  - Auto-migration: detect old JSON files, import into SQLite, move originals to backup dir
  - Update `Session` struct methods to use SQLite

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Database schema design, migration logic, replacing entire persistence layer
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES — parallel with Wave 2
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 43 (undo/redo uses session data)
  - **Blocked By**: Task 18 (config tells us where to store)

  **References**:
  - `src/session.rs` — ENTIRE FILE. Replace internals with rusqlite.
  - `src/ai/types.rs:ContentBlock` — Content stored as JSON in messages table

  **Acceptance Criteria**:
  - [ ] SQLite database created automatically on first run
  - [ ] Sessions CRUD works (create, load, list, delete)
  - [ ] Messages stored with full ContentBlock JSON
  - [ ] Old JSON sessions auto-migrated
  - [ ] Database survives ungraceful exit (WAL mode)

  **Commit**: YES
  - Message: `feat(session): replace JSON files with SQLite storage`
  - Files: `src/session.rs`, `Cargo.toml`

---

### Wave 4 — Agent System

- [ ] 21. Agent Trait + AgentConfig + Dispatch

  **What to do**:
  - Create `src/agents/mod.rs` with agent system
  - Define `AgentConfig`: name, system_prompt, tool_whitelist (list of allowed tool names), model_override, max_tokens_override, max_iterations
  - Define `AgentDispatch`: holds all agent configs, can spawn agent execution with appropriate config
  - Modify agent loop (Task 5) to accept AgentConfig and filter tools accordingly
  - Each agent gets its own system prompt template
  - Tool filtering: agent loop only includes whitelisted tools in API request

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: NO — Wave 4 sequential start

  **References**:
  - `src/agent_loop.rs` (from Task 5) — Loop to parameterize with agent config
  - OpenCode agents docs: https://opencode.ai/docs/agents/

  **Acceptance Criteria**:
  - [ ] AgentConfig defines agent behavior (tools, model, system prompt)
  - [ ] Agent loop respects tool whitelist
  - [ ] Multiple agent configs can coexist
  - [ ] Agent dispatch selects correct config by name

  **Commit**: YES
  - Message: `feat(agents): add agent trait, config, and dispatch system`
  - Files: `src/agents/mod.rs`, `src/agents/config.rs`, `src/agent_loop.rs`

---

- [ ] 22. Build + Plan Agents

  **What to do**:
  - Configure `build` agent: full tool access, primary agent for implementation tasks
    - System prompt: coding assistant focused on implementation, testing, debugging
    - Tools: all tools
  - Configure `plan` agent: read-only tools only, primary agent for planning
    - System prompt: planning and analysis focus
    - Tools: read, grep, glob, list, webfetch, websearch, todowrite, todoread
    - Cannot use: bash, write, edit, patch
  - Add agent selection UI: user can switch between build and plan agents
  - Default to build agent

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: NO — after Task 21

  **Acceptance Criteria**:
  - [ ] Build agent has all tools
  - [ ] Plan agent has only read-only tools
  - [ ] User can switch agents (slash command or keybind)

  **Commit**: YES
  - Message: `feat(agents): configure build and plan primary agents`
  - Files: `src/agents/build.rs`, `src/agents/plan.rs`, `src/agents/mod.rs`

---

- [ ] 23. General + Explore Subagents

  **What to do**:
  - Implement subagent spawning mechanism: parent agent can request a subagent via tool call
  - `general` subagent: full tool access, for delegated multi-step tasks
    - Runs in isolated context (own conversation, but shared file system)
    - Returns result to parent agent
  - `explore` subagent: read-only, fast codebase exploration
    - Limited tool set (read, grep, glob, list)
    - Optimized for speed (smaller model if configured)
    - Returns findings to parent agent
  - Add `subagent` tool that primary agents can call to spawn subagents

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: Complex concurrency — subagent runs as independent conversation, needs result collection and error handling
  - **Skills**: []

  **Parallelization**: NO — after Task 22

  **Acceptance Criteria**:
  - [ ] Subagent spawns with isolated conversation
  - [ ] General subagent has full tool access
  - [ ] Explore subagent limited to read-only tools
  - [ ] Results returned to parent conversation
  - [ ] Subagent errors don't crash parent

  **Commit**: YES
  - Message: `feat(agents): implement general and explore subagent spawning`
  - Files: `src/agents/subagent.rs`, `src/agents/mod.rs`, `src/tools/subagent.rs`

---

- [ ] 24. Hidden Agents: Compaction + Title + Summary

  **What to do**:
  - `compaction` agent: triggered when context window approaches limit
    - Summarizes conversation history into compact form
    - Replaces old messages with summary message
    - System prompt focused on preserving key context
  - `title` agent: auto-generates session title from first few messages
    - Quick, single-shot API call
    - Updates session title in storage
  - `summary` agent: generates session summary for session list view
    - Brief summary of what was accomplished
  - All three are "hidden" — not user-selectable, triggered automatically
  - Implement context length tracking (count tokens approximately)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: NO — after Task 23

  **Acceptance Criteria**:
  - [ ] Compaction triggers automatically when context nears limit
  - [ ] Title auto-generated after first exchange
  - [ ] Summary generated on session save/close
  - [ ] Compacted conversation preserves critical context

  **Commit**: YES
  - Message: `feat(agents): implement compaction, title, and summary hidden agents`
  - Files: `src/agents/compaction.rs`, `src/agents/title.rs`, `src/agents/summary.rs`

---

### Wave 5 — TUI Enhancement

- [ ] 25. Slash Command System (Part 1)

  **What to do**:
  - Implement slash command parser: detect `/` prefix in input
  - Implement core commands:
    - `/new` — create new session
    - `/help` — show help overlay
    - `/sessions` — show session list
    - `/exit` — quit application
    - `/compact` — trigger context compaction
    - `/details` — show message details (tokens, timing)
    - `/editor` — open $EDITOR for multi-line input
    - `/export` — export conversation to markdown
  - Command registry with name, description, handler function
  - Tab completion for command names

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: NO — Wave 5 sequential start

  **References**:
  - `src/main.rs` — Key handling where input is processed. Intercept / prefix.
  - `src/app.rs` — App methods for new session, etc.
  - OpenCode TUI docs: https://opencode.ai/docs/tui/

  **Acceptance Criteria**:
  - [ ] `/` prefix detected and parsed as command
  - [ ] All listed commands functional
  - [ ] Tab completion shows matching commands
  - [ ] Unknown command shows error

  **Commit**: YES
  - Message: `feat(tui): implement slash command system with core commands`
  - Files: `src/commands/mod.rs`, `src/main.rs`, `src/app.rs`

---

- [ ] 26. Slash Commands Part 2

  **What to do**:
  - `/models` — list and switch models
  - `/themes` — list and switch themes
  - `/thinking` — toggle extended thinking display
  - `/connect` — connect to MCP server (placeholder until Wave 7)
  - `/share` + `/unshare` — share/unshare conversation (placeholder until Wave 10)
  - `/undo` + `/redo` — undo/redo last change (placeholder until Wave 10)
  - `/init` — initialize project configuration

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: NO — after Task 25

  **Acceptance Criteria**:
  - [ ] All listed commands registered and functional (or stub with "not yet implemented")
  - [ ] /models shows available models
  - [ ] /themes switches theme immediately

  **Commit**: YES
  - Message: `feat(tui): implement remaining slash commands`
  - Files: `src/commands/mod.rs`

---

- [ ] 27. File References (@file) + Bash Prefix (!)

  **What to do**:
  - Parse `@filename` in user input: read file content, attach to message as context
  - Support `@directory/` to include directory listing
  - Parse `!command` prefix: execute shell command directly, show output
  - File path completion for @ references (using glob)
  - Multiple @ references in single message

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — parallel with 28-30

  **Acceptance Criteria**:
  - [ ] @file reads file and includes in message
  - [ ] @dir/ lists directory contents
  - [ ] !command executes and shows output
  - [ ] Multiple @ references work

  **Commit**: YES
  - Message: `feat(tui): add @file references and !command bash prefix`
  - Files: `src/app.rs`, `src/main.rs`

---

- [ ] 28. Multi-line Input + Improved Markdown

  **What to do**:
  - Multi-line text input (Shift+Enter for newline, Enter to send)
  - Or use `tui-textarea` crate for proper text area widget
  - Improved markdown rendering in responses:
    - Tables (basic ASCII table rendering)
    - Links (show URL)
    - Horizontal rules
    - Nested lists
    - Better code block syntax highlighting (leverage syntect already in deps)
  - Use `pulldown-cmark` for proper markdown parsing (instead of manual regex)
  - Add `pulldown-cmark` crate to Cargo.toml

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: Rich text rendering requires visual design sense
  - **Skills**: []

  **Parallelization**: YES — parallel with 27, 29, 30

  **Acceptance Criteria**:
  - [ ] Shift+Enter creates newline in input
  - [ ] Enter sends message
  - [ ] Markdown tables render as ASCII tables
  - [ ] Code blocks have proper syntax highlighting
  - [ ] Nested lists indent correctly

  **Commit**: YES
  - Message: `feat(ui): multi-line input and improved markdown rendering`
  - Files: `src/ui/chat.rs`, `src/main.rs`, `Cargo.toml`

---

- [ ] 29. Theme System

  **What to do**:
  - Expand theme system from 2 hardcoded themes to configurable system
  - Built-in themes: catppuccin-mocha, catppuccin-latte, dracula, tokyo-night, solarized-dark, solarized-light, nord, gruvbox
  - Theme definition struct with all color fields
  - Theme loading from JSON/JSONC config
  - Custom theme support: user defines colors in config
  - `/themes` slash command integration
  - Live theme switching (no restart needed)

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
  - **Skills**: []

  **Parallelization**: YES — parallel with 27, 28, 30

  **Acceptance Criteria**:
  - [ ] 8+ built-in themes available
  - [ ] /themes command lists and switches themes
  - [ ] Custom themes definable in config
  - [ ] Live switching works without restart

  **Commit**: YES
  - Message: `feat(ui): expand theme system with 8+ built-in themes and custom support`
  - Files: `src/ui/theme.rs`, `src/config.rs`

---

- [ ] 30. Keybind System

  **What to do**:
  - Implement leader key system (default: Ctrl+X)
  - After leader key, single key triggers action
  - Configurable keybinds from JSON config
  - Default keybinds matching OpenCode: leader+n (new), leader+s (save), leader+l (sessions), leader+h (help), leader+q (quit), leader+m (models), leader+t (themes), etc.
  - 60+ keybind mappings
  - Keybind display in help overlay
  - Keybind conflict detection

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: YES — parallel with 27, 28, 29

  **Acceptance Criteria**:
  - [ ] Leader key (Ctrl+X) followed by action key works
  - [ ] Keybinds configurable from config file
  - [ ] Help overlay shows current keybinds
  - [ ] Default keybinds cover 60+ actions

  **Commit**: YES
  - Message: `feat(tui): implement configurable leader-key keybind system`
  - Files: `src/keybinds.rs`, `src/main.rs`, `src/config.rs`, `src/ui/help.rs`

---

### Wave 6 — Permissions

- [ ] 31. Permission System

  **What to do**:
  - Create `src/permissions/mod.rs` with permission checking
  - Permission levels: `allow` (auto-approve), `ask` (prompt user), `deny` (block)
  - Per-tool permissions: `{ "bash": "ask", "write": "allow", "edit": "allow" }`
  - Glob patterns for bash commands: `{ "allow": ["git *", "cargo *"], "deny": ["rm -rf *"] }`
  - Permission prompt UI: show tool name + parameters, ask allow/deny
  - Remember decisions for session (optional: "always allow this")
  - Load permissions from config

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: NO — Wave 6 start

  **References**:
  - OpenCode permissions docs: https://opencode.ai/docs/permissions/

  **Acceptance Criteria**:
  - [ ] Tools with "allow" execute without prompt
  - [ ] Tools with "ask" show permission prompt
  - [ ] Tools with "deny" blocked with message
  - [ ] Bash command glob matching works
  - [ ] "Always allow" option available

  **Commit**: YES
  - Message: `feat(permissions): implement allow/ask/deny permission system with glob patterns`
  - Files: `src/permissions/mod.rs`, `src/agent_loop.rs`, `src/config.rs`

---

- [ ] 32. Agent Overrides + Doom Loop + External Directory

  **What to do**:
  - Per-agent permission overrides in config
  - `doom_loop` detection: detect when agent is stuck in repetitive tool call pattern
    - Track recent tool calls, detect repetition (same tool + similar input N times)
    - Break loop with warning message
  - `external_directory` control: restrict file operations to project directory
    - Validate all file paths are within allowed directories
    - Block access to sensitive paths (~/.ssh, ~/.env, etc.)
  - Add these checks to agent loop

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: NO — after Task 31

  **Acceptance Criteria**:
  - [ ] Agent-specific permission overrides work
  - [ ] Doom loop detected after N repetitions
  - [ ] File operations restricted to allowed directories
  - [ ] Sensitive paths blocked

  **Commit**: YES
  - Message: `feat(permissions): add agent overrides, doom loop detection, directory restrictions`
  - Files: `src/permissions/mod.rs`, `src/agent_loop.rs`

---

### Wave 7 — MCP Integration

- [ ] 33. MCP Local Client (stdio)

  **What to do**:
  - Create `src/mcp/mod.rs` with MCP client implementation
  - Implement stdio transport: spawn MCP server process, communicate via stdin/stdout
  - JSON-RPC 2.0 protocol: send requests, receive responses and notifications
  - Implement MCP lifecycle: initialize → list tools → call tool → shutdown
  - Register MCP tools in ToolRegistry (prefixed with server name)
  - MCP server configuration from config file
  - Handle server crashes and restarts
  - Evaluate `rmcp` crate vs custom implementation

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: JSON-RPC protocol implementation, process lifecycle management, async I/O between processes
  - **Skills**: []

  **Parallelization**: NO — Wave 7 start

  **References**:
  - MCP specification: https://spec.modelcontextprotocol.io/
  - OpenCode MCP docs: https://opencode.ai/docs/mcp-servers/

  **Acceptance Criteria**:
  - [ ] MCP server process spawned and initialized
  - [ ] Tool list retrieved from server
  - [ ] Tools callable through ToolRegistry
  - [ ] Server crash handled gracefully
  - [ ] Server shutdown on app exit

  **Commit**: YES
  - Message: `feat(mcp): implement local MCP client with stdio transport`
  - Files: `src/mcp/mod.rs`, `src/mcp/stdio.rs`, `src/mcp/protocol.rs`, `Cargo.toml`

---

- [ ] 34. MCP Remote Client (HTTP/SSE)

  **What to do**:
  - Implement HTTP/SSE transport for remote MCP servers
  - HTTP POST for requests, SSE for streaming responses/notifications
  - Use existing reqwest for HTTP calls
  - Handle connection lifecycle: connect → initialize → operate → disconnect
  - Support server-sent events for streaming tool results
  - Reconnection logic for dropped connections

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: NO — after Task 33

  **Acceptance Criteria**:
  - [ ] HTTP transport connects to remote MCP server
  - [ ] SSE events received and processed
  - [ ] Tool calls work over HTTP
  - [ ] Reconnection on disconnect

  **Commit**: YES
  - Message: `feat(mcp): implement remote MCP client with HTTP/SSE transport`
  - Files: `src/mcp/http.rs`, `src/mcp/mod.rs`

---

- [ ] 35. MCP OAuth + Tool Registration

  **What to do**:
  - Implement OAuth 2.0 flow for authenticated MCP servers
  - Dynamic client registration
  - Token storage and refresh
  - Register MCP tools with appropriate namespacing (server_name::tool_name)
  - MCP tool parameters mapped to ToolDefinition JSON Schema
  - Handle MCP resources and prompts (not just tools)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: NO — after Task 34

  **Acceptance Criteria**:
  - [ ] OAuth flow completes for authenticated servers
  - [ ] Tokens stored and refreshed
  - [ ] MCP tools properly namespaced in registry
  - [ ] MCP resources accessible

  **Commit**: YES
  - Message: `feat(mcp): add OAuth support and tool registration for remote MCP servers`
  - Files: `src/mcp/oauth.rs`, `src/mcp/mod.rs`

---

### Wave 8 — LSP Integration

- [ ] 36. LSP Client + Diagnostics

  **What to do**:
  - Create `src/lsp/mod.rs` with LSP client
  - Use `lsp-types` crate for protocol types
  - Implement stdio transport: spawn language server, JSON-RPC via stdin/stdout
  - LSP lifecycle: initialize → initialized → textDocument/didOpen → textDocument/diagnostics
  - Collect diagnostics (errors, warnings) from language server
  - Make diagnostics available as tool (for AI to query)
  - Add `lsp-types` crate to Cargo.toml

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: LSP protocol is complex — lifecycle management, capability negotiation, async notifications
  - **Skills**: []

  **Parallelization**: YES — parallel with Wave 7

  **References**:
  - OpenCode LSP docs: https://opencode.ai/docs/lsp/
  - LSP specification: https://microsoft.github.io/language-server-protocol/

  **Acceptance Criteria**:
  - [ ] Language server spawned and initialized
  - [ ] Diagnostics received for open files
  - [ ] Diagnostics available to AI via tool
  - [ ] Server shutdown on app exit

  **Commit**: YES
  - Message: `feat(lsp): implement LSP client with diagnostics support`
  - Files: `src/lsp/mod.rs`, `src/lsp/client.rs`, `src/lsp/transport.rs`, `Cargo.toml`

---

- [ ] 37. LSP Server Configurations

  **What to do**:
  - Define configuration for 30+ language servers:
    - TypeScript (typescript-language-server), Rust (rust-analyzer), Python (pyright/pylsp), Go (gopls)
    - C/C++ (clangd), Java (jdtls), Ruby (solargraph), PHP (intelephense)
    - Dart (dart), Elixir (elixir-ls), Haskell (hls), Kotlin (kotlin-language-server)
    - Lua (lua-language-server), Nix (nil), OCaml (ocamllsp), Swift (sourcekit-lsp)
    - Svelte (svelte-language-server), Vue (volar), Astro (astro-ls), etc.
  - Auto-detection: determine which language server to use based on project files
  - Each config: command, args, root_uri patterns, initialization_options
  - Configurable via openrust.json

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Mostly configuration data, not complex logic
  - **Skills**: []

  **Parallelization**: NO — after Task 36

  **Acceptance Criteria**:
  - [ ] 30+ language server configurations defined
  - [ ] Auto-detection works for common languages
  - [ ] Custom server configuration via config file

  **Commit**: YES
  - Message: `feat(lsp): add configurations for 30+ language servers with auto-detection`
  - Files: `src/lsp/servers.rs`, `src/lsp/mod.rs`

---

### Wave 9 — Provider Expansion

- [ ] 38. Google Gemini Provider

  **What to do**:
  - Create `src/ai/google.rs` implementing Provider trait
  - Google Gemini API: `POST /v1beta/models/{model}:generateContent`
  - Support tool calling (function declarations → function calls → function responses)
  - SSE streaming support
  - Authentication: API key or OAuth

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — parallel with 39, 40, 41

  **Acceptance Criteria**:
  - [ ] Gemini API calls work with streaming
  - [ ] Tool calling works (function declarations)
  - [ ] API key authentication works

  **Commit**: YES
  - Message: `feat(providers): add Google Gemini provider with tool calling`
  - Files: `src/ai/google.rs`, `src/ai/mod.rs`

---

- [ ] 39. AWS Bedrock + Azure OpenAI Providers

  **What to do**:
  - `src/ai/bedrock.rs`: AWS Bedrock Converse API
    - AWS SigV4 authentication (use `aws-sigv4` crate or manual signing)
    - Support Claude, Llama, Mistral models on Bedrock
    - Tool calling via Bedrock Converse API format
  - `src/ai/azure.rs`: Azure OpenAI
    - Same as OpenAI but different base URL and auth (API key or Azure AD)
    - URL format: `{endpoint}/openai/deployments/{deployment}/chat/completions?api-version={version}`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: YES — parallel with 38, 40, 41

  **Acceptance Criteria**:
  - [ ] Bedrock authenticates with AWS credentials
  - [ ] Azure uses deployment-based URL format
  - [ ] Both support tool calling
  - [ ] Both support streaming

  **Commit**: YES
  - Message: `feat(providers): add AWS Bedrock and Azure OpenAI providers`
  - Files: `src/ai/bedrock.rs`, `src/ai/azure.rs`, `src/ai/mod.rs`, `Cargo.toml`

---

- [ ] 40. OpenRouter + Generic OpenAI-Compatible + Ollama

  **What to do**:
  - Refactor OpenAI provider to be reusable as base for compatible APIs
  - `OpenRouter`: OpenAI-compatible with `https://openrouter.ai/api/v1` base URL + extra headers
  - Generic OpenAI-compatible: user provides base_url, works with any OpenAI-compatible API
  - `Ollama`: `http://localhost:11434/v1` base URL, no auth needed
  - `LM Studio`, `llama.cpp`, `vLLM`: all OpenAI-compatible with different base URLs
  - Provider factory updated to handle all provider names

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — parallel with 38, 39, 41

  **Acceptance Criteria**:
  - [ ] OpenRouter works with proper headers
  - [ ] Generic OpenAI-compatible works with any base URL
  - [ ] Ollama connects to local server
  - [ ] All support tool calling (where the underlying model supports it)

  **Commit**: YES
  - Message: `feat(providers): add OpenRouter, Ollama, and generic OpenAI-compatible providers`
  - Files: `src/ai/openai.rs` (refactor), `src/ai/mod.rs`, `src/config.rs`

---

- [ ] 41. Provider Configuration System

  **What to do**:
  - Dynamic provider loading from config
  - Provider registry: name → config → provider instance
  - Custom provider definitions in config (user can add any OpenAI-compatible API)
  - Model listing: each provider exposes available models
  - Model selection: /models command shows all available models across providers
  - Provider health check: test connectivity on startup

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — parallel with 38, 39, 40

  **Acceptance Criteria**:
  - [ ] Providers loaded dynamically from config
  - [ ] Custom providers definable in config
  - [ ] Model listing works
  - [ ] /models shows all available models

  **Commit**: YES
  - Message: `feat(providers): dynamic provider configuration and model listing`
  - Files: `src/ai/mod.rs`, `src/config.rs`, `src/commands/models.rs`

---

### Wave 10 — Polish & Extended Features

- [ ] 42. CLI Mode

  **What to do**:
  - Non-interactive mode: `openrust --message "fix the bug in main.rs"`
  - Pipe mode: `echo "explain this code" | openrust`
  - Output to stdout (not TUI), machine-readable option (JSON output)
  - Support --format flag: text (default), json, markdown
  - Exit with appropriate code (0 success, 1 error)
  - Support --continue flag to continue last session

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**: YES — Wave 10, all parallel

  **References**:
  - OpenCode CLI docs: https://opencode.ai/docs/cli/

  **Acceptance Criteria**:
  - [ ] --message sends single message and prints response
  - [ ] Pipe input works
  - [ ] JSON output format works
  - [ ] Tool calls execute in CLI mode

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: CLI mode executes single message
    Tool: Bash
    Steps:
      1. cargo run -- --message "What is 2+2?" 2>&1
      2. Assert: output contains answer
      3. Assert: exit code 0
      4. echo "List files" | cargo run 2>&1
      5. Assert: output contains file listing
    Expected Result: CLI mode works non-interactively
    Evidence: Command output captured
  ```

  **Commit**: YES
  - Message: `feat(cli): implement non-interactive CLI mode`
  - Files: `src/main.rs`, `src/cli.rs`

---

- [ ] 43. Undo/Redo (Git-Based)

  **What to do**:
  - Before each tool execution that modifies files, create a git stash or commit snapshot
  - `/undo` command: revert last file-modifying operation using git
  - `/redo` command: re-apply reverted operation
  - Undo stack: track sequence of file operations with git refs
  - Use existing git tool functionality for git operations
  - Only applies to file-modifying tools (write, edit, patch, bash)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`git-master`]
    - `git-master`: Git operations expertise for snapshot/revert pattern

  **Parallelization**: YES — Wave 10, parallel

  **Acceptance Criteria**:
  - [ ] /undo reverts last file change
  - [ ] /redo re-applies reverted change
  - [ ] Multiple undo levels supported
  - [ ] Non-file operations not affected

  **Commit**: YES
  - Message: `feat(undo): implement git-based undo/redo for file operations`
  - Files: `src/undo.rs`, `src/agent_loop.rs`, `src/commands/mod.rs`

---

- [ ] 44. Conversation Sharing + Export

  **What to do**:
  - `/share` command: generate shareable URL or file
  - `/unshare` command: revoke sharing
  - `/export` command: export conversation to markdown, JSON, or HTML
  - Sharing: upload conversation to configurable endpoint (or local file)
  - Export formats: clean markdown with tool calls formatted, JSON with full data

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 10, parallel

  **Acceptance Criteria**:
  - [ ] /export creates markdown file of conversation
  - [ ] /share generates shareable artifact
  - [ ] Export includes tool calls formatted readably

  **Commit**: YES
  - Message: `feat(sharing): implement conversation sharing and export`
  - Files: `src/sharing.rs`, `src/commands/mod.rs`

---

- [ ] 45. Custom Commands

  **What to do**:
  - User-defined slash commands in config
  - Command template: name, description, prompt template with variable substitution
  - Example: `/review` → sends "Please review the code changes" with git diff attached
  - Commands loaded from config on startup
  - Integrated with slash command system

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 10, parallel

  **Acceptance Criteria**:
  - [ ] Custom commands defined in config
  - [ ] Commands appear in /help
  - [ ] Template variables substituted
  - [ ] Tab completion works

  **Commit**: YES
  - Message: `feat(commands): implement user-defined custom slash commands`
  - Files: `src/commands/custom.rs`, `src/config.rs`

---

- [ ] 46. Code Formatters

  **What to do**:
  - Auto-format files after write/edit operations
  - Language-specific formatter configuration:
    - Rust: rustfmt
    - JavaScript/TypeScript: prettier
    - Python: black/ruff
    - Go: gofmt
    - etc.
  - Configurable in openrust.json: `{ "formatters": { "*.rs": "rustfmt", "*.ts": "prettier --write" } }`
  - Format on save, skip if formatter not installed
  - Detect formatter availability with `which`

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 10, parallel

  **Acceptance Criteria**:
  - [ ] Files auto-formatted after write/edit
  - [ ] Formatter configured per file extension
  - [ ] Missing formatter skipped gracefully
  - [ ] Format can be disabled in config

  **Commit**: YES
  - Message: `feat(formatters): implement auto-formatting with language-specific formatters`
  - Files: `src/formatters.rs`, `src/tools/write.rs`, `src/tools/edit.rs`, `src/config.rs`

---

- [ ] 47. Plugins + File Watcher

  **What to do**:
  - Plugin system: loadable extensions that can add tools, commands, or UI elements
  - Plugin definition in config: `{ "plugins": { "my-plugin": { "command": "path/to/plugin", "type": "stdio" } } }`
  - Plugins communicate via stdio (like MCP but simpler)
  - File watcher: detect file changes in project directory
    - Use `notify` crate for filesystem events
    - Notify AI of file changes (optional, configurable)
    - Re-read modified files if referenced in context
  - Add `notify` crate to Cargo.toml

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: YES — Wave 10, parallel

  **Acceptance Criteria**:
  - [ ] Plugins loadable from config
  - [ ] Plugin tools appear in registry
  - [ ] File watcher detects changes
  - [ ] File change notifications configurable

  **Commit**: YES
  - Message: `feat(plugins): implement plugin system and file watcher`
  - Files: `src/plugins/mod.rs`, `src/watcher.rs`, `Cargo.toml`

---

- [ ] 48. Custom Tools + Rules/Instructions

  **What to do**:
  - Custom tool definitions in config (like MCP but simpler):
    - `{ "tools": { "my-tool": { "command": "python script.py", "description": "...", "parameters": {...} } } }`
    - Execute as subprocess, pass parameters as JSON on stdin
  - Rules/instructions system:
    - Load from `.openrust/rules.md` or config
    - Prepend to system prompt for all conversations
    - Support project-level and user-level rules
  - Integrate custom tools into ToolRegistry

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**: YES — Wave 10, parallel

  **Acceptance Criteria**:
  - [ ] Custom tools executable from config definition
  - [ ] Rules loaded and prepended to system prompt
  - [ ] Project + user rules merge

  **Commit**: YES
  - Message: `feat(custom): implement custom tool definitions and rules/instructions`
  - Files: `src/tools/custom.rs`, `src/rules.rs`, `src/config.rs`

---

### Wave 11 — Testing

- [ ] 49. Test Infrastructure

  **What to do**:
  - Set up test infrastructure:
    - Test helpers module: `src/test_helpers.rs` (or `tests/helpers/mod.rs`)
    - Mock provider for testing (returns predefined responses)
    - Mock tool for testing (records calls, returns configured output)
    - Test fixtures directory: `tests/fixtures/`
  - Unit tests for core modules:
    - ContentBlock serialization/deserialization
    - ToolRegistry CRUD operations
    - Config loading, merging, variable substitution
    - Session SQLite operations
    - Permission checking logic
    - Slash command parsing
  - Add any needed test dependencies to Cargo.toml

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: Can start any time after Wave 1

  **Acceptance Criteria**:
  - [ ] `cargo test` runs and passes
  - [ ] Mock provider and tool available for tests
  - [ ] Core modules have unit tests
  - [ ] Tests don't require API keys

  **Commit**: YES
  - Message: `test: add test infrastructure and core module unit tests`
  - Files: `src/test_helpers.rs`, `tests/`, various `#[cfg(test)]` modules

---

- [ ] 50. Integration Tests

  **What to do**:
  - Integration tests for critical paths:
    - Agent loop: mock provider → tool_use → tool execution → tool_result → end_turn
    - Multi-tool chain: provider returns multiple tool calls, all executed
    - Error handling: tool fails, agent loop continues with error result
    - Config loading: full config lifecycle (create default, modify, reload)
    - Session lifecycle: create, add messages, save, load, verify
  - End-to-end test with mock provider:
    - Send message → agent loop → tool calls → final response
    - Verify correct number of API calls made
    - Verify tool results included in conversation

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**: After Wave 1

  **Acceptance Criteria**:
  - [ ] `cargo test` passes all integration tests
  - [ ] Agent loop integration test covers full cycle
  - [ ] Error handling test covers tool failures
  - [ ] No tests require network access or API keys

  **Commit**: YES
  - Message: `test: add integration tests for agent loop and critical paths`
  - Files: `tests/agent_loop.rs`, `tests/config.rs`, `tests/session.rs`

---

## Commit Strategy

| After Task(s) | Message | Key Files |
|---------------|---------|-----------|
| 1 | `refactor(types): replace String content with ContentBlock type system` | ai/types.rs, app.rs |
| 2 | `feat(tools): add Tool trait and ToolRegistry` | tools/traits.rs, tools/mod.rs |
| 3 | `feat(anthropic): add tool calling with streaming` | ai/anthropic.rs |
| 4 | `feat(openai): add tool calling with streaming` | ai/openai.rs |
| 5 | `feat(core): implement agent orchestration loop` | agent_loop.rs, app.rs |
| 6-8 | `feat(tools): implement bash, read, write, edit tools` | tools/*.rs |
| 10 | `feat(ui): render tool calls in chat view` | ui/chat.rs |
| 9, 11-17 | `feat(tools): complete tool suite (grep, glob, list, patch, todo, question, web, skill)` | tools/*.rs |
| 18-19 | `feat(config): JSON/JSONC config with variable substitution` | config.rs |
| 20 | `feat(session): SQLite storage` | session.rs |
| 21-24 | `feat(agents): complete agent system` | agents/*.rs |
| 25-30 | `feat(tui): slash commands, keybinds, themes, file refs` | commands/, ui/, keybinds.rs |
| 31-32 | `feat(permissions): permission system` | permissions/ |
| 33-35 | `feat(mcp): MCP client (stdio + HTTP + OAuth)` | mcp/ |
| 36-37 | `feat(lsp): LSP client with 30+ servers` | lsp/ |
| 38-41 | `feat(providers): Google, Bedrock, Azure, Ollama, OpenRouter` | ai/*.rs |
| 42-48 | `feat: CLI mode, undo/redo, sharing, commands, formatters, plugins` | various |
| 49-50 | `test: comprehensive test suite` | tests/ |

---

## Success Criteria

### Verification Commands
```bash
cargo build --release          # Expected: success, single binary
cargo clippy                   # Expected: no errors
cargo test                     # Expected: all tests pass
cargo fmt -- --check           # Expected: no formatting issues
./target/release/openrust --help  # Expected: shows help with all commands
./target/release/openrust --message "hello"  # Expected: CLI mode works
```

### Final Checklist
- [ ] All 15 built-in tools functional via tool calling
- [ ] Agent loop handles multi-step tool chains
- [ ] Build + Plan + General + Explore agents configured
- [ ] Compaction, Title, Summary agents working
- [ ] MCP client connects to at least stdio servers
- [ ] LSP diagnostics available for at least TypeScript + Rust
- [ ] JSON/JSONC config loads with variable substitution
- [ ] SQLite sessions persist across restarts
- [ ] All 17 slash commands functional
- [ ] 60+ keybinds configurable
- [ ] 8+ themes available
- [ ] Permission system enforced
- [ ] CLI mode works (--message, pipe)
- [ ] Undo/redo works for file operations
- [ ] `cargo build --release` produces single binary < 50MB
- [ ] Zero panics in normal operation
