# CLAUDE.md Update (copy/paste sections)

This file is intended to be pasted into `~/.claude/CLAUDE.md` as an update for the Life OS ecosystem.

**Snapshot context:** `life-os-codebase-20260202`

---

## Shared CODEX_HOME policy (2026-02)

Use this policy text in `~/.claude/CLAUDE.md`:

- **Shared CODEX_HOME (source of truth):** `/Users/jmwillis/.codex`
- **Legacy backup only (retired):** `/Users/jmwillis/.codex-lifeos`
- Life workspace override should point to shared root:
  - `/Users/jmwillis/Library/Application Support/com.dimillian.codexmonitor/workspaces.json`
- Automations are intentionally paused while single-root behavior is stabilized.

---

## CodexMonitor-lifeos (Life OS desktop app)

**Root:** `/Volumes/YouTube 4TB/CodexMonitor-lifeos`  
**Tech:** Tauri + React + Rust

### What it is
A desktop app that runs a Life Stream UI. User input becomes **cards**, which are:
- decided by Codex app-server (LLM)
- optionally enriched by MCP tool calls
- persisted to the Obsidian vault
- updated in UI via Tauri events

### Key files (read first)
- `src-tauri/src/life_stream/service.rs` — decision engine + orchestration
- `src-tauri/src/life_stream/mcp_registry.rs` — tool whitelist + prompt list
- `src-tauri/src/life_stream/mcp_bridge.rs` — MCP stdio bridge to Node
- `src-tauri/src/life_stream/types.rs` — Rust card types + patches
- `src/features/life-stream/components/stream/CardBubble.tsx` — card rendering
- `src/features/life-stream/utils/cardHighlights.ts` — highlight chips
- `src/features/life-stream/hooks/useLifeStream.ts` — invoke + event listener bridge
- `src/features/life-stream/state/streamStore.ts` — store + patching
- `src/types.ts` — shared UI types (workspaces/domains)
- `package.json`, `src-tauri/Cargo.toml`

### Tauri event channels
- `life_stream_event`
- `app_server_event`
- `open_file`

### Mode behavior
In this repo snapshot, `src/App.tsx` sets `lifeOsMode = true` (Life Stream is always-on).

---

## life-mcp (MCP Server)

**Root:** `/Volumes/YouTube 4TB/code/_archive/life-mcp`  
**Tech:** Node.js + MCP SDK

### What it is
A Model Context Protocol server exposing many “Life OS” tools:
- delivery logging + advisor
- nutrition logging
- finance logging
- media / youtube logging + enrichment
- tasks + analysis
- meta registry tools (list/search/execute)

### Key files
- `index.js` — Node entrypoint
- `src/server/mcp.js` — MCP server setup + tool registration
- `src/tool-registry.js` — categories + keyword index + registry
- `src/tools/*.js` — tool implementations
- `src/supabase/` — DB integration
- `src/clients/` — external API clients (TMDB, etc)
- `scripts/generate-tools-reference.js` — tool docs generator

### Categories (Node registry)
- delivery, advisor, nutrition, finance, youtube, media, creators, tasks, analysis, agents, goals, relationships, inbox, notes, knowledge, rewards, digest, meta

### High-frequency tool behavior
In stdio mode, MCP registers:
- meta tools
- high-frequency tools (subset)

To access non-registered tools from other clients, use `execute_tool`.

---

## Obsidian Vault (datastore)

**Root:** `/Volumes/YouTube 4TB/Obsidian`

### Expected structure
- `Stream/` — monthly logs (`YYYY-MM.md`)
- `Entities/` — entity markdown with YAML frontmatter (Food/ Media/ YouTube/ Delivery/ …)
- `Indexes/` — JSON indexes/caches
- `Runtime/` — session state + logs (`life-os-threads.json`, `life-stream.log`)
- `_config/` — configuration (`categories.yml`)

---

## life-os (system root)

**Root:** `/Volumes/YouTube 4TB/code/life-os`

### What it is
Config root for system definitions:
- `systems/*.yaml` — domain schemas/config
- `justfile` — command runner

---

## Critical gotchas (read these before edits)
- Rust ↔ TS casing: structs are camelCase, enums often snake_case.
- Tool availability depends on both Node “registered” set and Rust whitelist.
- “Nested result.result” response shapes exist; unwrap logic is defensive.
- `LIFE_OS_PATH` must be set correctly for life-mcp.
- Timezone boundaries can shift “today” if not Pacific/local aligned.
- Stream file metadata uses HTML comments; manual edits can break parsing.
- Types must stay in sync:
  - TS: `src/features/life-stream/types.ts`
  - Rust: `src-tauri/src/life_stream/types.rs`

---

## Where to go for more
- `ARCHITECTURE.md`, `API_REFERENCE.md`, `MCP_INTEGRATION.md`, `GOTCHAS.md`
