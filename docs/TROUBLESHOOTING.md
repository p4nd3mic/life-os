# TROUBLESHOOTING.md — Common Issues & Fixes

This is the “what’s broken and how do I unbreak it” guide for Life OS.

> Cross‑refs:
> - **[GOTCHAS.md](GOTCHAS.md)** — the sneaky stuff (naming, nested results, env vars, timezones)
> - **[DEPLOYMENT.md](DEPLOYMENT.md)** — setup, launchd, MCP registration
> - **[MCP_INTEGRATION.md](MCP_INTEGRATION.md)** — bridge + parsing details
> - **[STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)** — where state is stored and how it flows

---

## First 60 seconds checklist ✅

Before digging deep:

1) **Are the paths valid?**
- `LIFE_OS_PATH` points to your `life-os/` root
- `LIFE_MCP_PATH` points to your `life-mcp/` repo (or `index.js`)

2) **Do the basic processes work?**
- `node -v` works
- `codex app-server --help` works
- If using iOS: daemon is listening on `127.0.0.1:4732`

3) **Can you reproduce with logs visible?**
- Run the desktop app via `npm run tauri dev` so Rust + Node logs show in your terminal
- If using launchd: tail the log file(s)

---

## Quick symptom → cause → fix table

| Symptom | Most likely cause | Fix |
|---|---|---|
| “MCP server not responding” | life-mcp not spawning, wrong `LIFE_MCP_PATH`, Node missing | See **Connection → MCP** below |
| “Codex connection failed” | `codex` not installed, wrong `codexBin`, `codex app-server` fails in workspace | See **Connection → Codex** below |
| “Workspace not found” | stale workspace id / deleted entry | Refresh workspaces; check app data `workspaces.json` |
| Cards blank / not rendering | store not hydrated or UI rendering guard tripped | Check stream store + devtools |
| Images not loading | missing `TMDB_API_KEY`, fetch failed, cache path wrong | See **UI → Images** |
| “Tool not found” | Rust whitelist vs Node tool registry mismatch | Update registry + parsers (see **Data issues**) |
| Wrong timezone day boundary | Pacific-time assumptions | See **Data issues → timezone** |
| Duplicate cards | patch merging / stale persistence | See **Data issues → duplicates** |
| Tauri build fails | Rust toolchain / native deps missing | Run `npm run doctor`, install deps |
| TS compile errors | TS ↔ Rust types drift | Update both + run lint/typecheck |

---

## Connection issues

### 1) “MCP server not responding” / tool calls hang

#### What’s happening
Life Stream uses a Rust bridge (`src-tauri/src/life_stream/mcp_bridge.rs`) that spawns Node:

```text
node <life-mcp>/index.js
```

…and then does JSON-RPC over stdio.

#### Fixes (in order)

1) **Confirm environment variables**
```bash
echo "$LIFE_OS_PATH"
echo "$LIFE_MCP_PATH"
```

2) **Confirm the life-mcp entry point exists**
If `LIFE_MCP_PATH` is a directory, the bridge will look for:
- `<dir>/dist/index.js` *(preferred if present)*
- otherwise `<dir>/index.js`

3) **Run life-mcp manually**
```bash
cd "$LIFE_MCP_PATH"
node index.js
```

Expected log:
- `Life MCP server running in stdio mode`

If it crashes:
- your `.env` is invalid / missing
- required secrets (Supabase, Sheets) are missing for the tool you invoked

4) **Check Node is reachable from the parent process**
If you start CodexMonitor via launchd, its PATH may be minimal.  
Fix by setting `PATH` explicitly in the launchd plist or using absolute paths.

5) **Look for timeouts**
The bridge uses request timeouts; if a tool runs long, you’ll see a timeout error.  
Reduce tool scope or optimize long-running tool queries.

---

### 2) “Codex connection failed” / app-server won’t initialize

#### What’s happening
CodexMonitor starts Codex with:

```text
codex app-server
```

and expects an `initialize` response within ~15 seconds. If not, it kills the process and surfaces:

> “Codex app-server did not respond to initialize. Check that `codex app-server` works in Terminal.”

#### Fixes

1) **Check Codex is installed**
```bash
codex app-server --help
```

2) **Check workspace validity**
CodexMonitor runs `codex app-server` with `current_dir` set to the workspace path.  
If the workspace directory is missing or permission-restricted, app-server can fail silently.

3) **Check configured codexBin**
If you set `codexBin` in settings/workspace settings, ensure it points to a valid executable.

