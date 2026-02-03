# CodexMonitor Mobile Plan (ChatGPT 5.2 Pro)

The following plan was provided by JMWillis and is preserved verbatim for implementation reference.

---

You are implementing a native iOS/iPadOS client for the existing open-source Mac app “CodexMonitor” (Tauri + React + Rust) that wraps Codex CLI (codex app-server). The iOS app must replicate CodexMonitor functionality while running all Codex/Git operations on an always-on Mac mini server reachable over Tailscale.

REPO CONTEXT (must read/understand before edits)
- Frontend (desktop UI): /src (React/TS, feature-sliced)
- Tauri backend (Rust commands): /src-tauri/src/lib.rs lists the full command surface
- Codex app-server integration: /src-tauri/src/backend/app_server.rs
- Remote backend daemon (TCP JSON lines): /src-tauri/src/bin/codex_monitor_daemon.rs
- Remote client in desktop app: /src-tauri/src/remote_backend.rs (shows protocol expectations)
- Workspaces persisted in JSON: workspaces.json + settings.json under Tauri app_data_dir
- Liquid glass desktop style: tauri_plugin_liquid_glass (macOS)

GOAL
Build a UNIVERSAL native iOS/iPadOS app (“CodexMonitor Mobile”) that provides feature parity with the desktop CodexMonitor UI, but operates as a remote client:
- iOS app connects via Tailscale to the Mac mini running a headless “CodexMonitor Daemon Server”
- All Codex CLI, filesystem, git, GitHub, prompts, terminal operations execute on the Mac mini
- iOS app is native SwiftUI + UIKit where needed (no webview UI)
- The iOS UI should match the “liquid glass” aesthetic using iOS materials, blur, vibrancy, translucency, and smooth animations.

HARD REQUIREMENTS
1) Full functionality parity with CodexMonitor (as much as makes sense on iOS):
   - Workspaces (add/remove/connect, groups, sort order, per-workspace codex bin override)
   - Worktrees and clones (add_worktree/remove_worktree/rename_worktree/rename_worktree_upstream/apply_worktree_changes/add_clone)
   - Threads: list/resume/start/archive, stop/interrupt turns, live streaming updates
   - Approvals UI: show request, approve/deny, remember approval rules
   - Conversation view: markdown, tool calls, diffs, review mode, streaming deltas
   - Git panel: status, diffs, stage/unstage/revert/commit/push/pull/sync, branches
   - GitHub panel: issues, PRs, PR diffs, PR comments (via server)
   - Prompt library: list/create/update/delete/move; workspace + global scopes
   - File browser: list files respecting ignore rules, read file (with truncation indicator)
   - Terminal: interactive terminal sessions per workspace (server PTY, client emulator)
   - Model list, skills list, account rate limits
   - Local usage snapshot charts
   - Dictation: must exist on iOS, native-feeling (see dictation section below)
2) Network/security:
   - Operate over Tailscale; assume the user runs Tailscale on iOS and Mac mini
   - Use daemon token auth (already exists) stored in iOS Keychain
   - Prefer binding daemon to localhost and using `tailscale serve` to expose it to tailnet
3) Performance:
   - Smooth scrolling on long conversations
   - Efficient incremental rendering of streaming deltas
   - Avoid huge memory spikes on large diffs/logs
4) Codebase hygiene:
   - Do not break existing desktop app
   - If you refactor shared Rust logic, keep desktop behavior identical
   - Add tests for protocol decoding and key server methods

HIGH-LEVEL ARCHITECTURE YOU MUST IMPLEMENT

A) MAC MINI: “CodexMonitor Daemon Server” (Rust)
- Start from existing `src-tauri/src/bin/codex_monitor_daemon.rs`
- Extend it to implement the ENTIRE command surface of `src-tauri/src/lib.rs` (minus purely desktop-menu stuff), so the iOS app can reach feature parity.

Transport/protocol:
- Keep the existing newline-delimited JSON messages:
  - request:  {id, method, params}
  - response: {id, result} or {id, error:{message}}
  - notification: {method:"app-server-event", params:{workspace_id, message}} etc
- Keep `auth` method as first call (token)
- Add:
  1) `server_info` method returning:
     - serverVersion (daemon build version), protocolVersion integer, capabilities array
     - os, hostname, dataDir, codexBin default, etc
  2) `capabilities` field should enumerate which optional features are enabled
     (terminal, github, local_usage, etc). iOS can hide UI if missing.
- Add a periodic `ping` support (already exists) and advise iOS to keepalive.

Implement missing RPC methods
Compare against `src-tauri/src/lib.rs` and add equivalents to daemon dispatch:
- settings:
  - get_app_settings (exists), update_app_settings (exists)
