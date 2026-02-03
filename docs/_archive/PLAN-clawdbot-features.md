# Codex Implementation Plan: ClawdBot-Inspired Features

**Created:** 2026-01-26
**Source:** ChatGPT 5.2 Pro
**Status:** Planning

---

## 0. Ground Rules for This Change Set

### Primary goal

Add three major capabilities to CodexMonitor:

1. **Automatic Memory (pre-compaction flush)**
   * ClawdBot-style: when a thread is nearing context overflow, force a "durable memory save" *before* the model compacts/summarizes and context is lost.

2. **Browser control**
   * Provide **Codex-compatible** browser automation + optional remote UI.
   * Implement as a daemon service + MCP server (so Codex can call it as a tool).

3. **Skill management**
   * Give CodexMonitor a first-class Skills manager UI + RPCs.
   * Align with **Codex Skills format and config** (folders with `SKILL.md`, enable/disable via `skills/config/write`).
   * Borrow UX concepts from ClawdBot's skills tooling (locations, status, gating).

### Non-goals (for this pass)

* Don't attempt to replicate ClawdHub end-to-end registry distribution (we can do "install by git URL" now; registry browsing later).
* Don't build an Anthropic-only extension workflow.

---

## 1. Current System Reality Check (must match repo)

### Backend topology (already in repo)

* **Daemon**: `src-tauri/src/bin/codex_monitor_daemon.rs`
  * TCP JSON-RPC-ish protocol
  * token auth, built for Tailscale
  * spawns **one Codex `app-server` per workspace**

* **Codex session wrapper**: `src-tauri/src/backend/app_server.rs`
  * supports "background thread callbacks" (used for commit message generation)

* **Memory**: Supabase + MiniMax embeddings
  * `src-tauri/src/memory/service.rs` + `supabase.rs` + `embeddings.rs`
  * Daemon RPC: `memory_status`, `memory_search`, `memory_append`, `memory_bootstrap`, `memory_delete` already exists

* **Skills**: currently just proxying `skills/list` from app-server (`skills_list` in daemon + `src-tauri/src/codex.rs`)

### Key architectural leverage point

You already have a proven pattern for "background Codex work" (commit message generation) using:
* `thread/start` in a background thread
* capture `item/agentMessage/delta`
* stop at `turn/completed`

We will reuse **exactly that** for memory flush summarization.

---

## 2. Feature A: Automatic Memory (Pre-Compaction Flush)

### 2.1 What we're copying from ClawdBot (behavior)

ClawdBot's core insight is:
* When conversation tokens approach context limit, do an **automatic memory flush** first
* The flush runs as a hidden turn with a `NO_REPLY` style instruction
* Only after flushing does compaction happen

We'll implement the same *behavioral contract*, but adapted to:
* **Codex app-server** events for token usage (`thread/tokenUsage/updated` already handled in iOS)
* **Supabase memory** as the storage target (not Markdown files)

### 2.2 Desired UX in CodexMonitor

* Auto memory is **off by default**, toggleable in Settings (desktop + iOS).
* When on:
  * daemon watches token usage
  * when nearing limit → daemon triggers background summarizer → daemon writes 1–2 memory entries:
    * `daily` entry (append-only log style)
    * optional `curated` entry (stable knowledge / long-term facts)
* Show a **toast/log** in Debug panel:
  * "Auto-memory flush triggered (thread X, tokens Y/Z)"
  * "Flush wrote N entries" or "Flush skipped (cooldown)" etc.

### 2.3 Data model / settings changes