4) **Watch stdout/stderr**
During dev (`npm run tauri dev`) the backend prints:
- `[app-server stdout] …`
- `[app-server stderr] …`

This usually reveals missing auth, permissions, or version mismatch issues.

---

### 3) “Workspace not found”

This typically means the UI is sending a workspace ID that doesn’t exist in the backend map.

Common causes:
- workspace deleted from disk
- workspace storage entry corrupted
- switching between Local/Remote backend with different workspace lists

Fixes:
- Use the UI to refresh or re-add the workspace
- Inspect app state storage (macOS typical location):
  - `~/Library/Application Support/<tauri-identifier>/workspaces.json`

> See **[STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)** for what gets stored where.

---

### 4) iOS/Remote backend can’t connect to Mac mini daemon

Symptoms:
- iOS shows disconnected state
- Remote backend mode in desktop app can’t reach host

Fixes:
1) Confirm daemon is listening locally:
```bash
lsof -iTCP:4732 -sTCP:LISTEN
```

2) If using Tailscale Serve, confirm:
```bash
tailscale serve status
```

3) Verify the token:
- Desktop daemon expects `--token` (or `CODEX_MONITOR_DAEMON_TOKEN`)
- iOS stores token in Keychain; re-enter if needed

4) Confirm the daemon host/port:
- default: `127.0.0.1:4732`
- over tailnet: `mac-mini.tailnet.ts.net:4732`

---

## UI issues

### Layout broken (massive right panel / grid weirdness)

Common culprits:
- persisted panel sizes in local storage / settings
- CSS grid changes between versions causing old stored sizes to look wrong

Fix:
- reset panel layout preferences:
  - clear local storage keys related to layout (search for “panel” in DevTools Application tab)
- restart the app

If you can reproduce, add a regression test in:
- `src/features/layout/hooks/useResizablePanels.test.ts`

---

### Desktop opens the wrong UI (old multi-workspace shell vs Life Stream shell)

Symptoms:
- app launches, but shows legacy multi-workspace layout
- missing day navigation / timeline card stream / Life composer

Most likely cause:
- desktop entrypoint drifted (`src/main.tsx`) to a different root than intended for current lane

Checks:
1) Confirm branch lane:
   - `codex/desktop-stable` for known-good desktop
   - `codex/ipad-webview-wip` for iPad WebView iteration
2) Confirm entrypoint in `src/main.tsx` matches the lane intent.
3) Rebuild from root with:
   ```bash
   just app
   ```

Notes:
- Keep desktop fixes and iPad WebView work separated by branch to avoid shell crossover.
- See **[BRANCH_LANES.md](BRANCH_LANES.md)** for lane mapping.

---

### Cards not rendering / stream view empty

Checks:
1) Confirm you are in Life OS mode (workspace purpose = life, or Life Stream route).
2) Confirm stream store has data:
   - `src/features/life-stream/state/streamStore.ts`
3) Confirm event wiring:
   - desktop: Tauri events → store updates
   - remote: daemon notifications → UI

Debug tip:
- Use React DevTools to inspect props/state for the stream view components.

> Note: This snapshot does **not** expose `window.__STREAM_STORE__` by default.  
> If you want that, you can add a DEV-only line like:
>
> ```ts
> // in a dev-only module
> (window as any).__STREAM_STORE__ = streamStore;
> ```

---

### Images not loading (TMDB, cache, display)

Common causes:
- `TMDB_API_KEY` missing for cover enrichment
- fetch failed (network)
- cache path permissions
- UI failing to resolve local file URL

Fixes:
1) Confirm you have an API key:
```bash
echo "$TMDB_API_KEY"
```

2) Confirm cache directory is writable:
- for desktop Tauri: app data dir under `~/Library/Application Support/<identifier>/`
- for daemon: the daemon `--data-dir`

3) Check backend logs for fetch errors.

4) If a cover exists on disk but UI won’t show it:
- confirm you’re using the Tauri file URL helper (e.g. `convertFileSrc` usage)

---

### Auto-fetch images doesn’t find expected local files

Symptoms:
- `🖼️ Fetch images (ask)` returns empty candidates
- external library files exist but don’t show in picker

Checks:
1) Verify external roots exist and are accessible:
```bash
ls -la "/Volumes/YouTube 4TB/photos"
ls -la "/Volumes/YouTube 4TB/Photos"
```

2) Confirm entity folder conventions match scanner expectations:
- `Photos/Media/<Title>/...`
- `Photos/Games/<Title>/...`
- `Photos/Books/<Title>/...`

