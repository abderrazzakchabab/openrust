# Architectural Decisions

## Metis Review Decisions
- Vertical slice first: Anthropic + bash + agent loop + TUI rendering proves architecture
- SQLite deferred to Wave 3 (JSON sessions fine for early development)
- ContentBlock and StreamEvent are LOAD-BEARING types — design carefully
- Agent loop is co-equal blocker with tool calling parsing
- Dropped: portable-pty, axum, nucleo (unnecessary for this project)
- LOC estimate: 25,000-35,000

## Crate Selections
- SQLite: rusqlite (bundled) — simple, self-contained
- JSONC: json_comments or serde_jsonc
- Glob: glob crate
- Diff: similar crate
- Markdown: pulldown-cmark
- MCP: evaluate rmcp vs custom at Task 33

## Guardrails
- No Vercel AI SDK equivalent — native tool calling per provider
- No ORM — raw rusqlite SQL
- No web server — we're an MCP client
- No glob imports
