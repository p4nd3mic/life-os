# CodexMonitor Mobile Companion Plan (from Five Point Two Pro)

> Source: User-provided plan in this work session. This file preserves the original scope and phases so we can track implementation.

---

YOU ARE CODEX (EXTRA HIGH). Implement a fully native iOS/iPadOS companion app for the existing CodexMonitor repo.

## REPO CONTEXT (already present in this repo)
- Desktop app: Tauri 2 + React 19 + TS (src/), Rust backend (src-tauri/src/).
- CodexMonitor orchestrates Codex “app-server” processes per workspace.
- There is already a remote backend implementation:
  - Desktop-side remote client: src-tauri/src/remote_backend.rs (TCP line-delimited JSON-RPC + auth + event notifications).
  - Server-side daemon binary: src-tauri/src/bin/codex_monitor_daemon.rs (proof-of-concept JSON-RPC over TCP, sends “app-server-event” notifications).

## GOAL
Build a native iOS 26+ app (iPhone + iPad) that has feature parity with CodexMonitor desktop:
- Workspaces: add/remove, connect, worktrees/clones, settings
- Threads: list/resume/start/archive, drafts, unread/running state
- Conversation: render streaming messages, diffs, tool output, approval prompts, reviews
- Composer: queueing, attachments (at least Photos/Files/paste), skill/prompt/file autocomplete
- Git: status, diffs, staging, revert, branches, log, commit prompt + commit generation, pull/push/sync
- GitHub via gh: issues/PR lists, PR diffs, PR comments
- Files: file tree + read file contents
- Prompts: list/create/update/delete/move (workspace + global)
- Local usage snapshot
- Terminal dock: open/write/resize/close; stream output
- Dictation: provide a mobile-equivalent (can be iOS Speech locally OR add remote whisper transcription)
- Settings: remote host/token, default model/access/effort, experimental flags where applicable
- UI look: native, smooth, and uses iOS 26 Liquid Glass design language (SwiftUI glassEffect APIs). Must feel first-party.

## ARCHITECTURE DECISION
- iOS app will connect over Tailscale to the user’s always-on Mac mini M4.
- The Mac mini runs the codex_monitor_daemon server (extended to full parity).
- iOS app speaks the existing daemon protocol: line-delimited JSON-RPC over TCP + auth handshake.
- Keep ONE persistent TCP connection for requests + server notifications; auto-reconnect; robust to backgrounding.

## NON-GOALS
- Do NOT build a web wrapper.
- Do NOT rely on a browser-based UI.
- Do NOT require the iOS app to run Codex locally.

-----------------------------------------
PHASE 0 — AUDIT / SPEC / PARITY CHECK
-----------------------------------------
0.1 Inventory the “true API surface” used by the frontend:
- Read src/services/tauri.ts and list every invoke-call method that desktop UI uses.
- Cross-check with src-tauri/src/lib.rs handler list (tauri::generate_handler![...]) to confirm authoritative method list.
- Cross-check daemon implementation in src-tauri/src/bin/codex_monitor_daemon.rs. Identify what’s missing.

Deliverable:
- docs/mobile_api_parity.md
  - table of methods:
    - Method name
    - Params schema
    - Response schema
    - Events emitted (if any)
    - Implemented? (desktop local / desktop remote / daemon)
    - Needed for iOS parity? (yes/no)
  - Include notes on param casing (some are camelCase like workspaceId, some snake like codex_bin).

Acceptance Criteria:
- A complete list of all remote-callable methods needed for mobile parity.
- Clear plan for any methods that are “desktop-only” (e.g., file picker dialogs).

-----------------------------------------
PHASE 1 — BACKEND: MAKE DAEMON FULL-PARITY
-----------------------------------------
Current situation:
- Daemon implements only a subset: workspaces + threads + send_user_message + model_list + rate limits + skills + collab modes, etc.
- Desktop “remote mode” is partial: many modules do not check is_remote_mode() (git, prompts, terminal, dictation, local_usage, commit message helpers).

Goal:
- Implement all parity methods server-side in the daemon.
- (Optional but recommended) make desktop remote mode actually work for those methods too by adding remote-mode delegation where missing.

