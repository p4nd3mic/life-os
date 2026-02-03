# DEPLOYMENT.md — Setup & Deployment Guide

This doc covers **local development** and **Mac mini production** setup for the Life OS ecosystem:

- **CodexMonitor-lifeos** (Tauri + React + Rust desktop app)
- **life-mcp** (Node MCP server — spawned via stdio)
- **Obsidian vault** (Life OS data store)
- **life-os** (domain configs + ops scripts)

> Cross‑refs: start with **[ARCHITECTURE.md](ARCHITECTURE.md)** for the big picture, then **[MCP_INTEGRATION.md](MCP_INTEGRATION.md)** for bridge/tool behavior, and **[STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)** for state + persistence.

---

## Snapshot + canonical paths

| Project | Canonical path (your machine) | In this repo snapshot |
|---|---|---|
| CodexMonitor-lifeos | `/Volumes/YouTube 4TB/CodexMonitor-lifeos` | `CodexMonitor-lifeos/` |
| life-mcp | `/Volumes/YouTube 4TB/code/_archive/life-mcp` | `life-mcp/` |
| Obsidian vault | `/Volumes/YouTube 4TB/Obsidian` | `Obsidian-samples/` *(sample)* |
| life-os | `/Volumes/YouTube 4TB/code/life-os` | `life-os/` |

---

## Quick start matrix

| Component | Install | Run (dev) | Notes |
|---|---|---|---|
| CodexMonitor-lifeos (desktop) | `npm install` | `npm run tauri dev` *(or `npm run tauri:dev`)* | `tauri:dev` runs `scripts/doctor.sh` first. |
| life-mcp (stdio MCP) | `npm install` | `node index.js` | Usually **spawned automatically** by the Life Stream MCP bridge. |
| life-os (configs) | *(none)* | *(none)* | Uses `systems/*.yaml` + `justfile`. |
| Obsidian vault | *(none)* | *(open in Obsidian)* | Must exist for persistence; configured per workspace (see below). |

---

## Local development setup

### Prerequisites

Minimum (macOS):

| Tool | Why |
|---|---|
| Node.js + npm | Builds/runs React UI + life-mcp server |
| Rust toolchain (`rustup`, `cargo`) | Builds Tauri backend + Rust binaries |
| Xcode Command Line Tools | Native toolchain for Tauri + some deps |
| `cmake` | Required by `whisper-rs` (checked by `scripts/doctor.sh`) |
| `git` | Source control |
| `codex` CLI | Needed for **`codex app-server`** sessions (local backend mode) |

Optional:

| Tool | Why |
|---|---|
| `just` | Runs `life-os/justfile` recipes |
| Tauri CLI | Already included via `@tauri-apps/cli` in the repo, but global install is fine |
| Tailscale | Required for secure remote iOS ↔ Mac mini access |

### 1) Install dependencies

#### CodexMonitor-lifeos

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor-lifeos"
npm install
```

#### life-mcp

```bash
cd "/Volumes/YouTube 4TB/code/_archive/life-mcp"
npm install
cp .env.example .env   # then fill in secrets
```

### 2) Environment variables & secrets

#### A) Environment variables used by CodexMonitor-lifeos (Life OS features)

These are read by the Rust backend (Tauri process and/or daemon):

| Variable | Used by | Purpose | Example |
|---|---|---|---|
| `LIFE_OS_PATH` | Life Stream + life-mcp bridge | Points to **life-os** root (needed by life-mcp and domain configs) | `/Volumes/YouTube 4TB/code/life-os` |
| `LIFE_MCP_PATH` | Life Stream MCP bridge | Points to **life-mcp** root folder *(or a JS file)* | `/Volumes/YouTube 4TB/code/_archive/life-mcp` |
| `TMDB_API_KEY` | Cover enrichment (daemon) | Enables TMDB fetch + caching for media cards | `…` |
| `IGDB_CLIENT_ID` / `IGDB_CLIENT_SECRET` | Cover enrichment (daemon) | Optional: game lookup/covers | `…` |
| `EXA_API_KEY` | Search enrichment (daemon) | Optional: web search enrichment | `…` |

**Notes**
- If `LIFE_MCP_PATH` is a directory, the bridge will run `dist/index.js` if present, otherwise `index.js`.
- If `LIFE_OS_PATH` is missing, the bridge fails fast with `LIFE_OS_PATH not configured`.

#### B) Environment variables used by life-mcp

life-mcp loads `.env` from its repo root (`index.js` uses dotenv). Key ones:

| Variable | Purpose |
|---|---|
| `LIFE_OS_PATH` | Lets life-mcp load domain configs/contracts from `life-os/` |
| `SUPABASE_URL` + `SUPABASE_SERVICE_KEY` | Enables Supabase-backed tools |
| `GOOGLE_SHEETS_ID` + `GOOGLE_SERVICE_ACCOUNT_KEY` | Enables Sheets-backed tools |
| `USE_SUPABASE_TASKS` | Feature flag for task storage |

> There are many more env vars supported; see `life-mcp/.env.example` and `life-mcp/migrations/README.md`.

### 3) Run the desktop app (Tauri)

From the CodexMonitor-lifeos repo:

```bash
# Template-style command (works because package.json defines "tauri": "tauri")
npm run tauri dev