- workspaces:
  - list_workspaces (exists)
  - is_workspace_path_dir (exists)
  - add_workspace (exists)
  - add_clone (MISSING) -> copy implementation from workspaces.rs, adapt to daemon state
  - add_worktree (exists)
  - remove_workspace (exists)
  - remove_worktree (exists)
  - rename_worktree (exists)
  - rename_worktree_upstream (exists)
  - apply_worktree_changes (MISSING) -> copy implementation from workspaces.rs
  - update_workspace_settings (exists)
  - update_workspace_codex_bin (exists)
  - connect_workspace (exists)
  - list_workspace_files (exists)
  - read_workspace_file (exists)
  - open_workspace_in (MISSING) -> run `open -a <app> <path>` on server macOS
- codex:
  - codex_doctor (MISSING) -> copy from codex.rs but adapt: no tauri State/AppHandle
  - start_thread/resume_thread/list_threads/archive_thread/send_user_message/turn_interrupt/start_review/respond_to_server_request/remember_approval_rule (mostly exist)
  - get_commit_message_prompt (MISSING)
  - generate_commit_message (MISSING) -> requires git diff + background thread pattern
  - model_list/account_rate_limits/skills_list/collaboration_mode_list (exist)
- git (ALL MISSING in daemon):
  - get_git_status
  - list_git_roots
  - get_git_diffs
  - get_git_log
  - get_git_commit_diff
  - get_git_remote
  - stage_git_file, stage_git_all, unstage_git_file
  - revert_git_file, revert_git_all
  - commit_git, push_git, pull_git, sync_git
  - list_git_branches, checkout_git_branch, create_git_branch
  - github:
    - get_github_issues
    - get_github_pull_requests
    - get_github_pull_request_diff
    - get_github_pull_request_comments
- prompts (ALL MISSING in daemon):
  - prompts_list/create/update/delete/move
  - prompts_workspace_dir
  - prompts_global_dir
- terminal (MISSING in daemon):
  - terminal_open/write/resize/close
  - and must emit terminal output notifications (method:"terminal-output")
- local_usage_snapshot (MISSING in daemon):
  - port `src-tauri/src/local_usage.rs` logic to daemon method

Rust refactor guidance (recommended, but optional if you can do it cleanly):
- Many existing modules are tightly coupled to `tauri::State` / `AppHandle` and `TauriEventSink`.
- Introduce a new internal “core server state” struct (non-tauri) and move reusable logic into it.
- Keep the desktop Tauri commands as thin wrappers calling the refactored core functions.
- For terminal: refactor `terminal.rs` so it can emit via a generic EventSink (daemon already has DaemonEventSink).
  - Replace the hard dependency on `TauriEventSink` inside terminal_open; instead, take EventSink from state.
- For codex sessions: daemon already uses `backend::app_server::spawn_workspace_session` with an EventSink. Ensure parity with desktop session spawn behavior (codex_home resolution, approvals, collaborationMode).
- Ensure storage behavior matches desktop:
  - workspaces.json and settings.json format identical
  - When daemon runs on macOS, default data dir should be the SAME as the Tauri app data dir for identifier `com.dimillian.codexmonitor` (~/Library/Application Support/com.dimillian.codexmonitor/).
  - Keep `--data-dir` override, but make the default macOS-friendly.

Daemon “always-on” operation on Mac mini:
- Provide a `launchd` plist example in repo (e.g. `extras/launchd/com.codexmonitor.daemon.plist`) that:
  - runs on login
  - sets CODEX_MONITOR_DAEMON_TOKEN
  - binds 127.0.0.1:4732
  - sets data-dir to the shared data directory
- Provide a Tailscale command snippet:
  - `tailscale serve tcp 4732 tcp://127.0.0.1:4732`
  - (optional) `tailscale funnel off` (do NOT expose publicly)
- Document that MagicDNS hostname can be used from iOS (e.g. macmini.tailnet-xyz.ts.net)

B) iOS/iPadOS APP: “CodexMonitor Mobile” (Swift)
Create a new Xcode project inside repo:
- Path: /ios/CodexMonitorMobile/
- App target: iOS 17+ (universal: iPhone + iPad)
- Tech: SwiftUI for layout + UIKit wrappers where needed for:
  1) a high-performance composer text editor (UITextView/TextKit)
  2) terminal emulator (SwiftTerm preferred; if not, a custom lightweight terminal view)
- No web-based UI. No React Native. No WKWebView for the main UI.

iOS Networking Layer (must be robust)
Implement `CodexMonitorRPCClient` using Network.framework:
- Use `NWConnection` to host:port
- Write newline-delimited JSON
- Read stream and split by newline; decode JSON per line
- Maintain:
  - atomic/incrementing request id
  - pending requests dictionary: id -> continuation