3) If macOS prompts for external drive permission repeatedly:
- approve once for the signed app build
- avoid launching from unsigned/transient bundle paths between runs

4) For auto-apply mode, verify entity isn’t unresolved `general:*`:
- current guard skips generic entities in `auto_apply` to prevent bad catalog writes
- use ask-first mode, pick candidate manually once, then auto-fetch will reuse canonical entity mapping

---

## Data issues

### “Tool not found”

What it means:
- Rust side has a whitelist/registry of allowed tool names
- Node life-mcp has its own registry of implemented tool names

If these drift, you’ll see:
- tool lookup failures
- “not allowed” behavior
- or “tool missing” when parsing tool outputs

Fix:
- Update **both**:
  - Rust registry: `src-tauri/src/life_stream/mcp_registry.rs`
  - Node tools: `life-mcp/src/tools/*` and `src/tool-registry.js`

Then confirm categories + names in **[API_REFERENCE.md](API_REFERENCE.md)**.

---

### Wrong timezone (Pacific hardcoded)

Some date logic uses Pacific conventions (for day boundaries / “today”).

Fix:
- Locate the Pacific helpers (see `src/utils/pacificTime.ts` and Rust equivalents).
- Decide whether you want:
  - “user locale”
  - “server locale”
  - or “always Pacific” as a product decision

This is called out in **[GOTCHAS.md](GOTCHAS.md)** because it will bite dashboards and daily logs.

---

### Duplicate cards (patch versioning / stale state)

Common causes:
- persistence replay merges incorrectly
- “patch” cards are treated as new cards
- stale state not cleared between sessions

Fixes:
- verify card IDs are stable across patches
- ensure patch updates are merged, not appended
- if the persisted stream file contains duplicates, de-dupe by ID during load

Relevant areas:
- Rust decision + persistence: `src-tauri/src/life_stream/service.rs`
- Frontend store merge logic: `src/features/life-stream/state/streamStore.ts`

---

## Build issues

### Tauri build fails (Rust toolchain / native deps)

Fix checklist:
1) Run doctor:
```bash
cd CodexMonitor-lifeos
npm run doctor
```

2) Ensure `cmake` is installed (doctor checks this explicitly).

3) Ensure Rust toolchain is sane:
```bash
rustc --version
cargo --version
```

4) On macOS, ensure Xcode CLT is installed:
```bash
xcode-select --install
```

---

### TypeScript errors (TS ↔ Rust type sync)

Symptoms:
- TS build breaks after changing Rust types (or vice versa)
- runtime JSON decoding mismatches

Fix:
- update both:
  - `CodexMonitor-lifeos/src/types.ts`
  - `CodexMonitor-lifeos/src-tauri/src/types.rs`
  - and Life Stream-specific types under `src-tauri/src/life_stream/types.rs`

Then re-run:
```bash
npm run typecheck
npm run lint
```

See **[DATA_MODELS.md](DATA_MODELS.md)** and the “keep these in sync” section of **[GOTCHAS.md](GOTCHAS.md)**.

---

## Debug techniques

### Rust logs (desktop / daemon)

**Desktop dev:** run via terminal:
```bash
npm run tauri dev
```

You’ll see:
- `eprintln!` logs from Rust
- inherited logs from spawned processes (life-mcp, codex app-server)

**Daemon (launchd):**
- Template logs:
  - `/tmp/codex-monitor-daemon.log`
  - `/tmp/codex-monitor-daemon.err`

### MCP logs (life-mcp)

When spawned by the bridge, life-mcp stderr is inherited by the Rust process — it shows up in:
- your dev terminal (desktop)
- launchd log files (daemon)

### React state inspection

Use:
- React DevTools Components panel
- Console logging inside store subscription points
- Add temporary “debug HUD” components for rendering key store fields

### Verifying MCP servers in Claude Code

Quick commands:
```bash
claude mcp list
claude mcp get life-mcp
```

Inside Claude Code:
```text
/mcp
```

---

## If you’re still stuck 😅

Grab three things and you’ll usually solve it fast:
1) The exact error string
2) The relevant log snippet (Rust + Node + app-server)
3) The current values of `LIFE_OS_PATH` and `LIFE_MCP_PATH`

Then jump back to:
- **[DEPLOYMENT.md](DEPLOYMENT.md)** for setup verification
- **[MCP_INTEGRATION.md](MCP_INTEGRATION.md)** for tool parsing/whitelisting
- **[GOTCHAS.md](GOTCHAS.md)** for the “why is it like this” stuff
