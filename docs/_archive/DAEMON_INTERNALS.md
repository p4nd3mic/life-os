# Daemon Internals (Rust)

This document describes the **CodexMonitor daemon** implementation and the shared Rust backend modules it reuses.

The daemon is implemented as an extra Rust binary in the Tauri crate:

- `src-tauri/src/bin/codex_monitor_daemon.rs`

It reuses the same core backend modules as the desktop app via `#[path = "../..."] mod ...` imports.

---

## Entry point and lifecycle

### Binary entry

File: `src-tauri/src/bin/codex_monitor_daemon.rs`

Main steps:

1. Parse CLI args (`Args`):
   - `--port` (default `4732`)
   - `--bind` (default `127.0.0.1`)
   - `--data-dir` (optional)
   - `--token` or env `CODEX_MONITOR_DAEMON_TOKEN`
   - `--insecure-no-auth` (dev-only)

2. Resolve data directory:
   - If `--data-dir` specified: use it
   - Else: Linux-style default (`$XDG_DATA_HOME/codex-monitor-daemon` or `~/.local/share/codex-monitor-daemon`)
   - **Note:** production deployment on macOS uses an explicit `--data-dir` (see `docs/DEPLOYMENT.md`).

3. Load persistent state:
   - `workspaces.json` → `HashMap<WorkspaceId, WorkspaceEntry>`
   - `settings.json` → `AppSettings`

4. Start TCP listener:
   - Accept connections
   - Spawn `handle_client(...)` per connection

---

## High-level architecture

### Core state container: `DaemonState`

`DaemonState` owns:

- `workspaces: Mutex<HashMap<String, WorkspaceEntry>>`
- `app_settings: Mutex<AppSettings>`
- `sessions: Mutex<HashMap<String, WorkspaceSession>>`  
  (one live `codex app-server` session per connected workspace)
- `terminal_sessions: Mutex<HashMap<String, TerminalSession>>`
- `data_dir: PathBuf`
- `event_sink: DaemonEventSink` → broadcasts `DaemonEvent` to clients

Persistent files:
- `<data-dir>/workspaces.json`
- `<data-dir>/settings.json`

Runtime-only (in-memory):
- workspace `sessions`
- terminal sessions

### Client connections: `handle_client`

Each client connection:

- Reads newline-delimited JSON messages (`BufRead::lines()`).
- If auth is enabled:
  - requires the first successful method to be `auth`
  - otherwise returns `{"error":{"message":"unauthorized"}}`
- After auth, it subscribes to the daemon event broadcast and starts forwarding:
  - `DaemonEvent::AppServerEvent` → notification `app-server-event`
  - `DaemonEvent::TerminalOutput` → notification `terminal-output`

---

## Module structure (shared + daemon-only)

The daemon binary includes these shared modules:

| Module | Source file | Purpose |
|---|---|---|
| `backend` | `src-tauri/src/backend/mod.rs` | Shared backend building blocks |
| `codex_config` | `src-tauri/src/codex_config.rs` | Read/write `$CODEX_HOME/config.toml` feature flags |
| `codex_home` | `src-tauri/src/codex_home.rs` | Resolve CODEX_HOME / legacy `.codexmonitor` |
| `git_utils` | `src-tauri/src/git_utils.rs` | Helper utilities (find repo root, etc.) |
| `local_usage_core` | `src-tauri/src/local_usage_core.rs` | Parse Codex session JSONL logs for usage stats |
| `rules` | `src-tauri/src/rules.rs` | Read/write `$CODEX_HOME/rules/default.rules` |
| `storage` | `src-tauri/src/storage.rs` | Read/write JSON state files |
| `types` | `src-tauri/src/types.rs` | Shared serializable structs/enums |
| `utils` | `src-tauri/src/utils.rs` | Misc helpers |

And it defines significant daemon-only code inside the binary file itself:
- TCP server
- RPC dispatcher (`handle_rpc_request`)
- Git operations (libgit2 + some CLI)
- Workspace management (clone/worktree management, cleanup)
- Terminal session management glue

