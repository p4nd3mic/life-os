# CodexMonitor Work Log

- Date: 2026-01-26
- Assistant: OpenAI (Codex)
- User: JMWillis

## Log
- 2026-01-26: Implemented Phase C iOS Memory tab (Swift models, RPC methods, MemoryView UI, and RootView/segmented navigation updates).
- 2026-01-26: Added MemoryView to Xcode project sources and updated CodexStore memory helpers.
- 2026-01-26: Ran cargo build and npm run build (warnings only).
## Phase D: MCP Server for Codex - Sun Jan 25 21:47:56 PST 2026
- Added codex_monitor_memory_mcp binary exposing memory_bootstrap, memory_search, memory_append
- Wired MCP tool handling to MemoryService (Supabase-backed)
- 2026-01-26: Added MCP Memory Server setup instructions to docs/DEPLOYMENT.md.
- 2026-01-26: Wired MCP server in ~/.codex/config.toml and built release binary for memory MCP.
- 2026-01-26: Added native iOS Memory search bar (searchable + submit handling).
- 2026-01-26: Added desktop Memory panel (Supabase-backed) with search, status, and append UI.
- 2026-01-26: Added right-panel tabs (Git/Memory) and wiring in desktop layout.
- 2026-01-26: Added composer plan-mode shortcut (Shift+Tab default) + settings field.
- 2026-01-26: Added iOS plan-mode toggle and collaboration mode wiring (list + send). 
- 2026-01-26: Added memory RPC helpers in web app and updated shared types.
- 2026-01-26: Ran cargo build and npm run build (warnings only).
- 2026-01-26: Added memory panel polish (results header, status chips, clearer tabs styling).
- 2026-01-26: Added memory panel shortcut (cmd+shift+m) and wiring + settings.
- 2026-01-26: Updated MCP binary path in docs + local config to src-tauri/target/release.
- 2026-01-26: Ran MCP memory tools end-to-end; search now falls back to text when embeddings fail.
- 2026-01-26: Ran cargo build --release and npm run build (warnings only).

## 2026-01-26 – Phase E polish / MiniMax rate-limit fix (follow-up)
- ✅ Rebuilt release after MiniMax client changes (cargo build --release).
- 🔧 Added MiniMax rate-limit handling (min interval + retry/backoff) in embeddings client.
- 🧪 MCP test run without env vars showed memory disabled (expected when SUPABASE_* not set).