# Recommended: runs scripts/doctor.sh first
npm run tauri:dev
```

### 4) Confirm the Codex app-server connection

CodexMonitor uses `codex app-server` over stdio (one per workspace session). Quick sanity check:

```bash
codex app-server --help
```

If the app reports:

> “Codex app-server did not respond to initialize…”

…it usually means `codex app-server` fails in your terminal for that workspace.

### 5) How life-mcp is run during desktop usage

You normally **do not** run life-mcp manually. When Life Stream needs a tool call:

`LifeMcpBridge` spawns:

```text
node <resolved life-mcp script>
```

with:

- `MCP_MODE=stdio`
- `LIFE_OS_PATH=<your life-os root>`

and communicates via JSON-RPC over stdio.

If you *do* run it manually for debugging:

```bash
cd "/Volumes/YouTube 4TB/code/_archive/life-mcp"
node index.js
# Expected log: "Life MCP server running in stdio mode"
```

---

## Production deployment (Mac mini)

This section assumes:
- Mac mini is always-on
- You want iOS remote control + Life OS dashboards
- Remote access is **via Tailscale**, not public internet

### Build a release desktop app

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor-lifeos"
npm install
npm run tauri build
```

Output bundle location (Tauri default):

```text
CodexMonitor-lifeos/src-tauri/target/release/bundle/
```

### Run the always-on daemon (for iOS + remote backend mode)

The iOS app connects to the **`codex_monitor_daemon`** TCP server (default `127.0.0.1:4732`).

#### Build daemon binary

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor-lifeos"
cargo build --manifest-path src-tauri/Cargo.toml --release --bin codex_monitor_daemon
```

Binary:

```text
CodexMonitor-lifeos/src-tauri/target/release/codex_monitor_daemon
```

#### Configure token + data dir

- Token comes from env or CLI flag:
  - env: `CODEX_MONITOR_DAEMON_TOKEN`
  - flag: `--token <token>`

- Data dir default is `~/.local/share/codex-monitor-daemon` (unless `XDG_DATA_HOME` is set).
  - You can override with `--data-dir <path>`.

Recommended (macOS-friendly) data dir:

```text
/Users/<you>/Library/Application Support/codex-monitor-daemon
```

Generate a token:

```bash
openssl rand -hex 32
```

### launchd auto-start (daemon)

A template plist exists in the repo:

- `CodexMonitor-lifeos/scripts/com.codexmonitor.daemon.plist`

It runs:

```text
/Users/YOU/.local/bin/codex_monitor_daemon
  --listen 127.0.0.1:4732
  --token your-secret-token
  --data-dir /Users/YOU/Library/Application Support/codex-monitor-daemon
```

**Install as a LaunchAgent (user session):**

```bash
cp "CodexMonitor-lifeos/scripts/com.codexmonitor.daemon.plist"   ~/Library/LaunchAgents/com.codexmonitor.daemon.plist

