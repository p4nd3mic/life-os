# 🧭 Master Plan — CodexMonitor Auto‑Memory, Browser Control & Skill Management (JMWillis)

## 🎯 Goal & Success Criteria
**Goal:** Deliver ClawdBot‑inspired **auto‑memory**, **Codex‑compatible browser control**, and **first‑class skills management** inside CodexMonitor (daemon + desktop + iOS + MCP), aligned to current repo patterns.

**✅ Success =**
- **Auto‑memory** triggers **before compaction risk** and writes durable entries to Supabase memory.
  - ✅ Triggered by `thread/tokenUsage/updated` near threshold.
  - ✅ Writes **daily** and/or **curated** entries with tags.
  - ✅ Manual “Flush to Memory” works from UI + RPC.
- **Browser control** works end‑to‑end.
  - ✅ iOS can create session → navigate → screenshot → click/tap.
  - ✅ Codex can call MCP browser tools to navigate + extract page content.
- **Skills management** is fully usable.
  - ✅ List/install/enable/disable/uninstall skills.
  - ✅ `skills/config/write` persists enablement.
  - ✅ SKILL.md metadata parsing + requirement warnings.

---

## 🚫 Non‑Goals / Out of Scope
- ❌ Full ClawdHub registry browsing/distribution.
- ❌ Anthropic/Claude Chrome extension workflows.
- ❌ Perfect compaction event detection (heuristics acceptable).
- ❌ Advanced browser UI (full live remote control) beyond screenshot + click.

---

## 🧩 Assumptions (Locked)
- **Token usage schema**: use `params.tokenUsage` (or `token_usage`) with `total.totalTokens` + `modelContextWindow` (as in current frontend normalization).
- **Browser automation**: use **Node + Playwright worker** (separate `browser-worker/` project).
- **MCP auth**: browser MCP uses env vars:
  - `CODEX_MONITOR_DAEMON_ADDR` (default `127.0.0.1:8787`)
  - `CODEX_MONITOR_DAEMON_TOKEN` (same token required by daemon)
- **Settings persistence**: `settings.json` remains source of truth.
- **File output**: this plan is saved to
  - `📁 /Volumes/YouTube 4TB/CodexMonitor/docs/MASTER_PLAN.md`

---

## 🧠 Proposed Solution (Chosen)
### ✅ Chosen Approach
1. **Auto‑Memory Coordinator** in daemon listens to token usage events, triggers a hidden summarizer turn, and writes to Supabase memory (daily + curated).
2. **Browser Service** in daemon spawns a **Playwright Node worker** and exposes RPC + MCP tools.
3. **Skills Manager** adds config write, metadata parsing, validation, and install/uninstall + UI controls.

### 🔁 Alternatives Considered (and why not)
- **Rust‑only CDP browser**: More complex, slower to stabilize; Playwright has stronger tooling + screenshot support.
- **Rolling transcript cache instead of thread resume**: faster later, but current thread history access is already stable and simpler for MVP.
- **Registry‑first skills**: unnecessary scope; install by git URL covers real needs now.

---

## 🏗️ System Design
### 🧩 Components
- **AutoMemoryCoordinator** (`src-tauri/src/memory/auto_flush.rs`)
  - Watches app‑server events → detects near‑limit tokens → triggers summarizer.
- **MemoryService** (existing) + **live reload** via `RwLock<Option<MemoryService>>`.
- **BrowserService** (`src-tauri/src/browser/service.rs`)
  - Manages worker process + sessions.
- **Browser Worker** (`browser-worker/`)
  - Playwright, JSON‑line protocol.
- **Browser MCP server** (`src-tauri/src/bin/codex_monitor_browser_mcp.rs`)
  - MCP tool adapter → daemon RPC.
- **Skills Manager** (`src-tauri/src/skills/skill_md.rs` + RPC wrappers)
  - Parses SKILL.md frontmatter + requirements; validates + installs.

### 🔄 Data Flow (Auto‑Memory)
`thread/tokenUsage/updated` → AutoMemoryCoordinator → snapshot from `thread/resume` → background summarizer turn → JSON parse → `MemoryService.append()`

### 🔄 Data Flow (Browser)
UI/MCP → daemon RPC → BrowserService → Playwright worker → screenshot / DOM snapshot → UI/MCP response

---

## 🔌 Interfaces & Data Contracts
### ✅ New/Extended RPC Methods
- `memory_flush_now { workspace_id, thread_id, force?, reason? }`
- `browser_create_session | browser_list_sessions | browser_close_session`
- `browser_navigate | browser_screenshot | browser_click | browser_type | browser_press | browser_snapshot | browser_evaluate`
- `skills_config_write { workspaceId, enabled[], disabled[] }`
- `skills_validate` → issues per skill
- `skills_install_from_git { sourceUrl, target, workspaceId? }`
- `skills_uninstall { name, target, workspaceId? }`