---

## Codex CLI integration

### Process model: one `codex app-server` per workspace

The core integration is implemented in:

- `src-tauri/src/backend/app_server.rs`

Key concepts:
- A `WorkspaceSession` represents a running app-server process tied to:
  - a `cwd` (workspace root)
  - a `codex_bin` path (optional override)

Spawn behavior:
- The backend spawns a process roughly like:

```sh
codex app-server
```

with:
- `stdin` used for requests/responses
- `stdout` emitting newline-delimited JSON notifications/events
- `stderr` piped for logging/debug

### Sending requests

The app-server protocol is treated as:
- request: `{ "id": <number>, "method": "<string>", "params": <object> }`
- response: `{ "id": <same>, "result": <any> }` or `{ "id": <same>, "error": ... }`
- notification: `{ "method": "<string>", "params": <any> }`

In the daemon:
- the daemon RPC methods translate into app-server requests (e.g. `turn/start`).
- the initial app-server response is returned to the client call.
- subsequent updates arrive as notifications and are broadcast.

### Broadcasting events to clients

The shared backend uses an `EventSink` abstraction (see `src-tauri/src/backend/events.rs`):

- In the **desktop app**, the sink emits Tauri events.
- In the **daemon**, the sink broadcasts to TCP clients.

In the daemon binary:
- `DaemonEventSink` implements `EventSink`.
- It pushes `DaemonEvent::AppServerEvent { workspace_id, message }` into a `tokio::sync::broadcast::Sender`.

---

## Event system details

### What is an AppServerEvent?

`AppServerEvent` is a thin envelope:

- `workspace_id: String`
- `message: serde_json::Value` (opaque)

Daemon clients receive it as notification:
- method: `"app-server-event"`
- params: `AppServerEvent`

The `message` payload is the raw Codex event (examples):
- `item/agentMessage/delta`
- `item/completed`
- `turn/plan/updated`
- `thread/tokenUsage/updated`
- `account/rateLimits/updated`
- `requestApproval/*`

### Client-side parsing responsibility

Neither the daemon nor the Rust backend tries to interpret event semantics.
Interpretation is done in the clients:
- Desktop: reducers/hooks under `src/features/threads/`
- iOS: `CodexStore` + `ConversationHelpers`

---

## Terminal multiplexing

### PTY implementation

Terminal sessions use `portable_pty` (see `src-tauri/src/terminal.rs` in desktop backend).

On the daemon:
- `terminal_open` spawns a PTY shell process:
  - cwd = workspace path
  - shell = `$SHELL` if set, otherwise a default
- Output is read asynchronously and broadcast as `terminal-output` notifications.

### Session identity

The *client* chooses `terminalId` in `terminal_open`.
The daemon stores sessions in:

- `terminal_sessions: HashMap<String, TerminalSession>`

All subsequent calls (`terminal_write`, `terminal_resize`, `terminal_close`) use this `terminalId`.

### Output flooding risk

Terminal output is streamed as unbounded strings. A noisy command can:
- generate a high volume of events
- pressure memory / CPU in clients

Mitigations currently rely on:
- client-side trimming of buffers
- user discipline (no explicit server-side backpressure)

---

## Git operations

### Libraries used

The daemon uses:
- `git2` (libgit2 bindings) for many operations:
  - status
  - log traversal
  - diffs
  - committing
  - branch enumeration
- Some operations may invoke git CLI depending on implementation (worktree creation, clone).

### Workspace root vs git root

Most methods take a `workspaceId`. The daemon resolves:
- workspace root directory
- git root (by walking up from workspace root)

Some workspaces can contain multiple nested repos (monorepos); see:
- RPC: `list_git_roots`

### Security / safety

Git methods operate directly on repositories on the daemon host.
Combined with token auth, they are effectively “remote git control”.

---

## Settings and configuration

### Persistent settings (`settings.json`)