#### Rust: extend `AppSettings` (`src-tauri/src/types.rs`)

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMemorySettings {
    pub enabled: bool,

    /// ClawdBot-like "reserve tokens floor" to preserve headroom.
    /// Default 20000 (matches Clawd docs guidance).
    pub reserve_tokens_floor: u32,

    /// Trigger when remaining usable tokens <= soft_threshold_tokens.
    /// Default 4000 (Clawd-style).
    pub soft_threshold_tokens: u32,

    /// Cooldown between flushes per thread.
    pub min_interval_seconds: u32,

    /// How many most-recent turns to include in snapshot.
    pub max_turns: usize,

    /// Hard cap on per-flush snapshot size (chars).
    pub max_snapshot_chars: usize,

    /// If true, include truncated tool outputs (commands, errors).
    pub include_tool_output: bool,

    /// If true, include `git status -sb` + small diffstat.
    pub include_git_status: bool,

    /// Write daily entry
    pub write_daily: bool,

    /// Write curated entry
    pub write_curated: bool,
}
```

Then in `AppSettings`:

```rust
pub auto_memory: AutoMemorySettings
```

Default values should follow Clawd guidance (reserve floor and threshold), but let users tune.

#### Swift: extend iOS `AppSettings` model (`ios/.../CodexMonitorModels/Models.swift`)

Mirror the same shape:

```swift
public struct AutoMemorySettings: Codable, Equatable {
    public var enabled: Bool
    public var reserveTokensFloor: Int
    public var softThresholdTokens: Int
    public var minIntervalSeconds: Int
    public var maxTurns: Int
    public var maxSnapshotChars: Int
    public var includeToolOutput: Bool
    public var includeGitStatus: Bool
    public var writeDaily: Bool
    public var writeCurated: Bool
}
```

Add to `AppSettings`.

#### Daemon persistence

Your daemon persists `settings.json`. No new file required for config.

### 2.4 Runtime state needed for auto-memory

Create a new module:

`src-tauri/src/memory/auto_flush.rs`

Key structs:

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct AutoMemoryRuntime {
    per_thread: HashMap<String /*thread_id*/, ThreadAutoState>,
}

#[derive(Clone, Debug)]
pub struct ThreadAutoState {
    last_flush_at: Option<Instant>,
    last_seen_context_tokens: Option<u32>,
    // detects "compaction happened" by seeing a large drop
    last_compaction_epoch: u64,
    // last flush epoch to avoid double flush in same epoch
    last_flush_epoch: Option<u64>,
}
```

We'll infer "compaction epoch" by detecting a **drop in context tokens** (details below).

### 2.5 Trigger logic (token threshold)

Clawd's model is: keep reserve floor, then trigger flush when remaining <= soft threshold.

We'll implement:

```rust
fn should_flush(
    settings: &AutoMemorySettings,
    context_tokens: u32,
    model_context_window: u32,
) -> bool {
    if model_context_window == 0 { return false; }

    let usable_window = model_context_window.saturating_sub(settings.reserve_tokens_floor);
    // if reserve floor is larger than model window, usable_window becomes 0 -> never flush
    if usable_window == 0 { return false; }

    // trigger when we are within `soft_threshold_tokens` of usable limit
    context_tokens >= usable_window.saturating_sub(settings.soft_threshold_tokens)
}
```

Also enforce:
* cooldown (`min_interval_seconds`)
* don't flush twice in same "compaction epoch"

### 2.6 Detecting compaction without explicit events

CodexMonitor already receives `thread/tokenUsage/updated` and your iOS client treats `total.totalTokens` as "contextTokens".

If compaction occurs, you'll typically see **a large drop** in `contextTokens` (heuristic). So:

```rust
fn detect_compaction_epoch(prev: Option<u32>, now: u32, epoch: u64) -> u64 {
    match prev {
        None => epoch,
        Some(prev_tokens) => {
            // heuristic: context dropped by 30%+ => compaction/reset
            if now + (now / 2) < prev_tokens { epoch + 1 } else { epoch }
        }
    }
}
```

This isn't perfect, but it's practical and doesn't require undocumented events.

### 2.7 Collecting a "Memory Flush Snapshot"

We need the last N turns + optional tool output + optional git status.

#### Snapshot source strategy

Implement **Phase A** using thread history already returned by app-server:

* Call `thread/resume` (or reuse the in-memory ThreadRecord if you already have it)
* Extract last `max_turns` from `thread.turns`
* Extract text from items with type `userMessage` and `agentMessage`

This avoids building a rolling transcript cache first.

Later, Phase B can optimize with a rolling cache.

#### Snapshot shape (internal)

```rust
pub struct MemoryFlushSnapshot {
    pub workspace_id: String,
    pub thread_id: String,
    pub created_at_ms: i64,
    pub model: Option<String>,
    pub context_tokens: u32,
    pub model_context_window: u32,
    pub turns: Vec<SnapshotTurn>,
    pub git_status: Option<String>,
    pub tool_tail: Option<String>,
}

pub struct SnapshotTurn {
    pub role: String, // "user" | "assistant"
    pub text: String,
}
```

#### Git status collection (optional)

* Run `git -C <workspace> status -sb`
* Run `git -C <workspace> diff --stat` (truncate lines)
* Cap total chars

