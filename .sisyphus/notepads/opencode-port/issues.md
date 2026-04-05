# Issues & Gotchas

## Known Issues (Pre-Existing)
- 33 compiler warnings (dead code — tools not wired to AI)
- Clippy suggests strip_prefix over manual slicing in ui/chat.rs
- auth/server.rs has unused mutable variable (chars in percent_decode)
- No tests exist (tokio-test in dev-deps but unused)