The daemon persists a full `AppSettings` blob:
- backendMode (local/remote)
- codex binary override
- UI preferences (theme, scale, etc.)
- experimental feature toggles (collab/steer/unified_exec)
- workspace grouping preferences

**macOS app data path (desktop build):**
```
~/Library/Application Support/com.codexmonitor.app/settings.json
```

### Codex feature flags (`$CODEX_HOME/config.toml`)

`get_app_settings` overlays feature flags from Codex config:
- `features.collab`
- `features.steer`
- `features.unified_exec`

`update_app_settings` writes them back.

### Approval rules (`$CODEX_HOME/rules/default.rules`)

When a user accepts an approval “remember” action:
- daemon `remember_approval_rule` calls `rules::append_prefix_rule(...)`

This changes what Codex can execute without prompting and is security-sensitive.

---

## Security and risk notes

- **Plain TCP; no TLS**  
  The daemon assumes you protect the port using localhost binding + Tailscale.

- **Token == full remote control**  
  With a valid token, a client can:
  - spawn shells (terminal)
  - run git operations
  - read workspace files
  - instruct Codex to run commands depending on access mode and approval rules

- **No per-client authorization**  
  There is only one token for all clients; no user identities.

- **`--insecure-no-auth` caveat**  
  In no-auth mode, `auth` is not implemented. Desktop remote mode can omit auth if token is empty, but the iOS client always performs auth and will fail to connect.

---

## Memory System

### MemoryService

- **Path**: `src-tauri/src/memory/service.rs`
- Provides CRUD and semantic search against Supabase + pgvector
- Uses MiniMax embeddings API for vector generation

### Auto-Memory (Pre-compaction Flush)

- **Path**: `src-tauri/src/memory/auto_flush.rs`

The auto-memory system monitors token usage and triggers background summarization before the model's context window fills.

#### AutoMemoryRuntime

Maintains per-thread state:

```rust
struct ThreadAutoState {
    last_flush_at: Option<Instant>,        // Rate limiting
    last_seen_context_tokens: Option<u32>, // Previous token count
    last_compaction_epoch: u64,            // Increments on compaction detection
    last_flush_epoch: Option<u64>,         // Prevents duplicate flushes
}
```

#### Flush Trigger Logic

```rust
fn should_flush(settings, context_tokens, model_context_window) -> bool {
    let usable_window = model_context_window - reserve_tokens_floor;
    context_tokens >= usable_window - soft_threshold_tokens
}
```

Compaction detection: if `new_tokens + (new_tokens / 2) < previous_tokens`, a compaction occurred.

#### Snapshot Builder

When a flush triggers:
1. Call `thread/resume` to fetch recent turns
2. Extract user/assistant messages (optionally tool output)
3. Collect `git status -sb` + `git diff --stat` if configured
4. Build `MemoryFlushSnapshot` struct

#### Background Summarizer

Spawns an ephemeral Codex thread with a structured prompt:
- Input: `MemoryFlushSnapshot` as JSON
- Output: JSON with `{ no_reply, title, tags, daily_markdown, curated_markdown }`
- Timeout: 60 seconds
- Thread is archived after completion

The summarizer uses a "commit message" pattern - extract durable facts, decisions, TODOs, and project state worth remembering.

---

## Browser Control System

### BrowserService

- **Path**: `src-tauri/src/browser/service.rs`
- Lazy-spawns browser-worker on first request
- Maintains persistent stdio connection
- Request/response matching by JSON-RPC `id`

```rust
pub struct BrowserService {
    worker: Arc<Mutex<Option<BrowserWorkerClient>>>,
}
```

Environment variable:
- `CODEX_MONITOR_BROWSER_WORKER` - path to worker script (default: `browser-worker/dist/index.js`)

### Browser Worker (Node.js + Playwright)

- **Path**: `browser-worker/src/index.ts`
- Dependencies: `playwright` (Chromium)
- Session management: `Map<sessionId, Session>`

