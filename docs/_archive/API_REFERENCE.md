# Daemon API Reference (TCP JSON-RPC)

This document describes **every RPC method implemented by the CodexMonitor daemon** (`src-tauri/src/bin/codex_monitor_daemon.rs`).

The daemon speaks a lightweight, **newline-delimited JSON** protocol that is *JSON-RPC-inspired*, but **not strict JSON-RPC 2.0** (no `jsonrpc` field).

---

## Transport + framing

- **Transport:** TCP
- **Framing:** **one JSON object per line** (newline-delimited)

### Message shapes

**Request**

```json
{
  "id": 1,
  "method": "list_workspaces",
  "params": {}
}
```

**Response (success)**

```json
{
  "id": 1,
  "result": [
    {
      "id": "...",
      "name": "..."
    }
  ]
}
```

**Response (error)**

```json
{
  "id": 1,
  "error": {
    "message": "unauthorized"
  }
}
```

**Notification (server → client)**

```json
{
  "method": "app-server-event",
  "params": {
    "workspace_id": "...",
    "message": {
      "method": "item/agentMessage/delta",
      "params": {
        "delta": "hi"
      }
    }
  }
}
```

### Error semantics

- Errors are always shaped as:
  - `{"id": <id>, "error": {"message": "..."}}`
- There is no standardized error `code` field.


---

## Authentication

If the daemon is configured with a token (`--token` or `CODEX_MONITOR_DAEMON_TOKEN`), a client **must** call `auth` first. Until authenticated, all other methods respond with `error.message = "unauthorized"`.

When the token is **not** configured (dev-only `--insecure-no-auth`), clients may omit `auth` entirely. In that mode, `auth` is not implemented and returns `unknown method: auth`.


---

## RPC methods

Notes on types:
- Cross-platform data structures are documented in `docs/DATA_MODELS.md`.
- Some Codex-backed methods return the **raw Codex app-server response envelope** (an object containing its own `id` and `result`). The daemon wraps that envelope inside the daemon response’s `result`.


---

## Connection

### `auth`