1.1 Refactor shared logic into “core” modules (recommended)
Create new Rust module(s) that are independent of tauri::State / AppHandle:
- src-tauri/src/core/
  - core_workspaces.rs
  - core_codex.rs
  - core_git.rs
  - core_prompts.rs
  - core_terminal.rs
  - core_usage.rs
  - core_dictation.rs (if doing remote whisper)
Each core function should take:
- A plain state struct (HashMaps + paths)
- An EventSink trait object (already exists: backend/events.rs)
- And/or workspace_id, thread_id, etc.

Then:
- Tauri commands call into core_*.
- Daemon dispatch calls into core_*.

This prevents duplicating logic and keeps desktop+mobile consistent.

1.2 Implement missing daemon methods
Add these methods to daemon dispatch (mirror src/services/tauri.ts names):
- settings:
  - get_app_settings
  - update_app_settings
- codex utilities:
  - run_codex_doctor (desktop calls codex::codex_doctor)
  - remember_approval_rule
  - get_commit_message_prompt
  - generate_commit_message
- workspaces:
  - is_workspace_path_dir
  - add_clone
  - rename_worktree / rename_worktree_upstream / apply_worktree_changes
  - open_workspace_in (can be a no-op on daemon or return error “unsupported”; mobile won’t use it)
- git:
  - list_git_roots, get_git_status, get_git_diffs, get_git_log, get_git_commit_diff, get_git_remote
  - stage_git_file, stage_git_all, unstage_git_file, revert_git_file, revert_git_all
  - commit_git, pull_git, push_git, sync_git
  - list_git_branches, checkout_git_branch, create_git_branch
  - github via gh: get_github_issues, get_github_pull_requests, get_github_pull_request_diff, get_github_pull_request_comments
- files:
  - list_workspace_files
  - read_workspace_file (already exists in workspaces.rs; daemon must match behavior)
- prompts:
  - prompts_list, prompts_create, prompts_update, prompts_delete, prompts_move, prompts_workspace_dir, prompts_global_dir
- terminal:
  - terminal_open, terminal_write, terminal_resize, terminal_close
  - MUST emit “terminal-output” notifications (same as desktop remote backend expects).
- local usage:
  - local_usage_snapshot

1.3 IMPORTANT: Param casing must match existing UI
Example:
- add_workspace uses { path, codex_bin? } (snake case codex_bin)
- most others use camelCase keys like workspaceId, threadId, turnId, etc.
Do NOT “fix” casing globally; match existing.

1.4 Daemon protocol / server loop scaffolding
Keep existing line-delimited JSON objects.
Requests:
  {"id": number, "method": string, "params": object|null}
Responses:
  {"id": number, "result": any}
or
  {"id": number, "error": {"message": string}}
Notifications:
  {"method":"app-server-event","params":{...}}
  {"method":"terminal-output","params":{...}}

Auth handshake:
First request must be:
  {"id":1,"method":"auth","params":{"token":"..."}}

1.5 Add integration tests for daemon RPC
Create:
- src-tauri/tests/daemon_rpc.rs
Spin up daemon server on an ephemeral port with insecure auth disabled + a fixed token.
Then:
- Connect, auth, call ping
- Call list_workspaces, add_workspace (temp dir), connect_workspace
- Start a thread (if Codex available in CI environment; if not, mock or feature-gate)
- Validate error messages are stable and informative.

1.6 Desktop remote mode completion (recommended)
For each tauri command file that does NOT currently call is_remote_mode():
- Add remote delegation via call_remote(method, params).
Examples likely missing:
- src-tauri/src/git.rs
- src-tauri/src/prompts.rs
- src-tauri/src/terminal.rs
- src-tauri/src/dictation.rs
- src-tauri/src/local_usage.rs
- src-tauri/src/codex.rs for:
  - remember_approval_rule
  - get_commit_message_prompt
  - generate_commit_message

Pattern to follow (see src-tauri/src/workspaces.rs and methods already delegating):
  if is_remote_mode(&state).await {
      return call_remote(&state, "method_name", json!({ ... })).await;
  }

-----------------------------------------
PHASE 2 — MAC MINI DEPLOYMENT (TAILSCALE + LAUNCHD)
-----------------------------------------
2.1 Add a docs page:
- docs/mobile_backend_setup.md
Include:
- Build daemon:
  cd src-tauri && cargo build --release --bin codex_monitor_daemon
- Choose data dir (e.g. /Users/<you>/Library/Application Support/codex-monitor-daemon)
- Choose token