Supported methods:
| Method | Description |
|--------|-------------|
| `browser.create` | Create new session with optional headless, viewport, userDataDir, startUrl |
| `browser.list` | List active session IDs |
| `browser.close` | Close session |
| `browser.navigate` | Navigate to URL with waitUntil and timeout options |
| `browser.screenshot` | Capture PNG screenshot (fullPage option) |
| `browser.click` | Click by selector or x/y coordinates |
| `browser.type` | Type into element (with clearFirst option) |
| `browser.press` | Press keyboard key |
| `browser.evaluate` | Execute JavaScript in page context |
| `browser.snapshot` | Screenshot + DOM element list (first 50 interactive elements) |

### Browser MCP Server

- **Binary**: `src-tauri/src/bin/codex_monitor_browser_mcp.rs`
- Exposes browser tools to Codex CLI via MCP protocol
- Connects to daemon over TCP, proxies browser RPC calls

---

## Skills Management System

### SKILL.md Parser

- **Path**: `src-tauri/src/skills/skill_md.rs`
- Parses YAML frontmatter + markdown body

```rust
pub struct SkillDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub path: String,
    pub requirements: Requirements,
}

pub struct Requirements {
    pub bins: Vec<String>,  // Required CLI binaries
    pub env: Vec<String>,   // Required environment variables
    pub os: Vec<String>,    // Supported operating systems
}
```

#### Parsing Logic

1. Split frontmatter from body (delimited by `---`)
2. Parse YAML frontmatter for `name`, `description`, `requirements`
3. Fallback name: parent directory name
4. Fallback description: first non-empty line of body

#### Validation

The `validate_skill()` function checks:
1. **OS compatibility**: current OS in `requirements.os` list
2. **Binary availability**: all `requirements.bins` found via `which`
3. **Environment variables**: all `requirements.env` are set

Returns a list of issues (empty = valid).

### Skills Configuration

- **Config path**: `{CODEX_HOME}/skills/config.json`
- Stores enabled/disabled skill entries

```json
{
  "enabled": [{ "name": "skill-name", "path": "/path/to/skill" }],
  "disabled": [{ "name": "other-skill", "path": "/path/to/other" }]
}
```

---

## File inventory (Rust backend)

Top-level Tauri crate (`src-tauri/src/*.rs`):

- `lib.rs` — command registration + app bootstrap
- `main.rs` — desktop binary entry
- `state.rs` — `AppState` + shared state orchestration
- `storage.rs` — JSON file persistence helpers
- `types.rs` — shared serializable models
- `remote_backend.rs` — desktop remote backend TCP client
- `codex.rs` — high-level Codex request builders
- `terminal.rs` — PTY sessions + event emission
- `workspaces.rs` — workspace CRUD, file listing/reading
- `git.rs`, `git_utils.rs` — git operations + helpers
- `github.rs` — GitHub operations via `gh` CLI
- `prompts.rs` — prompt library CRUD
- `rules.rs` — approval rules file handling
- `local_usage_core.rs` — usage scanning
- `dictation/*` — desktop dictation support (not used by daemon)

Backend folder (`src-tauri/src/backend/*.rs`):
- `app_server.rs` — spawn/manage Codex app-server process
- `events.rs` — event types and sink abstraction

Memory folder (`src-tauri/src/memory/*.rs`):
- `mod.rs` — module exports
- `service.rs` — MemoryService (Supabase + embedding)
- `embeddings.rs` — MiniMax embedding client
- `auto_flush.rs` — AutoMemoryRuntime + snapshot/summarizer logic

Browser folder (`src-tauri/src/browser/*.rs`):
- `mod.rs` — module exports
- `service.rs` — BrowserService (worker management)

Skills folder (`src-tauri/src/skills/*.rs`):
- `mod.rs` — module exports
- `skill_md.rs` — SKILL.md parser + validator

Binaries (`src-tauri/src/bin/*.rs`):
- `codex_monitor_daemon.rs` — headless daemon server
- `codex_monitor_memory_mcp.rs` — MCP server for memory tools
- `codex_monitor_browser_mcp.rs` — MCP server for browser tools