- Support notifications:
  - app-server-event
  - terminal-output
- On connect:
  1) open connection
  2) send auth token via method "auth"
  3) call "server_info" and store capabilities
- Auto-reconnect:
  - exponential backoff when disconnected
  - show connection state UI
- Keepalive:
  - ping every 25–30s while foreground
- Token:
  - store in Keychain
  - allow user to edit host/token in Settings screen
- Client settings storage:
  - use AppStorage for host/port + Keychain for token
  - allow multiple server profiles (optional but nice)

iOS Data Model + State Management
- Create Swift models mirroring the JSON payloads used in the TS app:
  - WorkspaceInfo, WorkspaceEntry, WorkspaceGroup, WorkspaceSettings
  - Thread list response (cursor/threads), thread detail response (turns/items)
  - ConversationItem model like desktop’s `buildConversationItem`:
    - user message
    - assistant message deltas (streaming)
    - reasoning block
    - tool items (commandExecution, fileChange, mcpToolCall, collabToolCall, webSearch, imageView)
    - review mode started/completed
- Maintain stores:
  - `ServerStore` (connection, capabilities, serverInfo)
  - `WorkspaceStore` (workspaces list, connect/disconnect, groups)
  - `ThreadStore` per workspace (thread list, active thread, cached thread detail)
  - `ConversationStore` per thread:
    - items array
    - in-flight streaming buffers
    - plan text (from turn/plan/updated)
    - approvals queue (server requests)
  - `GitStore` per workspace (status/diffs/log/remote/branches)
  - `PromptStore` (global + workspace prompts)
  - `TerminalStore` (terminal sessions, output buffers)
  - `UsageStore` (local usage snapshot)
- Event routing:
  - Build an `AppServerEventRouter` that consumes app-server-event notifications and updates the correct stores, following desktop logic in `src/features/app/hooks/useAppServerEvents.ts` and `src/features/threads/hooks/useThreads.ts`.
  - Must support:
    - item/started, item/updated, item/completed
    - item/agentMessage/delta
    - item/tool/output/delta
    - turn/plan/updated
    - session/config (to refresh model/rate limits)
    - turn/started, turn/completed, turn/error
    - review mode enter/exit

UI/UX REQUIREMENTS (Liquid Glass iOS translation)
Overall aesthetic:
- Use iOS “materials” (.ultraThinMaterial/.thinMaterial), translucency, blur, subtle borders, rounded corners
- Use smooth spring animations; keep it fast (avoid heavy overdraw)
- Respect light/dark mode
- Use haptics for key actions (send, approve, stage, commit)

Layout mapping
iPad (including iPad mini):
- Use a 2–3 pane adaptive layout:
  - Primary sidebar: Workspace groups + workspaces + thread list
  - Main: Conversation view
  - Trailing “Inspector” panel (toggleable):
    - segmented: Diff / Files / Prompts / Terminal / GitHub / Usage / Settings
  - If screen too small, inspector becomes a sheet with detents.
- Support keyboard shortcuts on iPad hardware keyboard:
  - Cmd+Enter: send
  - Cmd+K: quick switch workspace/thread (optional)
  - Cmd+.: interrupt turn
iPhone:
- NavigationStack:
  - Workspaces -> Threads -> Conversation
  - Inspector panels open as full-screen sheet or bottom sheet:
    - Diff, Files, Terminal, GitHub, Prompts, Usage, Settings
- Keep a persistent bottom composer on conversation screen.

Key screens to implement (must match desktop features)
1) Connection Setup
   - Host:port, token, connect button, connection status, server_info display
2) Workspaces
   - List grouped, connected indicator
   - Add workspace (enter path), remove
   - Connect workspace
   - Worktrees & clones:
     - create worktree: branch name
     - rename worktree upstream
     - apply worktree changes
     - clone workspace into copies folder (use group’s copiesFolder if available)
3) Threads
   - list threads with pagination (cursor/limit)
   - pin/rename/copy/drafts behavior:
     - Desktop stores these locally; on iOS implement locally with persistence (SwiftData/CoreData or simple JSON file).
     - Optional enhancement: add server-side “thread metadata” methods to sync across devices. If you do, keep backwards compatibility.
4) Conversation
   - Render markdown messages (assistant + user)
   - Render tool cards (command executions, file changes, MCP tool calls, collab tool calls)
   - Streaming deltas must append smoothly
   - Auto-scroll when user is at bottom; do not yank scroll when user is reading above
   - Image attachments:
     - use PhotosPicker/camera
     - convert to data URL and send as `images` array (server already supports data: URLs)
   - Stop/interrupt: call turn_interrupt
   - Review: start_review + show review UI
   - Approvals:
     - show modal with details
     - approve/deny -> respond_to_server_request
     - “remember approval rule” -> remember_approval_rule