2.2 Provide launchd plist template
Create:
- scripts/com.codexmonitor.daemon.plist (template with placeholders)
Example:
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.codexmonitor.daemon</string>
  <key>ProgramArguments</key>
  <array>
    <string>/ABS/PATH/TO/codex_monitor_daemon</string>
    <string>--listen</string><string>127.0.0.1:4732</string>
    <string>--data-dir</string><string>/Users/YOU/Library/Application Support/codex-monitor-daemon</string>
    <string>--token</string><string>REPLACE_WITH_STRONG_TOKEN</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>/tmp/codex-monitor-daemon.out.log</string>
  <key>StandardErrorPath</key><string>/tmp/codex-monitor-daemon.err.log</string>
</dict>
</plist>

2.3 Tailscale guidance
Prefer:
- daemon binds localhost 127.0.0.1:4732
- use “tailscale serve tcp 4732” OR directly connect using MagicDNS + local port forwarding rules (document both).
DO NOT expose to public internet. Rely on tailnet + daemon token.

-----------------------------------------
PHASE 3 — iOS CLIENT LIBRARY (NETWORK + MODELS + STATE)
-----------------------------------------
Create new folder:
- ios/CodexMonitorMobile/

3.1 Xcode project
- iOS deployment target: iOS 26.0
- iPadOS supported.
- Swift 6 concurrency enabled.
- Use SwiftUI.

3.2 Create a Swift Package inside ios/:
- ios/Packages/CodexMonitorRPC/
Modules:
- CodexMonitorRPC (network + JSON-RPC + reconnect)
- CodexMonitorModels (Codable models matching TS/Rust types)
- CodexMonitorRendering (markdown/diff render helpers)

3.3 Implement JSON-RPC over TCP with newline framing using Network.framework
Core types:

// CodexMonitorRPC/Sources/CodexMonitorRPC/RPCMessage.swift
import Foundation

public struct RPCRequest: Codable {
  public let id: UInt64
  public let method: String
  public let params: JSONValue?
}

public struct RPCResponse: Codable {
  public let id: UInt64
  public let result: JSONValue?
  public let error: RPCError?
}

public struct RPCNotification: Codable {
  public let method: String
  public let params: JSONValue?
}

public struct RPCError: Codable, Error {
  public let message: String
}

Implement a JSONValue type (Codable enum) for arbitrary JSON:
- string/number/bool/null/object/array

3.4 The connection actor
// CodexMonitorRPC/Sources/CodexMonitorRPC/RPCClient.swift
import Foundation
import Network

public actor RPCClient {
  public struct Config: Sendable {
    public var host: String
    public var port: UInt16
    public var token: String
  }

  private var connection: NWConnection?
  private var nextID: UInt64 = 2
  private var pending: [UInt64: CheckedContinuation<JSONValue, Error>] = [:]
  private var buffer = Data()

  public var onNotification: (@Sendable (RPCNotification) -> Void)?

  public init() {}

  public func connect(_ config: Config) async throws {
    // 1) open NWConnection
    // 2) start receive loop
    // 3) send auth request id=1
    // 4) wait for auth response OK
  }

  public func disconnect() {
    connection?.cancel()
    connection = nil
  }

  public func call(method: String, params: JSONValue? = nil) async throws -> JSONValue {
    let id = nextID; nextID += 1
    let req = RPCRequest(id: id, method: method, params: params)
    return try await withCheckedThrowingContinuation { cont in
      pending[id] = cont
      sendLine(req)
    }
  }

  private func sendLine<T: Encodable>(_ value: T) {
    // encode JSON + append "\n"
    // connection?.send(...)
  }

  private func startReceiveLoop() {
    // receive(minimumIncompleteLength:maximumLength:)
    // accumulate into buffer
    // split by '\n'
    // decode each line:
    // - if has "id" => response
    // - else if has "method" => notification
  }
}

3.5 Typed API wrapper
// CodexMonitorRPC/Sources/CodexMonitorRPC/CodexMonitorAPI.swift
public struct CodexMonitorAPI {
  public let rpc: RPCClient

  public func ping() async throws { _ = try await rpc.call(method: "ping") }
  public func listWorkspaces() async throws -> [WorkspaceEntry] { ... }
  ...
}

