# 📱 CodexMonitor Mobile Backend Setup (Mac mini + Tailscale)

This guide configures the **codex_monitor_daemon** on your always‑on Mac mini so iOS devices can connect securely over Tailscale.

---

## ✅ 1) Build the daemon
```bash
cd /Volumes/YouTube\ 4TB/code/CodexMonitor/src-tauri
cargo build --release --bin codex_monitor_daemon
```

Binary output:
```
/Volumes/YouTube 4TB/code/CodexMonitor/src-tauri/target/release/codex_monitor_daemon
```

---

## ✅ 2) Choose a data directory + token
Recommended data dir:
```
/Users/<you>/Library/Application Support/codex-monitor-daemon
```

Generate a strong token (example using `openssl`):
```bash
openssl rand -hex 32
```

---

## ✅ 3) LaunchD (auto-start on boot)
Template plist is provided here:
```
/scripts/com.codexmonitor.daemon.plist
```

Steps:
1) Copy the template into your LaunchAgents folder.
2) Replace placeholder paths + token.
3) Load it with `launchctl`.

Example:
```bash
cp /Volumes/YouTube\ 4TB/code/CodexMonitor/scripts/com.codexmonitor.daemon.plist \
  ~/Library/LaunchAgents/com.codexmonitor.daemon.plist

launchctl unload ~/Library/LaunchAgents/com.codexmonitor.daemon.plist 2>/dev/null || true
launchctl load ~/Library/LaunchAgents/com.codexmonitor.daemon.plist
```

---

## ✅ 4) Tailscale exposure (secure)
**Preferred:** bind daemon to localhost and expose via Tailscale only.

Daemon should listen on:
```
127.0.0.1:4732
```

### Option A — Tailscale Serve (recommended)
```bash
tailscale serve tcp 4732 tcp://127.0.0.1:4732
```

### Option B — Direct MagicDNS + local firewall
If you’ve configured MagicDNS and a local firewall rule, connect from iOS using:
```
<mac-mini-hostname>.<tailnet>.ts.net:4732
```

> ⚠️ **Never expose this port to the public internet.** Use tailnet + token only.

---

## ✅ 5) iOS App Settings
Inside the iOS app Settings screen, set:
- **Host**: your tailnet hostname or Tailscale IP
- **Port**: `4732`
- **Token**: the same token from step 2

---

## ✅ Quick health check
From any machine on your tailnet:
```bash
nc <host> 4732
```
Then send:
```json
{"id":1,"method":"auth","params":{"token":"YOUR_TOKEN"}}
```
Followed by:
```json
{"id":2,"method":"ping","params":{}}
```