### 🧷 Settings Additions (Rust + Swift)
`AutoMemorySettings` with:
- `enabled`, `reserve_tokens_floor`, `soft_threshold_tokens`, `min_interval_seconds`, `max_turns`, `max_snapshot_chars`, `include_tool_output`, `include_git_status`, `write_daily`, `write_curated`

### 📦 Browser Worker Protocol (NDJSON)
Request:
```json
{ "id": "req-1", "method": "browser.screenshot", "params": { "sessionId": "b-1", "fullPage": true } }
```
Response:
```json
{ "id": "req-1", "result": { "base64Png": "...", "url": "...", "title": "..." } }
```

### 🧠 Memory Summarizer Output Schema
```json
{
  "no_reply": false,
  "title": "string",
  "tags": ["auto_memory", "workspace:..."],
  "daily_markdown": "...",
  "curated_markdown": "..."
}
```

---

## 🛠️ Execution Details (Phased)
### **Phase 1A — Auto‑Memory Backend (Rust)**
- ✅ Add `AutoMemorySettings` to `src-tauri/src/types.rs` + defaults.
- ✅ Implement `AutoMemoryCoordinator` (new `src-tauri/src/memory/auto_flush.rs`).
- ✅ Hook into `codex_monitor_daemon.rs` broadcast channel for `thread/tokenUsage/updated`.
- ✅ **MemoryService live reload**: wrap in `RwLock<Option<MemoryService>>` and rebuild on `update_app_settings`.
- ✅ Background summarizer uses existing commit‑message pattern in `src-tauri/src/codex.rs`.
- ✅ Add `memory_flush_now` RPC.

**Acceptance:** Trigger by lowering thresholds; see `memory_append` entries in Supabase.

### **Phase 1B — Auto‑Memory UI (iOS + Desktop)**
- ✅ iOS settings toggle + thresholds UI.
- ✅ “Flush to Memory” button in thread UI.
- ✅ Debug log: “Auto‑memory flush triggered/skipped.”

**Acceptance:** Toggle on/off, manual flush works from iPhone.

### **Phase 2 — Browser Daemon Service (RPC only)**
- ✅ Add `browser-worker/` Node project (Playwright).
- ✅ Add `BrowserService` (Rust) for process + session lifecycle.
- ✅ Add RPC endpoints.
- ✅ Add iOS `BrowserView` (screenshot + tap‑to‑click).

**Acceptance:** From iPhone on Tailscale: navigate + screenshot + click.

### **Phase 3 — Browser MCP Server**
- ✅ Implement `codex_monitor_browser_mcp` binary.
- ✅ MCP reads `CODEX_MONITOR_DAEMON_ADDR` + `CODEX_MONITOR_DAEMON_TOKEN`.
- ✅ Document `~/.codex/config.toml` entry.

**Acceptance:** Codex can open page + extract via browser tools.

### **Phase 4 — Skills Management**
- ✅ Add RPC wrapper `skills_config_write`.
- ✅ Parse `SKILL.md` YAML frontmatter.
- ✅ `skills_validate` requirement checks.
- ✅ Install/uninstall by git URL (global/workspace).
- ✅ iOS + Desktop Skills UI.

**Acceptance:** Install skill by URL, enable/disable persists.

### **Phase 5 — Polish & Docs**
- ✅ Update docs: `ARCHITECTURE.md`, `API_REFERENCE.md`, `DEPLOYMENT.md`.
- ✅ Include browser worker build/packaging steps.

---

## 🧪 Testing & Quality
- **Rust unit tests**
  - Auto‑memory threshold + compaction epoch detection.
  - SKILL.md YAML parsing.
- **Daemon integration test**
  - Mock MemoryWriter → assert auto‑flush calls.
- **Browser worker smoke tests (local)**
  - create → navigate → screenshot → close.
- **Manual acceptance**
  - iOS UI checks for auto‑memory + browser + skills.

---

## 🚀 Rollout, Observability & Ops
- **Feature flags** in settings (auto‑memory, browser enable).
- **Daemon logs** for flush decisions + worker restarts.
- **UI status toasts** for “flush triggered / skipped / failed”.
- **Rollback**: disable toggles or remove MCP config.

---

## ⚠️ Risks & Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| Token usage mismatch | Wrong trigger timing | Use existing normalization + add debug logging of raw tokenUsage payloads. |
| Compaction heuristic false positives | Extra flushes | Cooldown + epoch tracking + `min_interval_seconds`. |
| Playwright bundling issues | Worker fails on build | Add explicit build step + include dist in Tauri resources. |
| MCP auth leakage | Security risk | Require `CODEX_MONITOR_DAEMON_TOKEN`, never allow insecure mode for MCP. |

---

## ❓ Open Questions
- **None** (all decisions locked; verify with runtime logs if schema changes appear).