5) Git panel
   - get_git_status
   - show changed files with additions/deletions
   - staging controls
   - diff viewer with +/− highlighting
   - commit message editor + commit button
   - generate commit message button (calls generate_commit_message)
   - push/pull/sync
   - branch switcher + create branch
6) GitHub panel
   - issues list, PR list
   - PR detail: body + diffs + comments
   - open PR/issue URL in Safari
7) File browser
   - list_workspace_files (add search on client)
   - read_workspace_file with truncation indicator and copy/share
8) Prompts
   - list/create/update/delete/move
   - workspace vs global directories display
9) Terminal
   - terminal_open(workspaceId, terminalId, cols, rows)
   - terminal_write (user keystrokes)
   - terminal_resize on rotation and layout changes
   - terminal_close
   - Use a real terminal emulator view (SwiftTerm recommended) and feed it `terminal-output` notifications.
10) Usage
   - local_usage_snapshot (30 days default)
   - show small charts (simple bar chart) + totals
11) Settings
   - Client: server host/token, reconnect behavior
   - Server: expose get_app_settings/update_app_settings fields relevant to Codex (codexBin, defaultAccessMode, workspace groups)

DICTATION (iOS)
Desktop uses Whisper locally; on iOS we must implement native dictation:
- Provide a mic button in composer:
  - tap to start/stop OR press-and-hold “hold to talk”
- Show live waveform animation while recording
- Use Apple Speech framework if available:
  - Prefer on-device recognition if supported
- Output transcribed text into the composer
- This dictation is CLIENT-SIDE only (do not stream audio to server).
- Keep UX equivalent to desktop feature: fast, low-friction, feels native.

SERVER ↔ CLIENT COMPATIBILITY & SAFETY
- Do not expose daemon to the public internet.
- Recommend binding daemon to 127.0.0.1 and using Tailscale Serve for tailnet-only access.
- Token must be required (no insecure mode for real use).
- Add basic rate limiting / max message size protections:
  - Max JSON line length (e.g., 5–10MB)
  - Max file read already truncated; keep that behavior
- Add protocol versioning:
  - server_info.protocolVersion integer
  - iOS checks and warns if incompatible

DELIVERABLES / ACCEPTANCE CRITERIA
1) Daemon supports all required RPC methods and can run headless on Mac mini, keeping Codex sessions alive.
2) iOS app can:
   - connect over Tailscale
   - list workspaces, connect one
   - list threads and open/resume one
   - stream assistant deltas live
   - handle approvals
   - run git operations end-to-end (status -> stage -> commit -> push)
   - browse files + view file content
   - use prompt library
   - open a terminal and interact with it
   - show usage snapshot
   - dictation works and inserts text into composer
3) UI feels native iOS, with glass-like materials and smooth interactions, adapted for iPad mini + iPhone 15 Pro Max.

IMPLEMENTATION ORDER (DO NOT SKIP)
Phase 1: Server parity
- Add server_info + capabilities
- Implement missing daemon RPC methods (git/prompts/terminal/local_usage/clone/apply_worktree_changes/commit message)
- Add tests for daemon method dispatch and JSON parsing
Phase 2: iOS core
- Connection screen + RPC client
- Workspaces + connect + threads list + conversation open + send message + streaming
- Approvals UI
Phase 3: Feature panels
- Git + commit message gen
- Files
- Prompts
- Terminal
- GitHub
- Usage
Phase 4: Polish
- Liquid glass styling pass
- Performance tuning: conversation virtualization, diff rendering, terminal throughput
- iPad/keyboard shortcuts
- Error handling + reconnect UX

NOTES ON REUSING DESKTOP LOGIC
When implementing conversation item rendering and event routing, use these desktop references:
- `src/features/app/hooks/useAppServerEvents.ts`
- `src/features/threads/hooks/useThreads.ts`
- `src/utils/threadItems.ts` (buildConversationItem equivalents)
Keep the same semantics for tool item types and review mode.

When implementing the daemon methods, use these Rust references:
- workspaces: `src-tauri/src/workspaces.rs`
- codex helpers & commit message generation: `src-tauri/src/codex.rs`
- git + github: `src-tauri/src/git.rs` + `src-tauri/src/git_utils.rs`
- prompts: `src-tauri/src/prompts.rs`
- terminal: `src-tauri/src/terminal.rs` (refactor to remove TauriEventSink dependency)
- local usage: `src-tauri/src/local_usage.rs`

Finally, document:
- How to run daemon on Mac mini with launchd
- How to expose it via Tailscale Serve
- How to configure iOS app host/token
- Troubleshooting checklist (codex doctor, node present, gh auth, permissions)

