# 📱 Mobile API Parity (Desktop ↔️ Daemon ↔️ iOS)

> Source of truth: `src/services/tauri.ts` (frontend calls) + `src-tauri/src/lib.rs` (Tauri handlers) + `src-tauri/src/bin/codex_monitor_daemon.rs` (daemon RPC).

## 🔤 Param casing notes
- **Do not normalize casing.** The UI already expects **mixed** casing:
  - `add_workspace` uses **snake**: `{ path, codex_bin? }`.
  - Most other methods use **camel**: `{ workspaceId, threadId, turnId }`.

## ✅ Legend
- **Desktop Local** = Tauri handler exists (local macOS).
- **Desktop Remote** = Tauri handler delegates to `remote_backend::call_remote`.
- **Daemon** = `codex_monitor_daemon` implements method.
- **iOS parity** = required for full mobile feature parity.

---

## 🧱 Workspaces & Settings

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `list_workspaces` | `{}` | `WorkspaceInfo[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `add_workspace` | `{ path, codex_bin? }` | `WorkspaceInfo` | — | ✅ | ✅ | ✅ | ✅ | `codex_bin` is snake case. |
| `is_workspace_path_dir` | `{ path }` | `boolean` | — | ✅ | ✅ | ✅ | ✅ | Used for validation. |
| `add_clone` | `{ sourceWorkspaceId, copiesFolder, copyName }` | `WorkspaceInfo` | — | ✅ | ✅ | ✅ | ✅ | Needed for clone workflow. |
| `add_worktree` | `{ parentId, branch }` | `WorkspaceInfo` | — | ✅ | ✅ | ✅ | ✅ | — |
| `update_workspace_settings` | `{ id, settings }` | `WorkspaceInfo` | — | ✅ | ✅ | ✅ | ✅ | — |
| `update_workspace_codex_bin` | `{ id, codex_bin? }` | `WorkspaceInfo` | — | ✅ | ✅ | ✅ | ✅ | — |
| `remove_workspace` | `{ id }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `remove_worktree` | `{ id }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `rename_worktree` | `{ id, branch }` | `WorkspaceInfo` | — | ✅ | ✅ | ✅ | ✅ | — |
| `rename_worktree_upstream` | `{ id, oldBranch, newBranch }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `apply_worktree_changes` | `{ workspaceId }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `open_workspace_in` | `{ path, app }` | `void` | — | ✅ | ❌ | ❌ | ❌ | Desktop-only convenience. |
| `connect_workspace` | `{ id }` | `void` | — | ✅ | ✅ | ✅ | ✅ | Starts app-server session. |
| `get_app_settings` | `{}` | `AppSettings` | — | ✅ | ✅ | ✅ | ✅ | — |
| `update_app_settings` | `{ settings }` | `AppSettings` | — | ✅ | ✅ | ✅ | ✅ | — |
| `menu_set_accelerators` | `{ updates:[{id, accelerator}] }` | `void` | — | ✅ | ❌ | ❌ | ❌ | Desktop-only. |
| `codex_doctor` | `{ codexBin? }` | `CodexDoctorResult` | — | ✅ | ✅ | ✅ | ⚠️ | Useful for diagnostics; not required for mobile. |

---

## 🧠 Threads / Codex / Reviews

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `start_thread` | `{ workspaceId }` | `thread/start` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | Spawns new thread. |
| `resume_thread` | `{ workspaceId, threadId }` | `thread/resume` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | — |
| `list_threads` | `{ workspaceId, cursor?, limit? }` | `thread/list` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | — |
| `archive_thread` | `{ workspaceId, threadId }` | `thread/archive` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | — |
| `send_user_message` | `{ workspaceId, threadId, text, model?, effort?, accessMode?, images?, collaborationMode? }` | `send_user_message` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | Streams deltas. |
| `turn_interrupt` | `{ workspaceId, threadId, turnId }` | `turn/interrupt` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | — |
| `start_review` | `{ workspaceId, threadId, target, delivery? }` | `review` result | `app-server-event` | ✅ | ✅ | ✅ | ✅ | — |
| `respond_to_server_request` | `{ workspaceId, requestId, result }` | `void` | `app-server-event` | ✅ | ✅ | ✅ | ✅ | Approval requests. |
| `remember_approval_rule` | `{ workspaceId, command[] }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `model_list` | `{ workspaceId }` | `Model[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `collaboration_mode_list` | `{ workspaceId }` | `Mode[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `account_rate_limits` | `{ workspaceId }` | `RateLimit[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `skills_list` | `{ workspaceId }` | `Skill[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_commit_message_prompt` | `{ workspaceId }` | `string` | — | ✅ | ✅ | ✅ | ✅ | Needed for git commit flow. |
| `generate_commit_message` | `{ workspaceId }` | `string` | — | ✅ | ✅ | ✅ | ✅ | Needed for git commit flow. |

---

## 🧩 Git & GitHub

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `list_git_roots` | `{ workspaceId, depth }` | `string[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_git_status` | `{ workspaceId }` | `{ branchName, files[], stagedFiles[], unstagedFiles[], totalAdditions, totalDeletions }` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_git_diffs` | `{ workspaceId }` | `GitFileDiff[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_git_log` | `{ workspaceId, limit? }` | `GitLogResponse` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_git_commit_diff` | `{ workspaceId, sha }` | `GitCommitDiff[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_git_remote` | `{ workspaceId }` | `string?` | — | ✅ | ✅ | ✅ | ✅ | — |
| `stage_git_file` | `{ workspaceId, path }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `stage_git_all` | `{ workspaceId }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `unstage_git_file` | `{ workspaceId, path }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `revert_git_file` | `{ workspaceId, path }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `revert_git_all` | `{ workspaceId }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `commit_git` | `{ workspaceId, message }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `pull_git` | `{ workspaceId }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `push_git` | `{ workspaceId }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `sync_git` | `{ workspaceId }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `list_git_branches` | `{ workspaceId }` | `BranchInfo[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `checkout_git_branch` | `{ workspaceId, name }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `create_git_branch` | `{ workspaceId, name }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_github_issues` | `{ workspaceId }` | `GitHubIssuesResponse` | — | ✅ | ✅ | ✅ | ✅ | Requires `gh` auth on daemon. |
| `get_github_pull_requests` | `{ workspaceId }` | `GitHubPullRequestsResponse` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_github_pull_request_diff` | `{ workspaceId, prNumber }` | `GitHubPullRequestDiff[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `get_github_pull_request_comments` | `{ workspaceId, prNumber }` | `GitHubPullRequestComment[]` | — | ✅ | ✅ | ✅ | ✅ | — |

---

## 🗂️ Files

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `list_workspace_files` | `{ workspaceId }` | `string[]` | — | ✅ | ✅ | ✅ | ✅ | Includes ignored filtering. |
| `read_workspace_file` | `{ workspaceId, path }` | `{ content, truncated }` | — | ✅ | ✅ | ✅ | ✅ | Large file truncation. |

---

## 🧾 Prompts

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `prompts_list` | `{ workspaceId }` | `CustomPromptEntry[]` | — | ✅ | ✅ | ✅ | ✅ | — |
| `prompts_create` | `{ workspaceId, scope, name, description?, argumentHint?, content }` | `CustomPromptEntry` | — | ✅ | ✅ | ✅ | ✅ | — |
| `prompts_update` | `{ workspaceId, path, name, description?, argumentHint?, content }` | `CustomPromptEntry` | — | ✅ | ✅ | ✅ | ✅ | — |
| `prompts_delete` | `{ workspaceId, path }` | `void` | — | ✅ | ✅ | ✅ | ✅ | — |
| `prompts_move` | `{ workspaceId, path, scope }` | `CustomPromptEntry` | — | ✅ | ✅ | ✅ | ✅ | — |
| `prompts_workspace_dir` | `{ workspaceId }` | `string` | — | ✅ | ✅ | ✅ | ✅ | — |
| `prompts_global_dir` | `{}` | `string` | — | ✅ | ✅ | ✅ | ✅ | — |

---

## 🖥️ Terminal

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `terminal_open` | `{ workspaceId, terminalId, cols, rows }` | `{ id }` | `terminal-output` | ✅ | ✅ | ✅ | ✅ | Must emit `terminal-output` notifications. |
| `terminal_write` | `{ workspaceId, terminalId, data }` | `void` | `terminal-output` | ✅ | ✅ | ✅ | ✅ | — |
| `terminal_resize` | `{ workspaceId, terminalId, cols, rows }` | `void` | `terminal-output` | ✅ | ✅ | ✅ | ✅ | — |
| `terminal_close` | `{ workspaceId, terminalId }` | `void` | `terminal-output` | ✅ | ✅ | ✅ | ✅ | — |

---

## 🗣️ Dictation

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `dictation_model_status` | `{ modelId? }` | `DictationModelStatus` | — | ✅ | ❌ | ❌ | ⚠️ | iOS may use native Speech instead. |
| `dictation_download_model` | `{ modelId? }` | `DictationModelStatus` | — | ✅ | ❌ | ❌ | ⚠️ | — |
| `dictation_cancel_download` | `{ modelId? }` | `DictationModelStatus` | — | ✅ | ❌ | ❌ | ⚠️ | — |
| `dictation_remove_model` | `{ modelId? }` | `DictationModelStatus` | — | ✅ | ❌ | ❌ | ⚠️ | — |
| `dictation_start` | `{ preferredLanguage? }` | `DictationSessionState` | — | ✅ | ❌ | ❌ | ⚠️ | — |
| `dictation_stop` | `{}` | `DictationSessionState` | — | ✅ | ❌ | ❌ | ⚠️ | — |
| `dictation_cancel` | `{}` | `DictationSessionState` | — | ✅ | ❌ | ❌ | ⚠️ | — |

---

## 📈 Usage

| Method | Params (schema) | Response | Events | Desktop Local | Desktop Remote | Daemon | iOS Parity? | Notes |
|---|---|---|---|---|---|---|---|---|
| `local_usage_snapshot` | `{ days, workspacePath? }` | `LocalUsageSnapshot` | — | ✅ | ✅ | ✅ | ✅ | Used on Home dashboard. |

---

## ⚠️ Desktop-only helpers (non-RPC)
- `pickWorkspacePath()` and `pickImageFiles()` use the Tauri dialog plugin and are **not** RPC methods.
- iOS will supply its own file/asset pickers.
