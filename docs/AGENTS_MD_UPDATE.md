# AGENTS.md Update (copy/paste sections)

This file is intended to be pasted into `~/.codex/AGENTS.md` as a quick-reference for agents working on Life OS.

**Snapshot context:** `life-os-codebase-20260202`

---

## Shared CODEX_HOME policy (2026-02)

Use this policy text in `~/.codex/AGENTS.md` so Official Codex and Life OS CodexMonitor stay aligned:

- **Shared CODEX_HOME (source of truth):** `/Users/jmwillis/.codex`
- **Legacy backup only (retired):** `/Users/jmwillis/.codex-lifeos`
- Life workspace override should point to the shared root in:
  - `/Users/jmwillis/Library/Application Support/com.dimillian.codexmonitor/workspaces.json`
- Automations are intentionally paused while single-root behavior is stabilized.

---

## Life OS architecture quick reference

```
UI (React) -> Tauri invoke -> Rust LifeStreamService
                         -> Codex app-server (turn/start)
                         -> MCP bridge (tools/call) -> life-mcp tools
                         -> Obsidian (Stream + Entities + Indexes + Runtime)
                         -> (optional) Supabase
```

---

## Common tasks & where to touch

### Add a new Life Stream card type
1) Add enum/type:
- TS: `src/features/life-stream/types.ts` (`CardType`)
- Rust: `src-tauri/src/life_stream/types.rs` (`CardType`)

2) Update decision/enrichment logic:
- Rust: `src-tauri/src/life_stream/service.rs`
  - decision parsing (JSON)
  - `apply_mcp_enrichment(...)` (if tool-backed)
  - entity + stats shaping

3) Update UI rendering:
- `src/features/life-stream/components/stream/CardBubble.tsx`
- optional: highlight rules in `src/features/life-stream/utils/cardHighlights.ts`

4) Update Obsidian persistence (if new schema):
- `src-tauri/src/life_stream/obsidian.rs`

### Add a new MCP tool (life-mcp)
1) Implement tool:
- `life-mcp/src/tools/<domain>.js`

2) Decide how it will be callable:
- Add to Node high-frequency tools if it should be directly callable
  - `life-mcp/src/tool-registry.js` (`HIGH_FREQUENCY_TOOL_NAMES`)
- or call via meta `execute_tool`

3) Allow Rust to call it (Life Stream):
- `src-tauri/src/life_stream/mcp_registry.rs` (whitelist + intent map)

4) Parse tool output into card fields:
- `src-tauri/src/life_stream/service.rs` (`apply_mcp_enrichment`)

### Fix “tool not found” errors
- Verify Node tool registration set:
  - `life-mcp/src/server/mcp.js`
- Verify tool exists in registry:
  - `life-mcp/src/tool-registry.js`
- Verify Rust whitelist includes the tool:
  - `src-tauri/src/life_stream/mcp_registry.rs`

### Update command surfaces / Tauri APIs
- Rust command registration is in:
  - `src-tauri/src/lib.rs` (`generate_handler![]`)
- JS calls mostly live in:
  - `src/services/tauri.ts` (typed wrappers)
  - `src/features/life-stream/hooks/useLifeStream.ts` (direct invoke)

---

## Tool patterns to recognize

### MCP tools return dual payloads
- human text summary for UI
- structured JSON for stats/entities/images

### “High-frequency” vs “full registry”
- Stdio MCP server exposes meta + high-frequency by default
- Everything else can be executed via meta `execute_tool`

### One Codex thread per day
- Runtime mapping: `Runtime/life-os-threads.json`
- Key: `<workspace_id>:<YYYY-MM-DD>`

---

## Critical gotchas (don’t skip)
- JSON casing: Rust camelCase structs, snake_case enums.
- Stream file parsing depends on HTML comment markers.
- Front-end patch application uses truthy checks (empty string patches won’t apply).
- Keep TS and Rust Life Stream types in sync.

---

## Key files (bookmark these)

### CodexMonitor-lifeos
- `src-tauri/src/life_stream/service.rs`
- `src-tauri/src/life_stream/mcp_registry.rs`
- `src-tauri/src/life_stream/mcp_bridge.rs`
- `src-tauri/src/life_stream/obsidian.rs`
- `src/features/life-stream/hooks/useLifeStream.ts`
- `src/features/life-stream/components/stream/CardBubble.tsx`
- `src/features/life-stream/state/streamStore.ts`

### life-mcp
- `src/server/mcp.js`
- `src/tool-registry.js`
- `src/tools/*.js`