### 2.8 Generating the memory content with Codex (background thread pattern)

We will reuse the exact approach in `generate_commit_message(...)` in `src-tauri/src/codex.rs`.

Create helper in `auto_flush.rs`:

```rust
pub async fn run_memory_flush_summarizer(
    session: &WorkspaceSession,
    snapshot: &MemoryFlushSnapshot,
) -> Result<String, String> {
    // 1) start background thread
    // 2) register background callback channel
    // 3) send turn/start with prompt
    // 4) collect agent text deltas until turn/completed
    // 5) return final assistant text
}
```

#### Prompt format (must be parseable)

Require strict JSON output to avoid flaky parsing:

**System instruction** (in the prompt body, since app-server call doesn't expose separate system role easily):

* "You are writing durable memory notes…"
* "Return JSON ONLY"

**User content** = JSON snapshot + instructions.

Example prompt builder:

```rust
fn build_memory_flush_prompt(snapshot: &MemoryFlushSnapshot) -> String {
    format!(r#"
You are CodexMonitor Auto-Memory.

TASK:
- Extract durable facts, decisions, TODOs, and project state worth remembering.
- Output STRICT JSON ONLY, no markdown, no prose.

OUTPUT JSON SCHEMA:
{{
  "no_reply": boolean,
  "title": string,
  "tags": string[],
  "daily_markdown": string,
  "curated_markdown": string
}}

RULES:
- If nothing worth storing, set no_reply=true and leave other fields empty.
- daily_markdown: short append-only log entry (timestamped), bullet-heavy.
- curated_markdown: stable facts (names, endpoints, commands, gotchas), omit ephemeral chatter.
- Keep each field <= 1500 chars.

SNAPSHOT:
{snapshot_json}
"#,
    snapshot_json = serde_json::to_string_pretty(snapshot).unwrap())
}
```

This is essentially Clawd's "write important context to memory before compaction" concept adapted to your Supabase storage.

#### Parse output

* If invalid JSON → store raw text as daily memory with tag `#auto_memory_parse_error`
* If `no_reply=true` → skip write

### 2.9 Writing to Supabase (existing MemoryService)

Use existing:

```rust
memory.append("daily", daily_markdown, tags, Some(workspace_id)).await?;
memory.append("curated", curated_markdown, tags, Some(workspace_id)).await?;
```

Tagging strategy:
* Always add: `auto_memory`, `workspace:<id>`, `thread:<id>`
* Add user-provided tags

### 2.10 Where to hook the auto-memory trigger (daemon)

#### New daemon background task

In `codex_monitor_daemon.rs` after broadcast channel init:

* Create an `AutoMemoryCoordinator` with:
  * access to `DaemonState.sessions`
  * access to `DaemonState.memory`
  * access to `DaemonState.data_dir`
  * current `AppSettings` (or watch updates)

* Spawn a loop:

```rust
let mut rx = tx.subscribe();
tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        if let DaemonEvent::AppServer(app_evt) = event {
            // parse app_evt.message["method"]
            // if "thread/tokenUsage/updated" => maybe_trigger_flush(...)
        }
    }
});
```

#### Parsing token usage event

The iOS client expects `params.usage`. Mirror that:

```rust
if method == "thread/tokenUsage/updated" {
    let thread_id = params["threadId"].as_str().unwrap_or_default();
    let usage = &params["usage"];
    let context_tokens = usage["total"]["totalTokens"].as_u64().unwrap_or(0) as u32;
    let model_context_window = usage["modelContextWindow"].as_u64().unwrap_or(0) as u32;
    // maybe trigger
}
```

### 2.11 Manual controls

Add daemon RPC:

* `memory_flush_now`
  Params: `{ workspace_id, thread_id, reason?: string }`
  Behavior: bypass threshold check, but still respects cooldown unless `force=true`.

Expose in iOS and Desktop UI:

* Add "Flush to Memory" button in conversation view (or in Memory tab as "Capture current thread").

### 2.12 AppSettings live reload fix (important)

Right now `update_app_settings` updates `settings.json` but does not rebuild the `MemoryService` in daemon.

Fix: move memory service into a `RwLock<Option<MemoryService>>` and rebuild on update.

This matters because auto-memory depends on memory being enabled/disabled live.

---

## 3. Feature B: Browser Control (Codex-Compatible)

### 3.1 What we're copying from ClawdBot (architecture + security)

Clawd's browser tool concept:
* local "control server"
* browser automation via DevTools/CDP/Playwright
* strong warning: treat it like admin API; don't expose publicly; use localhost + Tailscale/SSH tunnels; token protect

We'll implement browser control as **daemon RPC + an MCP server**:
* RPC for iOS/desktop UI
* MCP for Codex tool use

### 3.2 Implementation choice: Node Playwright Worker (recommended)

Because:
* Playwright is robust and supports screenshots, selectors, network stability.
* Rust-only CDP stacks are doable but take longer to harden.

#### New folder

`browser-worker/`
* `package.json` (separate from frontend to avoid bundling Playwright into React build)
* `src/index.ts`
* build step to output `dist/index.js`

#### Wire into daemon

Daemon spawns `node browser-worker/dist/index.js` and speaks newline-delimited JSON over stdin/stdout.

### 3.3 Worker protocol

**Request**

```json
{ "id": "req-123", "method": "browser.create", "params": { "headless": true, "userDataDir": "..." } }
```

**Response**

```json
{ "id": "req-123", "result": { "sessionId": "b-abc123" } }
```

#### Required methods

* `browser.create`
  * params: headless, viewport, userDataDir (persist cookies), startUrl optional
* `browser.list`
* `browser.close`
* `browser.navigate`
  * params: sessionId, url, waitUntil ("load"|"domcontentloaded"|"networkidle"), timeoutMs
* `browser.screenshot`
  * params: sessionId, fullPage, type ("png"), quality optional
  * returns: `{ base64Png, url, title, width, height }`
* `browser.click`
  * params: sessionId, selector OR x/y coords
* `browser.type`
  * params: sessionId, selector, text, clearFirst
* `browser.press`
  * params: sessionId, key ("Enter", "Tab")
* `browser.evaluate`
  * params: sessionId, js (string)
* `browser.snapshot`
  * returns: screenshot + simplified DOM text (for model consumption)
  * include:
    * url/title
    * "interactive elements list" using `page.$eval('a,button,input,...')`
    * accessibility snapshot (optional)

### 3.4 Daemon BrowserService (Rust)

Create:
`src-tauri/src/browser/service.rs`

Responsibilities:
* spawn worker process on first use (or at daemon start if enabled)
* maintain map `sessionId -> metadata`
* route RPC requests to worker with timeouts
* restart worker if it crashes

Pseudo-code:

```rust
pub struct BrowserService {
    worker: WorkerClient,
}

impl BrowserService {
    pub async fn create_session(&self, params: CreateParams) -> Result<CreateResult, String>;
    pub async fn screenshot(&self, session_id: &str, full_page: bool) -> Result<ScreenshotResult, String>;
    // etc
}
```

WorkerClient uses:
* `tokio::process::Command`
* `ChildStdin` write lines
* `ChildStdout` read lines
* correlate by request id

### 3.5 Daemon RPC surface (for iOS + desktop)

Add methods to `handle_request`:
* `browser_create_session`
* `browser_list_sessions`
* `browser_close_session`
* `browser_navigate`
* `browser_screenshot`
* `browser_click`
* `browser_type`
* `browser_press`
* `browser_snapshot`

Add corresponding Swift API methods in `CodexMonitorAPI.swift` and models.

### 3.6 iOS UI (BrowserView)

Add new view:
`ios/CodexMonitorMobile/CodexMonitorMobile/Views/BrowserView.swift`

Features:
* list sessions
* create session (headless toggle)
* URL bar (navigate)
* screenshot preview (pull-to-refresh or auto-refresh every N seconds while open)
* tap-to-click:
  * translate tap coords to page coords (simple: assume screenshot dims == viewport dims; if not, scale)
  * call `browser_click` with x,y

Integrate into `RootView`:
* add `.browser` case to enum
* add tab item with SF Symbol `safari` or `globe`

Match your iOS 26 "glass" style: use `.glassEffect()` like other panels.

### 3.7 Codex tool access: Browser MCP server

#### Why MCP?

Your Codex app-server already supports tool calls and MCP (you already ship `codex_monitor_memory_mcp`). So we add `codex_monitor_browser_mcp` as another MCP server.

#### Implementation: Rust binary (recommended)

Add:
`src-tauri/src/bin/codex_monitor_browser_mcp.rs`

It should:
* expose MCP tools mirroring RPC methods (`browser_navigate`, `browser_screenshot`, etc.)
* internally call daemon RPC on `127.0.0.1:<port>` with token
  * or call BrowserService directly if running in same process (but MCP server is separate process, so RPC is simpler)

#### Config instructions

Document adding to `~/.codex/config.toml` similarly to memory MCP. (Same pattern you used in `ARCHITECTURE.md` for memory MCP.)

### 3.8 Security

Follow Clawd guidance:
* bind daemon to localhost by default
* use token auth always
* if exposing via Tailscale, keep token strong
* do not expose browser control endpoints unauthenticated

---

## 4. Feature C: Skill Management (Codex Skills + Clawd UX)

### 4.1 Align with Codex skills format and locations

Codex skills:
* live in folders containing `SKILL.md`
* discovered in `~/.codex/skills` and workspace skill dirs (`./codex/skills`, `./.codex/skills`)
* enabled/disabled via app-server method `skills/config/write`

ClawdBot skills UX:
* list installed
* enable/disable
* gating / config awareness
* "extraDirs" and watch-like behavior

### 4.2 Backend: add skill config RPC wrappers

Right now you only have:
* `skills_list` → `skills/list`

Add daemon RPC:
* `skills_config_write`
  * params: `{ workspaceId, enabled: [{name, path}], disabled: [{name, path}] }`
  * call app-server: `"skills/config/write"`

Optionally also add:
* `skills_rescan` (if Codex app-server supports reload; if not, just call `skills/list` again)

### 4.3 Skill metadata parsing (for gating & better UI)

Implement a parser that reads `SKILL.md` YAML frontmatter to extract:
* name, description
* optional metadata for requirements (bins/env/os)

Even if Codex doesn't require these fields, CodexMonitor UI can interpret them.

Add to daemon:
`src-tauri/src/skills/skill_md.rs`

* parse frontmatter YAML using `serde_yaml`
* return:

```rust
pub struct SkillDescriptor {
  pub name: String,
  pub description: Option<String>,
  pub path: String,
  pub requirements: Requirements,
}

pub struct Requirements {
  pub bins: Vec<String>,
  pub env: Vec<String>,
  pub os: Vec<String>,
}
```

### 4.4 Gating evaluation

Daemon method:
* `skills_validate`
  * returns per-skill issues:
    * missing binary
    * missing env var
    * unsupported OS

This is borrowed from Clawd's skills config approach, but implemented in CodexMonitor UX.

### 4.5 Skill installation (minimal but useful)

Implement:

* `skills_install_from_git`
  * params: `{ sourceUrl, target: "global"|"workspace", workspaceId? }`
  * behavior:
    * resolve destination:
      * global: `~/.codex/skills/<repo-name>/`
      * workspace: `<workspace>/.codex/skills/<repo-name>/`
    * do `git clone` (shell out `git` or use `git2`)
    * validate it contains `SKILL.md`
    * return updated skills list

* `skills_uninstall`
  * delete directory (careful: only inside allowed skill roots)

### 4.6 UI: Skills view in iOS + Desktop

#### iOS

Add `SkillsView.swift`:
* list skills (from `skills_list`)
* show enabled/disabled toggle
* show "missing requirements" warnings (from `skills_validate`)
* install by URL (text field)

Integrate into RootView:
* add `.skills` in detail views
* on iPhone, you may prefer putting Skills inside Projects tab (to avoid too many tab icons), but for simplicity add a tab.

#### Desktop

Add a new panel/tab under workspace:
* list skills
* enable/disable
* refresh button
* install by URL

Bonus: integrate with existing composer autocomplete:
* you already do `$<skill>` suggestions; keep it, but include descriptions.

---

## 5. Tests (must be included)

### 5.1 Rust unit tests

Create `src-tauri/src/memory/auto_flush_tests.rs`:
* `test_should_flush_threshold_basic`
* `test_should_flush_reserve_floor_blocks`
* `test_compaction_epoch_detection`
* `test_prompt_builder_char_caps`

Create `src-tauri/src/skills/skill_md_tests.rs`:
* parse valid SKILL.md frontmatter
* handles missing fields
* handles malformed YAML

### 5.2 Daemon integration tests (Rust)

Add a test that runs daemon in-process on ephemeral port:
* calls `update_app_settings` to enable auto memory
* simulates app-server token usage event by directly calling coordinator method (don't require real app-server in CI)
* asserts coordinator calls MemoryService.append with expected values (mock MemoryService)

This requires making MemoryService injectable behind a trait:

```rust
#[async_trait]
pub trait MemoryWriter {
  async fn append(&self, ... ) -> Result<..., String>;
}
```

Then MemoryService implements it, and tests use MockMemoryWriter.

### 5.3 Browser worker tests (Node)

Add minimal protocol tests:
* create session
* navigate to example.com
* screenshot returns base64
* close session

If CI can't run browsers, mark as "manual/local" tests but keep them runnable.

---

## 6. Phased Delivery Plan (don't mix phases)

### Phase 1 — Auto Memory MVP (background summarizer → Supabase)

* Add settings fields (Rust + Swift)
* Fix daemon memory service live reload on settings update
* Implement AutoMemoryCoordinator:
  * listen to daemon broadcast events
  * parse `thread/tokenUsage/updated`
  * threshold + cooldown + epoch detection
  * call background summarizer (reuse commit message generation pattern)
  * write to Supabase
* Add `memory_flush_now` RPC
* Add iOS settings toggles + "flush now" button

**Acceptance**
* Trigger by artificially lowering `reserveTokensFloor` and sending messages
* Confirm memory entries appear in iOS Memory tab

### Phase 2 — Browser daemon service (RPC only)

* Build Playwright worker + daemon BrowserService
* Add daemon RPC methods
* Add iOS BrowserView: create session, navigate, screenshot, click by tap

**Acceptance**
* From iPhone on Tailscale: open session, navigate, see screenshot update, click button

### Phase 3 — Browser MCP server (Codex tool)

* Implement `codex_monitor_browser_mcp`
* Document `~/.codex/config.toml` addition
* Verify Codex can call `browser_screenshot` etc in a thread

**Acceptance**
* In a Codex thread: ask it to open a page and extract something using browser tools

### Phase 4 — Skills management enhancements

* Add RPC wrapper for `skills/config/write`
* Implement SKILL.md parser + validate
* Add install/uninstall
* Add Skills UI in iOS + Desktop

**Acceptance**
* Install a test skill from git URL
* Enable/disable works and persists

### Phase 5 — Polish & Docs

* Update `docs/API_REFERENCE.md` to include memory + browser + skills_config methods
* Update `docs/ARCHITECTURE.md` with:
  * AutoMemoryCoordinator diagram
  * BrowserService diagram
  * Skill management flow
* Update `docs/DEPLOYMENT.md` with Playwright worker + MCP server install steps

---

## 7. Files To Create/Modify

### New Files

```
src-tauri/src/memory/auto_flush.rs           # Auto-memory coordinator
src-tauri/src/browser/mod.rs                 # Browser module
src-tauri/src/browser/service.rs             # Browser service (Rust)
src-tauri/src/skills/skill_md.rs             # SKILL.md parser
src-tauri/src/bin/codex_monitor_browser_mcp.rs

browser-worker/                              # Playwright Node worker
├── package.json
├── src/index.ts
└── tsconfig.json

ios/.../Views/BrowserView.swift
ios/.../Views/SkillsView.swift
```

### Modified Files

```
src-tauri/src/types.rs                       # AutoMemorySettings
src-tauri/src/bin/codex_monitor_daemon.rs    # Hook coordinator
src-tauri/src/memory/mod.rs                  # Export auto_flush
ios/.../Models.swift                         # Swift settings
ios/.../RootView.swift                       # New tabs
docs/ARCHITECTURE.md
docs/API_REFERENCE.md
docs/DEPLOYMENT.md
```

---

## 8. Notes

### SKILL.toml vs SKILL.md

The attached ClawdBot analysis doc references `SKILL.toml`, but both ClawdBot's current docs and Codex's skills system use `SKILL.md` with YAML frontmatter. Implement **SKILL.md** everywhere to stay compatible with Codex.

### References

* [ClawdBot Memory Docs](https://docs.clawd.bot/concepts/memory)
* [ClawdBot Browser Docs](https://docs.clawd.bot/tools/browser)
* [ClawdBot Skills Config](https://docs.clawd.bot/tools/skills-config)
* [Codex Agent Skills](https://developers.openai.com/codex/skills/)
* [Codex App Server](https://developers.openai.com/codex/app-server)

---

*Generated by ChatGPT 5.2 Pro - 2026-01-26*