3.6 App state + reducers (port the desktop logic)
Desktop has:
- src/features/threads/hooks/useThreadsReducer.ts
- src/utils/threadItems.ts
Port the core “streaming item merge” logic into Swift so the mobile UI matches:
- maintain per-thread array of “ConversationItem”
- apply “item/agentMessage/delta” patches
- handle “tool”, “diff”, “approval-request”, “review”, “reasoning”
- keep turn status: running/unread

Create:
- ios/CodexMonitorMobile/CodexStore.swift (ObservableObject @MainActor)
- Keep:
  - workspaces: [Workspace]
  - activeWorkspaceID
  - threadsByWorkspace: [String: ThreadListState]
  - threadItemsByThread: [ThreadID: [ConversationItem]]
  - git state caches, prompts caches, files caches
- Subscribe to RPC notifications:
  - “app-server-event”
  - “terminal-output”
Route them through the reducer.

-----------------------------------------
PHASE 4 — iOS/iPadOS UI (NATIVE + RESPONSIVE)
-----------------------------------------
Match desktop feature set, but adapt UX to touch + small screens.

4.1 Navigation structure
iPhone:
- TabView with 4 tabs (mirror PhoneLayout.tsx):
  - Projects (workspaces + threads entry)
  - Codex (conversation + composer)
  - Git (git panel + diff viewer drill-in)
  - Log (debug/events panel)

iPad:
- NavigationSplitView (3-column if possible):
  - Column 1: Workspaces
  - Column 2: Threads (for selected workspace)
  - Column 3: Detail (Conversation / Git / Files / Prompts via segmented control)
- Provide toolbar actions in top bar.

4.2 Screen checklist
A) Connection / Settings Screen
- host, port, token
- “Test Connection” (calls ping)
- show connection status + last error
- store token in Keychain

B) Home / Dashboard
- quick actions: new thread, resume recent, usage snapshot
- show rate limit usage ring (port the concept; doesn’t need to match pixel-perfect)

C) Workspaces
- list + grouping + sorting
- add workspace (manual path typing; optional “recent repo paths”)
- add worktree / clone
- workspace settings (git_root, codex_bin override, group, sort)
- connect/reconnect status

D) Threads list
- list threads with unread/running status
- pin/rename/archive/copy (if supported)
- tap thread => resume_thread then show conversation

E) Conversation view
- message list (LazyVStack)
- render markdown/code blocks/diffs/tool output
- approval prompts: show “Approve / Deny” actions wired to respond_to_server_request
- review view: start_review
- stop/interrupt current turn: turn_interrupt

F) Composer
- multiline editor
- queue mode (allow multiple sends? match desktop)
- attachments:
  - PhotosPicker
  - Files importer
  - Pasteboard image
  - Encode as data: URL base64 and pass into send_user_message images[]
- Autocomplete:
  - “$” => skills_list suggestions
  - “/prompts:” => prompts_list suggestions
  - “@” => file path suggestions from list_workspace_files or cached tree
- controls row:
  - model picker
  - effort (reasoning effort)
  - accessMode
  - context usage ring (if available from events)

G) Git
- status list
- staged/unstaged diff viewer
- stage/unstage/revert
- commit:
  - get_commit_message_prompt
  - generate_commit_message
  - commit_git
- pull/push/sync
- branches: list/checkout/create
- GitHub:
  - issues list
  - PR list
  - PR diff + comments

H) Files
- tree view with search
- tap file => read_workspace_file and display
- show file-type icons
- optional “share file” (download to temp + UIActivityViewController)

I) Prompts
- list prompts (workspace + global)
- create/edit/delete/move
- “Run in current thread” => paste into composer OR send directly

J) Terminal
- simple terminal view
- openTerminalSession => start pty
- display streaming output via “terminal-output”
- send input with writeTerminalSession
- resize on geometry changes

K) Debug panel
- show raw events & allow copy/clear

-----------------------------------------
PHASE 5 — LIQUID GLASS UI (iOS 26 SwiftUI APIs)
-----------------------------------------
Requirement:
- Must look/feel like CodexMonitor’s “liquid glass” aesthetic, but using native iOS 26 APIs.

5.1 Use system Liquid Glass where possible
- Use standard TabView / NavigationSplitView / toolbars so the OS gives you glass automatically.
- Avoid painting opaque backgrounds behind toolbars/sheets.
(Reference: WWDC25 “Build a SwiftUI app with the new design” for best practices.)

