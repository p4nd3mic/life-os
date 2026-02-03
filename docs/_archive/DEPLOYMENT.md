# Deployment

This document covers deploying the **CodexMonitor daemon** (and supporting infrastructure) so that:
- iOS/iPadOS clients can connect remotely
- desktop clients can optionally run in remote backend mode

> The daemon is a powerful remote control surface (git, file reads, PTY shells, Codex turns). Treat it like you would treat SSH access.

---

## Mac Mini daemon setup (launchd)

The repository includes a sample launchd plist:

- `scripts/com.codexmonitor.daemon.plist`

### What the plist does

Key settings (from the plist):

- Runs the daemon at boot:
  - `RunAtLoad = true`
  - `KeepAlive = true`
- Executes:
  - `/usr/local/bin/codex-monitor-daemon`
  - with arguments:
    - `--listen 127.0.0.1:4732`
    - `--data-dir /Users/admin/Library/Application Support/CodexMonitor/Daemon`
- Injects environment:
  - `CODEX_MONITOR_DAEMON_TOKEN` (required)
  - `PATH` (ensure `codex`, `node`, `git`, `gh` are discoverable)
- Logs:
  - stdout: `/tmp/codex-monitor-daemon.log`
  - stderr: `/tmp/codex-monitor-daemon.err`

### Install steps (typical)

1. Build the daemon binary

From the `src-tauri` crate:

```sh
cd src-tauri
cargo build --release --bin codex_monitor_daemon
```

This produces:
- `src-tauri/target/release/codex_monitor_daemon`

The plist expects the installed name `codex-monitor-daemon`, so you typically copy/rename:

```sh
sudo cp src-tauri/target/release/codex_monitor_daemon /usr/local/bin/codex-monitor-daemon
sudo chmod 755 /usr/local/bin/codex-monitor-daemon
```

2. Install the plist

```sh
sudo cp scripts/com.codexmonitor.daemon.plist /Library/LaunchDaemons/com.codexmonitor.daemon.plist
sudo chown root:wheel /Library/LaunchDaemons/com.codexmonitor.daemon.plist
sudo chmod 644 /Library/LaunchDaemons/com.codexmonitor.daemon.plist
```

3. Load and start

```sh
sudo launchctl load -w /Library/LaunchDaemons/com.codexmonitor.daemon.plist
sudo launchctl start com.codexmonitor.daemon
```

4. Verify

- Check logs:
  - `/tmp/codex-monitor-daemon.log`
  - `/tmp/codex-monitor-daemon.err`

- Verify it’s listening locally:

```sh
lsof -iTCP:4732 -sTCP:LISTEN
```

---

## Token authentication

The daemon requires a token unless started with the dev-only `--insecure-no-auth` mode.

Configure token via:
- plist environment: `CODEX_MONITOR_DAEMON_TOKEN`
- or CLI: `--token`

Clients authenticate by calling the `auth` RPC method first (see `docs/API_REFERENCE.md`).

---

## Tailscale integration (recommended)

The daemon transport is plain TCP (no TLS). The safe pattern is:

- bind daemon to localhost (`127.0.0.1:4732`)
- expose it privately via Tailscale to your tailnet devices

The repository provides a concrete recipe in:

- `docs/mobile_backend_setup.md`

### Typical commands

Enable Tailscale serve to forward tailnet traffic to the local daemon:

```sh
sudo tailscale serve tcp 4732 tcp://127.0.0.1:4732
```

Notes:
- This exposes the daemon to devices logged into your tailnet.
- You can revoke serving with `tailscale serve reset`.

### Connecting from iOS

Use the tailnet DNS name of the Mac mini:

- Host: `<your-machine>.tailnet-XXXX.ts.net`
- Port: `4732`
- Token: the same `CODEX_MONITOR_DAEMON_TOKEN` value

---

## iOS provisioning and entitlements

### Required Info.plist usage strings

The iOS app includes:

- `NSLocalNetworkUsageDescription` — required to connect to a LAN/daemon
- `NSMicrophoneUsageDescription` — required for dictation
- `NSSpeechRecognitionUsageDescription` — required for speech-to-text
- `NSPhotoLibraryUsageDescription` — required for image attachments

See:
- `ios/CodexMonitorMobile/CodexMonitorMobile/Info.plist`

### Signing

The app uses standard iOS signing. No special network entitlements are required beyond the local-network permission prompt. If you run into local network issues on iOS, confirm:
- the local network prompt was accepted
- the host and port are reachable (especially when using Tailscale)

---

## Environment variables used by the daemon / backend

| Variable | Used by | Purpose |
|---|---|---|
| `CODEX_MONITOR_DAEMON_TOKEN` | daemon | Shared token for `auth` |
| `CODEX_HOME` | daemon + desktop backend | Override Codex data dir (defaults to `~/.codex`) |
| `PATH` | daemon (launchd) | Ensure `codex`, `node`, `git`, `gh` are discoverable |
| `SHELL` | daemon terminal sessions | Chooses the spawned interactive shell |

Notes:
- Codex itself may use additional env vars; those are outside this repo.
- If `gh` is used (GitHub features), the daemon host must be authenticated via `gh auth login`.

---

## MCP Memory Server (Codex tools)

The Memory MCP server runs as a local stdio process and must be registered in Codex config.

### Build the MCP binary

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor"
cargo build --manifest-path src-tauri/Cargo.toml --release
```

### Add to Codex config

Edit `~/.codex/config.toml` and add:

```toml
[mcp_servers.codex_monitor_memory]
command = "/Volumes/YouTube 4TB/CodexMonitor/src-tauri/target/release/codex_monitor_memory_mcp"
args = []
env = {
  SUPABASE_URL = "https://<project>.supabase.co",
  SUPABASE_ANON_KEY = "<anon key>",
  MINIMAX_API_KEY = "<optional>"
}
```

Restart Codex/app-server after updating the config so the MCP tools are loaded.

---

## Operational checklist

- ✅ Daemon listens on `127.0.0.1:4732` (not `0.0.0.0`)
- ✅ Token set and kept secret
- ✅ Tailscale serve enabled (if remote access needed)
- ✅ `codex` and `node` available in launchd `PATH`
- ✅ Logs monitored for crashes

---

## Browser Worker + MCP (2026-01-26)

### Browser Worker
```bash
cd browser-worker
npm install
npm run build
```

Set optional override:
```bash
export CODEX_MONITOR_BROWSER_WORKER=/absolute/path/to/browser-worker/dist/index.js
```

### Browser MCP
```bash
export CODEX_MONITOR_DAEMON_ADDR=127.0.0.1:4732
export CODEX_MONITOR_DAEMON_TOKEN=your-token
```

Add to `~/.codex/config.toml`:
```toml
[mcp_servers.codex_monitor_browser]
command = "codex_monitor_browser_mcp"
```