launchctl unload ~/Library/LaunchAgents/com.codexmonitor.daemon.plist 2>/dev/null || true
launchctl load   ~/Library/LaunchAgents/com.codexmonitor.daemon.plist
```

**Logs (per the template):**
- stdout: `/tmp/codex-monitor-daemon.log`
- stderr: `/tmp/codex-monitor-daemon.err`

> If you install as a system LaunchDaemon instead, you’ll need root and should think carefully about filesystem permissions (workspaces, Obsidian vault, SSH keys, etc.).

### Tailscale VPN remote access

**Recommended posture:** keep daemon bound to localhost (`127.0.0.1:4732`) and expose it via Tailscale:

```bash
tailscale serve tcp 4732 tcp://127.0.0.1:4732
```

Then in iOS Settings use:
- Host: your MagicDNS name (e.g. `mac-mini.tailnet.ts.net`) or Tailscale IP
- Port: `4732`
- Token: the same daemon token

### Where logs and state live

| Component | Logs | State/data |
|---|---|---|
| Tauri dev run | your terminal | app data dir (see below) |
| Desktop app (release) | Console.app (system logs) | `~/Library/Application Support/<tauri identifier>/` (stores `settings.json`, `workspaces.json`, etc.) |
| Daemon (launchd) | `/tmp/codex-monitor-daemon*.log` *(template)* | daemon `--data-dir` (contains sessions, cached state) |
| life-mcp | stderr inherited by parent | `life-mcp/.env` + its own local caches |

---

## MCP server registration

This is about registering **life-mcp** as an MCP server for external clients (Codex, Claude Code).  
Within CodexMonitor-lifeos, the Life Stream bridge spawns life-mcp directly — no registration required.

### Register life-mcp with Codex (`~/.codex/config.toml`)

Example entry (stdio Node server):

```toml
[mcp_servers.life_mcp]
command = "node"
args = ["/Volumes/YouTube 4TB/code/_archive/life-mcp/index.js"]
env = {
  LIFE_OS_PATH = "/Volumes/YouTube 4TB/code/life-os"
  # Optionally pass keys here, or keep them in life-mcp/.env
  # SUPABASE_URL = "https://<project>.supabase.co"
  # SUPABASE_SERVICE_KEY = "<service key>"
}
```

Restart the Codex/app-server process after editing config so tools reload.

### Register life-mcp with Claude Code

Claude Code supports:
- **CLI install** (recommended): `claude mcp add …`
- **Project `.mcp.json`** (team-shared)
- **User scope** stored in `~/.claude.json`

**Example (project scope) `.mcp.json` file at your project root:**

```json
{
  "mcpServers": {
    "life-mcp": {
      "command": "node",
      "args": ["/Volumes/YouTube 4TB/code/_archive/life-mcp/index.js"],
      "env": {
        "LIFE_OS_PATH": "/Volumes/YouTube 4TB/code/life-os"
      }
    }
  }
}
```

Then in Claude Code you’ll be prompted to approve project servers, unless you’ve enabled auto-approval in settings.

### Testing MCP connection

**life-mcp itself:**

```bash
cd "/Volumes/YouTube 4TB/code/_archive/life-mcp"
node index.js
```

**Claude Code:**

```bash
claude mcp list
claude mcp get life-mcp
```

Inside Claude Code UI, run:

```text
/mcp
```

---

## Key files

### CodexMonitor-lifeos

| File | Why it matters |
|---|---|
| `src-tauri/tauri.conf.json` | App identifier, build config |
| `package.json` | Scripts: dev/build/test |
| `src-tauri/Cargo.toml` | Rust deps + binaries (`codex_monitor_daemon`, etc.) |
| `src-tauri/src/life_stream/mcp_bridge.rs` | Spawns Node (`life-mcp`) + stdio JSON-RPC |

### life-mcp

| File | Why it matters |
|---|---|
| `index.js` | Entry point (stdio by default) |
| `.env.example` | Env var template |
| `src/server/main.js` | Chooses stdio vs HTTP mode |

### life-os / Obsidian

| Path | Why it matters |
|---|---|
| `life-os/systems/*.yaml` | Domain configs referenced by tools |
| `Obsidian/Stream/*.md` | Life logs (persistence target) |
| `Obsidian/_config/categories.yml` | Category schema |

---

## See also

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — end-to-end system flow
- **[MCP_INTEGRATION.md](MCP_INTEGRATION.md)** — tool registry + parsing + enrichment
- **[LIFE_STREAM.md](LIFE_STREAM.md)** — card lifecycle + persistence formats
- **[GOTCHAS.md](GOTCHAS.md)** — non-obvious behaviors (env vars, timeouts, naming)