5.2 For custom surfaces: use glassEffect + GlassEffectContainer
Create reusable components:
- GlassCard
- GlassToolbarChip
- GlassBadge

Example:

import SwiftUI

struct GlassCard<Content: View>: View {
  let id: String
  @ViewBuilder var content: Content

  var body: some View {
    content
      .padding(14)
      .glassEffect(.regular, in: .rect(cornerRadius: 18))
      .overlay(
        RoundedRectangle(cornerRadius: 18)
          .strokeBorder(.white.opacity(0.18), lineWidth: 1)
      )
  }
}

Group adjacent glass elements inside a container to ensure correct sampling:
struct GlassGroup<Content: View>: View {
  @ViewBuilder var content: Content
  var body: some View {
    GlassEffectContainer { content }
  }
}

Use glassEffectID for morphing transitions between related glass items:
- e.g. selecting a thread morphs a chip into a header badge.

5.3 Interactive glass for tappable controls
Use interactive glass styles for buttons/chips:
- e.g. .glassEffect(.regular.tint(.blue).interactive(), in: ...)

5.4 Apply glassBackgroundEffect for panels/sheets if needed
For large surfaces (side panels, sheets), apply:
- .glassBackgroundEffect(in:shape, displayMode: ...)
(Use the official modifier name available in iOS 26 SDK.)

5.5 Visual hierarchy rules
- Content should be “primary”, glass is a functional overlay.
- Keep contrast/accessibility: dynamic type + sufficient vibrancy.

-----------------------------------------
PHASE 6 — PERFORMANCE, RELIABILITY, SECURITY
-----------------------------------------
6.1 Connection robustness
- Auto reconnect with exponential backoff.
- When app backgrounds: disconnect cleanly; on foreground: reconnect + refresh.
- On reconnect:
  - list_workspaces
  - connect_workspace for last active workspace
  - list_threads + resume_thread for active thread
- Ensure reducer handles duplicate events / out-of-order deltas safely.

6.2 Large lists & diffs
- Use LazyVStack
- Use incremental loading/cursor for list_threads if supported.
- For huge diffs: provide “Load more” or “Open file diff only”.

6.3 Security
- Token stored in iOS Keychain.
- Daemon refuses any request before auth success.
- Recommend running daemon only on localhost and exposing via Tailscale, not 0.0.0.0.

-----------------------------------------
PHASE 7 — DELIVERABLES / ACCEPTANCE TESTS
-----------------------------------------
7.1 Deliverables
- ios/CodexMonitorMobile Xcode project
- ios/Packages/CodexMonitorRPC Swift package
- Rust daemon updated with full method parity
- docs/mobile_backend_setup.md
- docs/mobile_api_parity.md

7.2 Acceptance tests (manual checklist)
- From iPhone on Tailscale:
  - Connect, auth, list workspaces
  - Start thread, send message, observe streaming response
  - Approval prompt appears and can be approved/denied
  - Git status loads; stage/unstage works; commit flow works
  - Files tree loads; file contents open
  - Prompts list + create/update works
  - Terminal opens and streams output
  - Local usage snapshot loads
- UI:
  - Feels native, smooth scrolling, no jank
  - Liquid Glass look: toolbars/panels/chips/cards use glassEffect correctly

-----------------------------------------
IMPLEMENTATION NOTES / FILES TO TOUCH (CONCRETE)
-----------------------------------------
Rust (src-tauri):
- src-tauri/src/bin/codex_monitor_daemon.rs (add methods + refactor into core modules)
- src-tauri/src/remote_backend.rs (if adding missing remote delegations)
- src-tauri/src/git.rs, prompts.rs, terminal.rs, dictation.rs, local_usage.rs, codex.rs
- add new src-tauri/src/core/* modules if refactoring
- add tests: src-tauri/tests/daemon_rpc.rs

iOS:
- ios/CodexMonitorMobile/CodexMonitorMobileApp.swift
- ios/CodexMonitorMobile/Views/*
- ios/CodexMonitorMobile/ViewModels/*
- ios/CodexMonitorMobile/CodexStore.swift
- ios/Packages/CodexMonitorRPC/Sources/*

Do the work in order, commit each phase separately with clear messages.