- **Direction:** client → daemon
- **Auth required:** no


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `token` | `string` | yes | Shared secret token configured on daemon (env CODEX_MONITOR_DAEMON_TOKEN or --token). |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "auth",
  "params": {
    "token": "..."
  }
}
```
```json
{
  "id": 1,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Must be the first call on a new TCP connection when the daemon is started with a token.

- If daemon is started without a token (dev-only `--insecure-no-auth`), this method is **not implemented** and will return `unknown method: auth`.



### `ping`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

_No params._


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "ping"
}
```
```json
{
  "id": 1,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Used as a cheap liveness check after auth.




---

## Workspaces

### `list_workspaces`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

_No params._


**Response**

WorkspaceInfo[] (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "list_workspaces"
}
```
```json
{
  "id": 2,
  "result": [
    {
      "id": "...",
      "name": "MyRepo",
      "path": "/Users/me/MyRepo",
      "connected": true,
      "codex_bin": null,
      "kind": "main",
      "parent_id": null,
      "worktree": null,
      "settings": {}
    }
  ]
}
```

**Notes**

- Order: sorted by `workspace.settings.sort_order` then name (see `sort_workspaces`).

- `connected` is computed from whether a Codex session exists in memory.



### `is_workspace_path_dir`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `path` | `string` | yes | Absolute path to validate. |


**Response**

boolean


**Example**

```json
{
  "id": 1,
  "method": "is_workspace_path_dir",
  "params": {
    "path": "..."
  }
}
```
```json
{
  "id": 3,
  "result": true
}
```

**Notes**

- Returns true if the path exists and is a directory on the daemon host.



### `add_workspace`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `path` | `string` | yes | Absolute path to an existing folder on the daemon host. |
| `codex_bin` | `string|null` | no | Optional per-workspace codex binary override. |


**Response**

WorkspaceInfo


**Example**

```json
{
  "id": 1,
  "method": "add_workspace",
  "params": {
    "path": "..."
  }
}
```
```json
{
  "id": 4,
  "result": {
    "id": "...",
    "name": "MyRepo",
    "path": "/Users/me/MyRepo",
    "connected": true,
    "codex_bin": null,
    "kind": "main",
    "parent_id": null,
    "worktree": null,
    "settings": {}
  }
}
```

**Notes**

- Immediately spawns a `codex app-server` session for the workspace; `connected` is returned as `true`.

- Persists to `<data-dir>/workspaces.json`.

- Fails if `path` is not a directory.



### `add_clone`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `sourceWorkspaceId` | `string` | yes | Workspace id to clone from. |
| `copiesFolder` | `string` | yes | Absolute folder under which clones will be created. |
| `copyName` | `string` | yes | Name of the new clone directory (will be sanitized). |


**Response**

WorkspaceInfo (new clone workspace)


**Example**

```json
{
  "id": 1,
  "method": "add_clone",
  "params": {
    "sourceWorkspaceId": "...",
    "copiesFolder": "...",
    "copyName": "..."
  }
}
```
```json
{
  "id": 5,
  "result": {
    "id": "...",
    "name": "MyRepo (clone)",
    "path": "/Users/me/CodexCopies/MyRepo-clone",
    "connected": true,
    "codex_bin": null,
    "kind": "clone",
    "parent_id": "<source>",
    "worktree": null,
    "settings": {}
  }
}
```

**Notes**

- Runs `git clone` under the hood (see daemon implementation).

- Spawns a Codex session for the clone immediately.

- Persists to `workspaces.json`.



### `add_worktree`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `parentId` | `string` | yes | Workspace id to create a worktree from. |
| `branch` | `string` | yes | Branch name for the new worktree. |


**Response**

WorkspaceInfo (new worktree workspace)


**Example**

```json
{
  "id": 1,
  "method": "add_worktree",
  "params": {
    "parentId": "...",
    "branch": "..."
  }
}
```
```json
{
  "id": 6,
  "result": {
    "id": "...",
    "name": "MyRepo (feature-x)",
    "path": "/Users/me/Library/Application Support/.../worktrees/<id>",
    "connected": true,
    "codex_bin": null,
    "kind": "worktree",
    "parent_id": "<parent>",
    "worktree": {
      "branch": "feature-x",
      "upstream": "origin/feature-x"
    },
    "settings": {}
  }
}
```

**Notes**

- Creates a git worktree directory under the daemon data dir (see `worktree_root_dir`).

- Also supports legacy `.codex-worktrees` behavior for older setups (desktop backend).

- Spawns a Codex session for the worktree immediately.



### `connect_workspace`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Workspace id to connect/spawn session for. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "connect_workspace",
  "params": {
    "id": "..."
  }
}
```
```json
{
  "id": 7,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Spawns (or re-spawns) the workspace's `codex app-server` session in memory.

- Most Codex-backed methods require the workspace to be connected, otherwise you get `workspace not connected`.



### `remove_workspace`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Workspace id to remove. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "remove_workspace",
  "params": {
    "id": "..."
  }
}
```
```json
{
  "id": 8,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Stops the workspace session if running, removes it from `workspaces.json`.

- For worktrees/clones, also deletes their on-disk directories (see daemon).



### `remove_worktree`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Worktree workspace id to remove. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "remove_worktree",
  "params": {
    "id": "..."
  }
}
```
```json
{
  "id": 9,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Only valid for `WorkspaceKind::Worktree` entries.

- Kills the session then removes the worktree directory and entry.



### `rename_worktree`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Worktree workspace id. |
| `branch` | `string` | yes | New branch name. |


**Response**

WorkspaceInfo (updated)


**Example**

```json
{
  "id": 1,
  "method": "rename_worktree",
  "params": {
    "id": "...",
    "branch": "..."
  }
}
```
```json
{
  "id": 10,
  "result": {
    "id": "...",
    "name": "MyRepo (new-branch)",
    "path": "...",
    "connected": true,
    "kind": "worktree",
    "parent_id": "...",
    "worktree": {
      "branch": "new-branch",
      "upstream": null
    },
    "settings": {}
  }
}
```

**Notes**

- Renames the worktree branch metadata and updates `workspaces.json`.



### `rename_worktree_upstream`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Worktree workspace id. |
| `oldBranch` | `string` | yes | Old upstream branch name. |
| `newBranch` | `string` | yes | New upstream branch name. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "rename_worktree_upstream",
  "params": {
    "id": "...",
    "oldBranch": "...",
    "newBranch": "..."
  }
}
```
```json
{
  "id": 11,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Used to keep upstream metadata consistent when upstream branch name changes.



### `apply_worktree_changes`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Worktree workspace id to apply changes into upstream/base (implementation-specific). |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "apply_worktree_changes",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 12,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Implementation applies worktree changes; see daemon code for exact git operations.



### `open_workspace_in`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `app` | `string` | yes | Target application identifier (e.g. 'finder', 'terminal', etc). |


**Response**

ERROR (not supported in daemon mode)


**Example**

```json
{
  "id": 1,
  "method": "open_workspace_in",
  "params": {
    "workspaceId": "...",
    "app": "..."
  }
}
```
```json
{
  "id": 13,
  "error": {
    "message": "open_workspace_in is not supported in daemon mode."
  }
}
```

**Notes**

- This command exists in the desktop backend (local mode) but is intentionally disabled in the daemon.



### `update_workspace_settings`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Workspace id. |
| `settings` | `WorkspaceSettings` | yes | Workspace settings blob (see DATA_MODELS.md). |


**Response**

WorkspaceInfo (updated)


**Example**

```json
{
  "id": 1,
  "method": "update_workspace_settings",
  "params": {
    "id": "...",
    "settings": {}
  }
}
```
```json
{
  "id": 14,
  "result": {
    "id": "...",
    "name": "...",
    "path": "...",
    "connected": true,
    "kind": "main",
    "parent_id": null,
    "worktree": null,
    "settings": {
      "sort_order": 1
    }
  }
}
```

**Notes**

- Rewrites the persisted workspace entry in `workspaces.json`.



### `update_workspace_codex_bin`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `id` | `string` | yes | Workspace id. |
| `codex_bin` | `string|null` | no | Per-workspace Codex binary override, or null to clear. |


**Response**

WorkspaceInfo (updated)


**Example**

```json
{
  "id": 1,
  "method": "update_workspace_codex_bin",
  "params": {
    "id": "..."
  }
}
```
```json
{
  "id": 15,
  "result": {
    "id": "...",
    "codex_bin": "/usr/local/bin/codex",
    "connected": true
  }
}
```

**Notes**

- Does not automatically restart a running workspace session; clients may call `connect_workspace` to respawn.




---

## Workspace files

### `list_workspace_files`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

string[] (relative file paths under workspace root)


**Example**

```json
{
  "id": 1,
  "method": "list_workspace_files",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 16,
  "result": [
    "README.md",
    "src/main.tsx",
    "src-tauri/src/lib.rs"
  ]
}
```

**Notes**

- Respects ignore rules via the `ignore` crate; skips heavy dirs like `.git` and `node_modules`.

- Has an internal max-file safety limit (see code).



### `read_workspace_file`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | Relative path within the workspace. |


**Response**

{ content: string, truncated: boolean }


**Example**

```json
{
  "id": 1,
  "method": "read_workspace_file",
  "params": {
    "workspaceId": "...",
    "path": "..."
  }
}
```
```json
{
  "id": 17,
  "result": {
    "content": "# README\\n...",
    "truncated": false
  }
}
```

**Notes**

- The daemon enforces that the resolved path stays within the workspace root.

- Large files are truncated (see `MAX_FILE_BYTES` in daemon).




---

## App settings & diagnostics

### `get_app_settings`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

_No params._


**Response**

AppSettings (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_app_settings"
}
```
```json
{
  "id": 18,
  "result": {
    "codexBin": null,
    "backendMode": "local",
    "remoteBackendHost": "127.0.0.1:4732",
    "remoteBackendToken": null,
    "defaultAccessMode": "current",
    "uiScale": 1.0,
    "theme": "system"
  }
}
```

**Notes**

- Before returning, the daemon overlays experimental feature flags from `$CODEX_HOME/config.toml` (collab/steer/unified_exec).



### `update_app_settings`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `settings` | `AppSettings` | yes | Full settings blob to persist. |


**Response**

AppSettings (echoed)


**Example**

```json
{
  "id": 1,
  "method": "update_app_settings",
  "params": {
    "settings": {}
  }
}
```
```json
{
  "id": 19,
  "result": {
    "codexBin": "/usr/local/bin/codex",
    "backendMode": "remote",
    "remoteBackendHost": "127.0.0.1:4732",
    "remoteBackendToken": "***",
    "defaultAccessMode": "current"
  }
}
```

**Notes**

- Persists to `<data-dir>/settings.json`.

- Also writes experimental feature flags to `$CODEX_HOME/config.toml` via `codex_config` helpers.



### `codex_doctor`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `codexBin` | `string|null` | no | Optional override for the codex binary to probe (defaults to settings.codex_bin or `codex`). |


**Response**

CodexDoctorResult (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "codex_doctor",
  "params": {}
}
```
```json
{
  "id": 20,
  "result": {
    "ok": true,
    "codexBin": "codex",
    "version": "...",
    "appServerOk": true,
    "details": null,
    "path": "/usr/local/bin:...",
    "nodeOk": true,
    "nodeVersion": "v20.11.0",
    "nodeDetails": null
  }
}
```

**Notes**

- Runs `codex --version` and `codex app-server --help` with timeouts to validate install.

- Also checks `node --version` (Codex depends on Node).




---

## Commit message helpers

### `get_commit_message_prompt`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

string (prompt text)


**Example**

```json
{
  "id": 1,
  "method": "get_commit_message_prompt",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 21,
  "result": "You are a helpful assistant..."
}
```

**Notes**

- Builds a prompt based on `git diff` / workspace diff for commit message generation.



### `generate_commit_message`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

string (generated commit message)


**Example**

```json
{
  "id": 1,
  "method": "generate_commit_message",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 22,
  "result": "feat: improve prompt parsing\\n\\n- ..."
}
```

**Notes**

- Creates a temporary Codex thread, streams assistant deltas internally, and archives the thread when done.

- These streaming events are suppressed from broadcast so they do not pollute connected clients’ conversations.




---

## Threads & turns (Codex app-server)

### `start_thread`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |


**Response**

Codex app-server response envelope for `thread/new` (often contains `result.thread`).


**Example**

```json
{
  "id": 1,
  "method": "start_thread",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 23,
  "result": {
    "id": 1,
    "result": {
      "thread": {
        "id": "t1",
        "title": "New thread",
        "createdAt": "..."
      }
    }
  }
}
```

**Notes**

- The daemon returns the *raw Codex app-server response* as its `result`. Clients often unwrap `result.result`.



### `resume_thread`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `threadId` | `string` | yes | Thread id to resume. |


**Response**

Codex app-server response envelope for `thread/resume`.


**Example**

```json
{
  "id": 1,
  "method": "resume_thread",
  "params": {
    "workspaceId": "...",
    "threadId": "..."
  }
}
```
```json
{
  "id": 24,
  "result": {
    "id": 2,
    "result": {
      "thread": {
        "id": "t1",
        "...": true
      }
    }
  }
}
```

**Notes**

- Resuming a thread causes Codex to replay persisted conversation items; clients refresh local cache accordingly.



### `list_threads`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `cursor` | `string|null` | no | Optional pagination cursor. |
| `limit` | `number|null` | no | Optional max items (u32). |


**Response**

Codex app-server response envelope for `thread/list`.


**Example**

```json
{
  "id": 1,
  "method": "list_threads",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 25,
  "result": {
    "id": 3,
    "result": {
      "threads": [
        {
          "id": "t1",
          "title": "..."
        }
      ],
      "nextCursor": null
    }
  }
}
```

**Notes**

- The desktop filters threads by `cwd` to show only threads for the workspace (see README / desktop hooks).



### `archive_thread`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `threadId` | `string` | yes | Thread id to archive. |


**Response**

Codex app-server response envelope for `thread/archive`.


**Example**

```json
{
  "id": 1,
  "method": "archive_thread",
  "params": {
    "workspaceId": "...",
    "threadId": "..."
  }
}
```
```json
{
  "id": 26,
  "result": {
    "id": 4,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- The daemon only supports archiving (no unarchive endpoint).



### `send_user_message`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `threadId` | `string` | yes | Thread id. |
| `text` | `string` | yes | User message text (may be empty if images provided). |
| `model` | `string|null` | no | Optional model override. |
| `effort` | `string|null` | no | Optional reasoning effort (Codex-specific). |
| `accessMode` | `string|null` | no | One of: `current`, `read-only`, `full-access` (default current). |
| `images` | `string[]|null` | no | Optional images. Each string may be a `data:` URL, `http(s)` URL, or a local filesystem path (desktop). |
| `collaborationMode` | `any|null` | no | Optional collaboration mode payload forwarded to Codex. |


**Response**

Codex app-server response envelope for `turn/start`.


**Example**

```json
{
  "id": 1,
  "method": "send_user_message",
  "params": {
    "workspaceId": "...",
    "threadId": "...",
    "text": "..."
  }
}
```
```json
{
  "id": 27,
  "result": {
    "id": 5,
    "result": {
      "turnId": "turn_1",
      "status": "started"
    }
  }
}
```

**Notes**

- The main content is streamed back via `app-server-event` notifications; this call’s response is just the initial app-server reply.

- If both `text` and `images` are empty, returns `empty user message`.

- iOS currently sends image attachments as `data:image/...;base64,...` strings.



### `turn_interrupt`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `threadId` | `string` | yes | Thread id. |
| `turnId` | `string` | yes | Turn id to interrupt. |


**Response**

Codex app-server response envelope for `turn/interrupt`.


**Example**

```json
{
  "id": 1,
  "method": "turn_interrupt",
  "params": {
    "workspaceId": "...",
    "threadId": "...",
    "turnId": "..."
  }
}
```
```json
{
  "id": 28,
  "result": {
    "id": 6,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- Used to stop an in-flight assistant turn.



### `start_review`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `threadId` | `string` | yes | Thread id. |
| `target` | `any` | yes | Review target object forwarded to Codex (see ReviewTarget in DATA_MODELS.md). |
| `delivery` | `string|null` | no | Optional delivery hint (Codex-specific). |


**Response**

Codex app-server response envelope for `review/start`.


**Example**

```json
{
  "id": 1,
  "method": "start_review",
  "params": {
    "workspaceId": "...",
    "threadId": "...",
    "target": {}
  }
}
```
```json
{
  "id": 29,
  "result": {
    "id": 7,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- The daemon does not validate `target` beyond requiring it to exist; it is forwarded to Codex unchanged.



### `model_list`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |


**Response**

Codex app-server response envelope for `model/list`.


**Example**

```json
{
  "id": 1,
  "method": "model_list",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 30,
  "result": {
    "id": 8,
    "result": {
      "models": [
        {
          "id": "gpt-4.1",
          "label": "GPT‑4.1"
        }
      ]
    }
  }
}
```

**Notes**

- Exact payload is determined by Codex.



### `collaboration_mode_list`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

Codex app-server response envelope for `collaborationMode/list`.


**Example**

```json
{
  "id": 1,
  "method": "collaboration_mode_list",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 31,
  "result": {
    "id": 9,
    "result": {
      "modes": [
        {
          "id": "solo",
          "label": "Solo"
        }
      ]
    }
  }
}
```

**Notes**

- Only meaningful when Codex feature flag `features.collab` is enabled.



### `account_rate_limits`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

Codex app-server response envelope for `account/rateLimits`.


**Example**

```json
{
  "id": 1,
  "method": "account_rate_limits",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 32,
  "result": {
    "id": 10,
    "result": {
      "snapshot": {
        "requestsRemaining": 123
      }
    }
  }
}
```

**Notes**

- Streaming updates may also arrive via `account/rateLimits/updated` events.



### `skills_list`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

Codex app-server response envelope for `skills/list`.


**Example**

```json
{
  "id": 1,
  "method": "skills_list",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 33,
  "result": {
    "id": 11,
    "result": {
      "skills": [
        {
          "name": "format",
          "description": "..."
        }
      ]
    }
  }
}
```

**Notes**

- Used for `$skill` autocomplete in the composer.



### `respond_to_server_request`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `requestId` | `number` | yes | Codex request id to respond to (u64). |
| `result` | `any` | yes | Response payload to send as the request result. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "respond_to_server_request",
  "params": {
    "workspaceId": "...",
    "requestId": 1,
    "result": {}
  }
}
```
```json
{
  "id": 34,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Used to respond to Codex-initiated requests (commonly approvals).

- This writes a response message directly to Codex stdin (`{{"id":...,"result":...}}`).



### `remember_approval_rule`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `command` | `string[]` | yes | Command argv to remember as an allowed prefix rule. |


**Response**

{ ok: true, rulesPath: string }


**Example**

```json
{
  "id": 1,
  "method": "remember_approval_rule",
  "params": {
    "workspaceId": "...",
    "command": [
      "..."
    ]
  }
}
```
```json
{
  "id": 35,
  "result": {
    "ok": true,
    "rulesPath": "/Users/me/.codex/rules/default.rules"
  }
}
```

**Notes**

- Appends a prefix rule to the Codex rules file under CODEX_HOME (`rules::append_prefix_rule`).

- This is security-sensitive: it changes which commands Codex can execute without prompting.




---

## Git

### `list_git_roots`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `depth` | `number|null` | no | Optional scan depth (defaults 2, clamped 1..6). |


**Response**

string[] (absolute paths to nested git repos under the workspace root)


**Example**

```json
{
  "id": 1,
  "method": "list_git_roots",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 40,
  "result": [
    "/Users/me/Repo",
    ".",
    "/Users/me/Repo/packages/pkg-a"
  ]
}
```

**Notes**

- Used by UI to select which git root to operate on when multiple repos exist.



### `get_git_status`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

GitStatusResponse (Swift) / GitStatusState (desktop) — see DATA_MODELS.md


**Example**

```json
{
  "id": 1,
  "method": "get_git_status",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 41,
  "result": {
    "branchName": "main",
    "files": [],
    "stagedFiles": [],
    "unstagedFiles": [],
    "totalAdditions": 0,
    "totalDeletions": 0
  }
}
```

**Notes**

- Uses libgit2 status API and computes simple addition/deletion totals.



### `get_git_diffs`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

GitFileDiff[] (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_git_diffs",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 42,
  "result": [
    {
      "path": "src/main.rs",
      "status": "modified",
      "additions": 3,
      "deletions": 1,
      "hunks": [
        {
          "header": "@@ ...",
          "lines": [
            "+foo",
            "-bar"
          ]
        }
      ]
    }
  ]
}
```

**Notes**

- Returns per-file diffs for the working tree (staged/unstaged depending on implementation).



### `get_git_log`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `limit` | `number|null` | no | Optional max commits to return. |


**Response**

GitLogResponse (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_git_log",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 43,
  "result": {
    "entries": [
      {
        "sha": "...",
        "subject": "...",
        "authorName": "...",
        "authorEmail": "...",
        "timestamp": 0
      }
    ]
  }
}
```

**Notes**

- Uses libgit2 to walk history from HEAD.



### `get_git_commit_diff`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `sha` | `string` | yes | Commit SHA. |


**Response**

GitFileDiff[] (diff for a specific commit)


**Example**

```json
{
  "id": 1,
  "method": "get_git_commit_diff",
  "params": {
    "workspaceId": "...",
    "sha": "..."
  }
}
```
```json
{
  "id": 44,
  "result": [
    {
      "path": "README.md",
      "status": "modified",
      "additions": 1,
      "deletions": 0,
      "hunks": [
        "..."
      ]
    }
  ]
}
```

**Notes**

- Computes diffs between the commit and its parent (implementation-specific).



### `get_git_remote`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

string|null (remote URL)


**Example**

```json
{
  "id": 1,
  "method": "get_git_remote",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 45,
  "result": "git@github.com:org/repo.git"
}
```

**Notes**

- Returns the URL of `origin` if present, otherwise the first remote.



### `stage_git_file`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | File path relative to repo root. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "stage_git_file",
  "params": {
    "workspaceId": "...",
    "path": "..."
  }
}
```
```json
{
  "id": 50,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Stages a single file.



### `stage_git_all`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "stage_git_all",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 51,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Stages all changes.



### `unstage_git_file`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | File path relative to repo root. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "unstage_git_file",
  "params": {
    "workspaceId": "...",
    "path": "..."
  }
}
```
```json
{
  "id": 50,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Unstages a single file.



### `revert_git_file`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | File path relative to repo root. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "revert_git_file",
  "params": {
    "workspaceId": "...",
    "path": "..."
  }
}
```
```json
{
  "id": 50,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Reverts a single file in the working tree.



### `revert_git_all`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "revert_git_all",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 51,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Reverts all unstaged changes.



### `commit_git`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `message` | `string` | yes | Commit message. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "commit_git",
  "params": {
    "workspaceId": "...",
    "message": "..."
  }
}
```
```json
{
  "id": 52,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Creates a commit of staged changes using libgit2.

- Does not push; use `push_git`.



### `pull_git`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "pull_git",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 53,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Runs a git pull for the repo.



### `push_git`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "push_git",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 53,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Runs a git push for the repo.



### `sync_git`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "sync_git",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 53,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Convenience: pull then push (implementation-specific).



### `list_git_branches`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

{ branches: BranchInfo[] } (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "list_git_branches",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 54,
  "result": {
    "branches": [
      {
        "name": "main",
        "last_commit": 1700000000
      }
    ]
  }
}
```

**Notes**

- Returns local branches sorted by most recent commit time.



### `checkout_git_branch`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `name` | `string` | yes | Branch name. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "checkout_git_branch",
  "params": {
    "workspaceId": "...",
    "name": "..."
  }
}
```
```json
{
  "id": 55,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Checks out an existing local branch.



### `create_git_branch`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `name` | `string` | yes | New branch name. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "create_git_branch",
  "params": {
    "workspaceId": "...",
    "name": "..."
  }
}
```
```json
{
  "id": 55,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Creates a new local branch from HEAD and checks it out (implementation-specific).




---

## GitHub (via gh CLI)

### `get_github_issues`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (used to determine repo remote). |


**Response**

GitHubIssue[] (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_github_issues",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 60,
  "result": [
    {
      "number": 1,
      "title": "Bug",
      "state": "open",
      "url": "...",
      "author": {
        "login": "me"
      }
    }
  ]
}
```

**Notes**

- Uses `gh` CLI; requires `gh auth login` on the daemon host.

- Repo is inferred from the git remote URL.



### `get_github_pull_requests`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

GitHubPullRequest[] (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_github_pull_requests",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 61,
  "result": [
    {
      "number": 12,
      "title": "PR",
      "state": "open",
      "url": "...",
      "author": {
        "login": "me"
      },
      "baseRefName": "main",
      "headRefName": "feature"
    }
  ]
}
```

**Notes**

- Uses `gh` CLI.



### `get_github_pull_request_diff`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `prNumber` | `number` | yes | Pull request number. |


**Response**

GitHubPullRequestDiffResponse (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_github_pull_request_diff",
  "params": {
    "workspaceId": "...",
    "prNumber": 1
  }
}
```
```json
{
  "id": 62,
  "result": {
    "diff": "..."
  }
}
```

**Notes**

- Uses `gh pr diff` / GitHub API via gh.



### `get_github_pull_request_comments`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `prNumber` | `number` | yes | Pull request number. |


**Response**

GitHubPullRequestCommentsResponse (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "get_github_pull_request_comments",
  "params": {
    "workspaceId": "...",
    "prNumber": 1
  }
}
```
```json
{
  "id": 62,
  "result": {
    "diff": "..."
  }
}
```

**Notes**

- Uses `gh api` to fetch review comments.




---

## Prompts

### `prompts_list`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (used for workspace-scoped prompts). |


**Response**

CustomPromptOption[]


**Example**

```json
{
  "id": 1,
  "method": "prompts_list",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 70,
  "result": [
    {
      "name": "Bugfix",
      "path": "/.../Bugfix.md",
      "description": "...",
      "content": "...",
      "scope": "workspace"
    }
  ]
}
```

**Notes**

- Loads both global prompts from `$CODEX_HOME/prompts` and workspace prompts from `<data-dir>/workspaces/<id>/prompts`.



### `prompts_create`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `scope` | `string` | yes | `global` or `workspace`. |
| `name` | `string` | yes | Prompt name (filename). |
| `description` | `string|null` | no | Optional description. |
| `argumentHint` | `string|null` | no | Optional argument hint. |
| `content` | `string` | yes | Prompt body content. |


**Response**

CustomPromptOption (created)


**Example**

```json
{
  "id": 1,
  "method": "prompts_create",
  "params": {
    "workspaceId": "...",
    "scope": "...",
    "name": "...",
    "content": "..."
  }
}
```
```json
{
  "id": 71,
  "result": {
    "name": "MyPrompt",
    "path": "/.../MyPrompt.md",
    "content": "...",
    "scope": "global"
  }
}
```

**Notes**

- Persists prompt as a file. Description and argumentHint may be stored as frontmatter/metadata (see prompts module).



### `prompts_update`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | Existing prompt file path. |
| `name` | `string` | yes | Updated name. |
| `description` | `string|null` | no | Updated description. |
| `argumentHint` | `string|null` | no | Updated argument hint. |
| `content` | `string` | yes | Updated prompt content. |


**Response**

CustomPromptOption (updated)


**Example**

```json
{
  "id": 1,
  "method": "prompts_update",
  "params": {
    "workspaceId": "...",
    "path": "...",
    "name": "...",
    "content": "..."
  }
}
```
```json
{
  "id": 72,
  "result": {
    "name": "MyPrompt",
    "path": "/.../MyPrompt.md",
    "content": "...",
    "scope": "workspace"
  }
}
```

**Notes**

- May rename the underlying file if `name` changes.



### `prompts_delete`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | Prompt file path to delete. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "prompts_delete",
  "params": {
    "workspaceId": "...",
    "path": "..."
  }
}
```
```json
{
  "id": 73,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Deletes the prompt file from disk.



### `prompts_move`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |
| `path` | `string` | yes | Prompt file path. |
| `scope` | `string` | yes | `global` or `workspace` (target scope). |


**Response**

CustomPromptOption (moved)


**Example**

```json
{
  "id": 1,
  "method": "prompts_move",
  "params": {
    "workspaceId": "...",
    "path": "...",
    "scope": "..."
  }
}
```
```json
{
  "id": 74,
  "result": {
    "name": "MyPrompt",
    "path": "/new/path/MyPrompt.md",
    "scope": "global",
    "content": "..."
  }
}
```

**Notes**

- Moves a prompt between workspace and global directories.



### `prompts_workspace_dir`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id. |


**Response**

string (absolute directory path)


**Example**

```json
{
  "id": 1,
  "method": "prompts_workspace_dir",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 75,
  "result": "/.../<data-dir>/workspaces/<id>/prompts"
}
```

**Notes**

- Returns the workspace prompts folder path on the daemon host.



### `prompts_global_dir`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

_No params._


**Response**

string (absolute directory path)


**Example**

```json
{
  "id": 1,
  "method": "prompts_global_dir"
}
```
```json
{
  "id": 76,
  "result": "/Users/me/.codex/prompts"
}
```

**Notes**

- Returns the global prompts folder under CODEX_HOME.




---

## Terminal

### `terminal_open`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (cwd for the shell). |
| `terminalId` | `string` | yes | Client-chosen identifier for this terminal tab. |
| `cols` | `number` | yes | Initial terminal columns (u32, clamped to u16::MAX). |
| `rows` | `number` | yes | Initial terminal rows (u32, clamped to u16::MAX). |


**Response**

{ id: string } (TerminalSessionInfo)


**Example**

```json
{
  "id": 1,
  "method": "terminal_open",
  "params": {
    "workspaceId": "...",
    "terminalId": "...",
    "cols": 1,
    "rows": 1
  }
}
```
```json
{
  "id": 80,
  "result": {
    "id": "term-1"
  }
}
```

**Notes**

- Spawns a PTY shell (uses `$SHELL` if available, otherwise a default).

- Output is streamed back via `terminal-output` notifications.



### `terminal_write`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `terminalId` | `string` | yes | Terminal session id. |
| `data` | `string` | yes | Bytes to write (usually UTF-8). |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "terminal_write",
  "params": {
    "terminalId": "...",
    "data": "..."
  }
}
```
```json
{
  "id": 81,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Writes raw data to the PTY writer.



### `terminal_resize`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `terminalId` | `string` | yes | Terminal session id. |
| `cols` | `number` | yes | Columns. |
| `rows` | `number` | yes | Rows. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "terminal_resize",
  "params": {
    "terminalId": "...",
    "cols": 1,
    "rows": 1
  }
}
```
```json
{
  "id": 82,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Resizes the PTY. Values are clamped to u16::MAX.



### `terminal_close`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `terminalId` | `string` | yes | Terminal session id. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "terminal_close",
  "params": {
    "terminalId": "..."
  }
}
```
```json
{
  "id": 83,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Kills the PTY child process and removes the session from memory.




---

## Local usage

### `local_usage_snapshot`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `days` | `number|null` | no | How many past days to scan (default 7). |
| `workspacePath` | `string|null` | no | Optional workspace path filter. |


**Response**

LocalUsageSnapshot (see DATA_MODELS.md)


**Example**

```json
{
  "id": 1,
  "method": "local_usage_snapshot",
  "params": {}
}
```
```json
{
  "id": 90,
  "result": {
    "days": [],
    "totals": {
      "requests": 0,
      "inputTokens": 0,
      "outputTokens": 0
    }
  }
}
```

**Notes**

- Scans Codex session JSONL logs under `$CODEX_HOME/sessions/...`.

- Purely local; does not require a connected workspace session.




---

## Memory

### `memory_flush_now`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `threadId` | `string` | yes | Thread id to flush memory for. |


**Response**

Codex app-server response envelope for `memory/flushNow`.


**Example**

```json
{
  "id": 1,
  "method": "memory_flush_now",
  "params": {
    "workspaceId": "...",
    "threadId": "..."
  }
}
```
```json
{
  "id": 100,
  "result": {
    "id": 1,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- Manually triggers a memory flush for the specified thread.

- Useful when you want to persist conversation context immediately rather than waiting for automatic flush.



---

## Browser

### `browser_create_session`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |


**Response**

Codex app-server response envelope for `browser/createSession`.


**Example**

```json
{
  "id": 1,
  "method": "browser_create_session",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 101,
  "result": {
    "id": 1,
    "result": {
      "sessionId": "browser-session-abc123"
    }
  }
}
```

**Notes**

- Creates a new browser automation session.

- Returns a session ID to use with other browser methods.



### `browser_list_sessions`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |


**Response**

Codex app-server response envelope for `browser/listSessions`.


**Example**

```json
{
  "id": 1,
  "method": "browser_list_sessions",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 102,
  "result": {
    "id": 1,
    "result": {
      "sessions": [
        {
          "sessionId": "browser-session-abc123",
          "url": "https://example.com"
        }
      ]
    }
  }
}
```

**Notes**

- Lists all active browser sessions for the workspace.



### `browser_close_session`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id to close. |


**Response**

Codex app-server response envelope for `browser/closeSession`.


**Example**

```json
{
  "id": 1,
  "method": "browser_close_session",
  "params": {
    "workspaceId": "...",
    "sessionId": "..."
  }
}
```
```json
{
  "id": 103,
  "result": {
    "id": 1,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- Closes and cleans up a browser session.



### `browser_navigate`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |
| `url` | `string` | yes | URL to navigate to. |


**Response**

Codex app-server response envelope for `browser/navigate`.


**Example**

```json
{
  "id": 1,
  "method": "browser_navigate",
  "params": {
    "workspaceId": "...",
    "sessionId": "...",
    "url": "https://example.com"
  }
}
```
```json
{
  "id": 104,
  "result": {
    "id": 1,
    "result": {
      "ok": true,
      "url": "https://example.com"
    }
  }
}
```

**Notes**

- Navigates the browser session to the specified URL.



### `browser_screenshot`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |


**Response**

Codex app-server response envelope for `browser/screenshot`.


**Example**

```json
{
  "id": 1,
  "method": "browser_screenshot",
  "params": {
    "workspaceId": "...",
    "sessionId": "..."
  }
}
```
```json
{
  "id": 105,
  "result": {
    "id": 1,
    "result": {
      "data": "data:image/png;base64,..."
    }
  }
}
```

**Notes**

- Takes a screenshot of the current browser page.

- Returns a base64-encoded image.



### `browser_click`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |
| `x` | `number` | no | X coordinate (pixels). Required if selector not provided. |
| `y` | `number` | no | Y coordinate (pixels). Required if selector not provided. |
| `selector` | `string` | no | CSS selector to click. Alternative to x/y coordinates. |


**Response**

Codex app-server response envelope for `browser/click`.


**Example**

```json
{
  "id": 1,
  "method": "browser_click",
  "params": {
    "workspaceId": "...",
    "sessionId": "...",
    "x": 100,
    "y": 200
  }
}
```
```json
{
  "id": 106,
  "result": {
    "id": 1,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- Clicks at the specified coordinates or CSS selector.

- Either x/y or selector must be provided.



### `browser_type`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |
| `text` | `string` | yes | Text to type. |


**Response**

Codex app-server response envelope for `browser/type`.


**Example**

```json
{
  "id": 1,
  "method": "browser_type",
  "params": {
    "workspaceId": "...",
    "sessionId": "...",
    "text": "Hello, world!"
  }
}
```
```json
{
  "id": 107,
  "result": {
    "id": 1,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- Types the specified text into the currently focused element.



### `browser_press`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |
| `key` | `string` | yes | Key to press (e.g., "Enter", "Tab", "Escape", "ArrowDown"). |


**Response**

Codex app-server response envelope for `browser/press`.


**Example**

```json
{
  "id": 1,
  "method": "browser_press",
  "params": {
    "workspaceId": "...",
    "sessionId": "...",
    "key": "Enter"
  }
}
```
```json
{
  "id": 108,
  "result": {
    "id": 1,
    "result": {
      "ok": true
    }
  }
}
```

**Notes**

- Presses a keyboard key.

- Common keys: Enter, Tab, Escape, Backspace, ArrowUp, ArrowDown, ArrowLeft, ArrowRight.



### `browser_snapshot`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |


**Response**

Codex app-server response envelope for `browser/snapshot`.


**Example**

```json
{
  "id": 1,
  "method": "browser_snapshot",
  "params": {
    "workspaceId": "...",
    "sessionId": "..."
  }
}
```
```json
{
  "id": 109,
  "result": {
    "id": 1,
    "result": {
      "screenshot": "data:image/png;base64,...",
      "dom": "<html>...</html>",
      "url": "https://example.com"
    }
  }
}
```

**Notes**

- Gets a full page snapshot including screenshot and DOM content.

- Useful for understanding the current page state.



### `browser_evaluate`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `sessionId` | `string` | yes | Browser session id. |
| `script` | `string` | yes | JavaScript code to execute. |


**Response**

Codex app-server response envelope for `browser/evaluate`.


**Example**

```json
{
  "id": 1,
  "method": "browser_evaluate",
  "params": {
    "workspaceId": "...",
    "sessionId": "...",
    "script": "document.title"
  }
}
```
```json
{
  "id": 110,
  "result": {
    "id": 1,
    "result": {
      "value": "Example Page Title"
    }
  }
}
```

**Notes**

- Executes JavaScript in the browser context and returns the result.

- The script runs in the page's execution context.



---

## Skills (Extended)

### `skills_config_write`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |
| `config` | `object` | yes | Configuration object with `enabled` and `disabled` arrays. |

The `config` object structure:

```json
{
  "enabled": [{ "name": "skill-name", "path": "/path/to/skill" }],
  "disabled": [{ "name": "other-skill", "path": "/path/to/other" }]
}
```


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "skills_config_write",
  "params": {
    "workspaceId": "...",
    "config": {
      "enabled": [{ "name": "format", "path": "/path/to/format" }],
      "disabled": []
    }
  }
}
```
```json
{
  "id": 111,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Writes the skills configuration to `{CODEX_HOME}/skills/config.json`.
- Config specifies which skills are enabled and which are disabled.
- Skills not in either list default to enabled.



### `skills_validate`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |


**Response**

Array of `SkillValidationResult` objects.


**Example**

```json
{
  "id": 1,
  "method": "skills_validate",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 112,
  "result": [
    {
      "name": "format",
      "path": "/path/to/format",
      "description": "Code formatting skill",
      "issues": []
    },
    {
      "name": "browser-tool",
      "path": "/path/to/browser-tool",
      "description": "Browser automation",
      "issues": ["missing binary: playwright", "missing env var: BROWSER_PATH"]
    }
  ]
}
```

**Notes**

- Validates all installed skills in the workspace.

- Each result includes the skill name, path, description, and array of issues.

- Issues include: missing binaries, missing env vars, unsupported OS.

- Empty `issues` array means the skill is valid and ready to use.



### `skills_install_from_git`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `sourceUrl` | `string` | yes | Git repository URL containing the skill. |
| `target` | `string` | yes | Installation target: `"global"` or `"workspace"`. |
| `workspaceId` | `string` | conditional | Required when `target` is `"workspace"`. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "skills_install_from_git",
  "params": {
    "sourceUrl": "https://github.com/example/my-skill.git",
    "target": "workspace",
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 113,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Clones the git repository into the skills directory.

- `"global"` target installs to `$CODEX_HOME/skills/`.

- `"workspace"` target installs to `<workspace>/.codex/skills/`.

- The repository should contain a `SKILL.md` file at its root.



### `skills_uninstall`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `name` | `string` | yes | Name of the skill (repository folder name). |
| `target` | `string` | yes | Uninstall target: `"global"` or `"workspace"`. |
| `workspaceId` | `string` | conditional | Required when `target` is `"workspace"`. |


**Response**

{ ok: true }


**Example**

```json
{
  "id": 1,
  "method": "skills_uninstall",
  "params": {
    "name": "my-skill",
    "target": "workspace",
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 114,
  "result": {
    "ok": true
  }
}
```

**Notes**

- Removes an installed skill by deleting its directory.

- `"global"` target removes from `$CODEX_HOME/skills/`.

- `"workspace"` target removes from `<workspace>/.codex/skills/`.

- Only works for user-installed skills, not built-in skills.



### `skills_config_read`

- **Direction:** client → daemon
- **Auth required:** yes


**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Workspace id (must be connected). |


**Response**

Skills configuration object.


**Example**

```json
{
  "id": 1,
  "method": "skills_config_read",
  "params": {
    "workspaceId": "..."
  }
}
```
```json
{
  "id": 115,
  "result": {
    "enabled": [
      { "name": "format", "path": "/path/to/format" }
    ],
    "disabled": [
      { "name": "browser-tool", "path": "/path/to/browser-tool" }
    ]
  }
}
```

**Notes**

- Reads the skills configuration from `{CODEX_HOME}/skills/config.json`.

- Returns empty arrays if no config file exists.

- Used by UI to determine which skills are enabled/disabled.


---

## Server → client notifications

Once authenticated, a client connection subscribes to a broadcast stream.

### `app-server-event`

- **Direction:** daemon → client (notification)
- **Auth required:** yes (subscription happens only after auth)
- **Params:** `AppServerEvent`:
  - `workspace_id: string`
  - `message: any` (raw message from Codex app-server)

Example:

```json
{
  "method": "app-server-event",
  "params": {
    "workspace_id": "w1",
    "message": {
      "method": "item/agentMessage/delta",
      "params": {
        "delta": "Hello"
      }
    }
  }
}
```

Notes:
- `message` is forwarded unmodified from Codex. The clients interpret it (see `docs/DESKTOP_APP.md` and `docs/IOS_CLIENT.md`).


### `terminal-output`

- **Direction:** daemon → client (notification)
- **Auth required:** yes
- **Params:** `TerminalOutput`:
  - `workspaceId: string`
  - `terminalId: string`
  - `data: string`

Example:

```json
{
  "method": "terminal-output",
  "params": {
    "workspaceId": "w1",
    "terminalId": "term-1",
    "data": "ls\\r\\n"
  }
}
```

Notes:
- This is raw PTY output. Clients are responsible for emulation/rendering.

---

## Browser (Updated 2026-01-26)

All browser methods are daemon RPCs that proxy to the Playwright worker. No `workspaceId` required.

### `browser_create_session`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `headless` | `boolean` | no | Defaults true |
| `viewport` | `object` | no | `{ width, height }` |
| `userDataDir` | `string` | no | Persistent profile dir |
| `startUrl` | `string` | no | Optional initial URL |

**Response**

```json
{ "sessionId": "b-..." }
```

### `browser_list_sessions`

**Response**

```json
{ "sessions": ["b-..."] }
```

### `browser_navigate`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `sessionId` | `string` | yes | Session id |
| `url` | `string` | yes | URL |
| `waitUntil` | `string` | no | `load`/`domcontentloaded`/`networkidle` |
| `timeoutMs` | `number` | no | Timeout |

### `browser_screenshot`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `sessionId` | `string` | yes | Session id |
| `fullPage` | `boolean` | no | Full page screenshot |

**Response**

```json
{ "base64Png": "...", "url": "...", "title": "...", "width": 1280, "height": 720 }
```

### `browser_click`, `browser_type`, `browser_press`, `browser_evaluate`, `browser_snapshot`, `browser_close_session`

See worker protocol in `browser-worker/src/index.ts` for exact params.

---

## Skills (Updated 2026-01-26)

### `skills_config_write`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Connected workspace |
| `config` | `object` | yes | Passed to `skills/config/write` |

### `skills_validate`

**Response**

```json
[{ "name": "...", "path": "...", "issues": [], "description": "..." }]
```

### `skills_install_from_git`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `sourceUrl` | `string` | yes | Git URL |
| `target` | `string` | yes | `global` or `workspace` |
| `workspaceId` | `string` | no | Required for `workspace` target |

### `skills_uninstall`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `name` | `string` | yes | Repo folder name |
| `target` | `string` | yes | `global` or `workspace` |
| `workspaceId` | `string` | no | Required for `workspace` target |

### `skills_config_read`

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `workspaceId` | `string` | yes | Connected workspace |

**Response**

```json
{ "enabled": [{"name":"...","path":"..."}], "disabled": [] }
```

---

## Global Configuration Files (2026-01-28)

### `read_global_agents_md`

Read the global AGENTS.md file from `~/.codex/AGENTS.md`.

**Request params**

_None_

**Response**

```json
{
  "exists": true,
  "content": "# My Agents\n...",
  "truncated": false
}
```

| Field | Type | Description |
|------|------|-------------|
| `exists` | `boolean` | Whether the file exists on disk |
| `content` | `string` | File contents (empty if not exists) |
| `truncated` | `boolean` | Whether content was truncated |

---

### `write_global_agents_md`

Write the global AGENTS.md file to `~/.codex/AGENTS.md`.

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `content` | `string` | yes | File contents to write |

**Response**

_None (empty object on success)_

---

### `read_global_config_toml`

Read the global Codex config.toml file from `~/.codex/config.toml`.

**Request params**

_None_

**Response**

```json
{
  "exists": true,
  "content": "[features]\n...",
  "truncated": false
}
```

| Field | Type | Description |
|------|------|-------------|
| `exists` | `boolean` | Whether the file exists on disk |
| `content` | `string` | File contents (empty if not exists) |
| `truncated` | `boolean` | Whether content was truncated |

---

### `write_global_config_toml`

Write the global Codex config.toml file to `~/.codex/config.toml`.

**Request params**

| Field | Type | Required | Description |
|------|------|----------|-------------|
| `content` | `string` | yes | File contents to write |

**Response**

_None (empty object on success)_
