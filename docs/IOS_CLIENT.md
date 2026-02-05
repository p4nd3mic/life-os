# IOS_CLIENT.md — Mobile App Documentation (SwiftUI)

This doc describes the **iOS / iPadOS client** included in the archive and how it connects to the Mac mini daemon.

> Cross‑refs:
> - **[ARCHITECTURE.md](ARCHITECTURE.md)** — end-to-end diagram (desktop/daemon/MCP/Obsidian)
> - **[DEPLOYMENT.md](DEPLOYMENT.md)** — daemon + Tailscale setup
> - **[API_REFERENCE.md](API_REFERENCE.md)** — RPC methods + event shapes
> - **[STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)** — remote backend vs local backend modes

---

## What’s in the archive

✅ iOS code **is present** under:

- `CodexMonitor-lifeos/ios/`

The mobile app is a **remote client**:
- it does **not** run Codex locally
- it connects over the network to a Rust daemon: **`codex_monitor_daemon`**
- most UI state updates are driven by streamed **notifications** (not polling)

---

## Source layout (important paths)

### App project

| Path | Purpose |
|---|---|
| `ios/CodexMonitorMobile/CodexMonitorMobile/CodexMonitorMobileApp.swift` | App entrypoint |
| `ios/CodexMonitorMobile/CodexMonitorMobile/CodexStore.swift` | Main state store (ObservableObject) |
| `ios/CodexMonitorMobile/CodexMonitorMobile/Views/RootView.swift` | Phone tabs + iPad split view |
| `ios/CodexMonitorMobile/CodexMonitorMobile/Views/*` | Chat, projects, dashboards, settings, etc. |
| `ios/CodexMonitorMobile/project.yml` | XcodeGen project definition (iOS 26.0 target in this snapshot) |

### Shared Swift package (RPC + models)

| Path | Purpose |
|---|---|
| `ios/Packages/CodexMonitorRPC/Package.swift` | Swift Package manifest |
| `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorRPC/RPCClient.swift` | TCP client + newline-framed JSON + auth |
| `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorRPC/CodexMonitorAPI.swift` | Typed RPC wrapper (methods) |
| `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift` | Canonical Swift model types |
| `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorRendering/Rendering.swift` | Markdown/AttributedString helpers |

---

## Architecture

### High-level flow

```mermaid
flowchart LR
  subgraph iOS[iOS / iPadOS]
    UI[SwiftUI Views] --> Store[CodexStore]
    Store --> API[CodexMonitorAPI]
    API --> RPC[RPCClient]
  end

  RPC -->|TCP + newline JSON| Daemon[codex_monitor_daemon :4732]

  subgraph Mac[Mac mini]
    Daemon --> AppServer[codex app-server]
    Daemon --> MCP[life-mcp (stdio, spawned)]
    Daemon --> Obsidian[Obsidian Vault]
  end
```

### UI structure

The app chooses a layout based on device type:

- **Phone:** `TabView` with sections (Projects, Domain, Codex, Memory, Git, Debug Log, Browser, Skills)
- **iPad/tablet:** **full-screen Life Stream WebView** (desktop‑identical UI)

Root router:
- `ios/CodexMonitorMobile/CodexMonitorMobile/Views/RootView.swift`

### iPad WebView shell (desktop parity)

On iPad, the app renders the **desktop Life Stream React UI** inside a WKWebView:

- View: `Views/LifeStreamWebView.swift`
- Entry HTML: `index.webview.html`
- Bundle assets: `ios/CodexMonitorMobile/CodexMonitorMobile/WebView/`

**Key hardening details:**
- `preferredContentMode = .desktop` to force desktop layout.
- Safe‑area handling via CSS (`env(safe-area-inset-*)`) to keep the top bar + composer aligned.
- Visual viewport height tracked to keep composer visible when the keyboard opens.
- Scroll bounce disabled to match desktop behavior.

### Local build + iPad mini simulator (justfile)

From the repo root:

```bash
just app-ipad
```

This will:
- build the WebView bundle (`npm run build:webview`)
- boot the **iPad mini** simulator
- build + install the iOS app
- launch the app on the simulator

You can also run both desktop + iPad in one command:

```bash
just app
```

---

## State management: CodexStore

**File:** `ios/CodexMonitorMobile/CodexMonitorMobile/CodexStore.swift`

`CodexStore` is an `ObservableObject` that owns:

### Connection state + settings

| Field | Meaning |
|---|---|
| `host` / `port` | Where the daemon lives (default port: `4732`) |
| `token` | Required auth token (stored in Keychain) |
| `isConnected` / `connectionError` | Derived connection state |
| `debugLog` | A rolling in-app log for notifications/events |

Connection behavior:
- `connect()` validates host + token and then connects
- `disconnect()` is called when the app backgrounds (the store observes `UIApplication.didEnterBackgroundNotification`)

### Workspace + chat state

CodexStore holds:
- workspace list + groups
- threads/conversations
- streaming updates from `app-server-event` notifications
- approval requests / decisions
- terminal output streams

This mirrors the desktop app’s backend structure, but optimized for mobile navigation.

---

## RPC connection details

### Transport + framing

**RPCClient** uses `Network.framework` (`NWConnection`) with **TCP**.

Framing rule:
- **one JSON object per newline**
- each outbound request is serialized to JSON and appended with `\n`

### Auth handshake

Immediately after TCP connect, the client sends an `auth` request:

```json
{ "method": "auth", "params": { "token": "<token>" } }
```

If the token is wrong, the daemon responds with an error and closes the connection.

### Notifications (push-style updates)

The daemon sends newline-framed JSON notifications. In this snapshot, the iOS client explicitly handles:

- `app-server-event` — streamed events from Codex sessions
- `terminal-output` — shell command output
- `debug-log` — debug message lines

See:
- `CodexStore.handleNotification(...)`
- `RPCMessage.swift` (notification envelope)

---

## Connecting to the Mac mini daemon

### Requirements

- The daemon must be running on the Mac mini:
  - binary: `codex_monitor_daemon`
  - listens on: `127.0.0.1:4732` by default
- Remote access should be via **Tailscale** (recommended)

### Recommended network posture

1) Daemon binds to localhost:
```text
127.0.0.1:4732
```

2) Expose via tailnet only:
```bash
tailscale serve tcp 4732 tcp://127.0.0.1:4732
```

3) Configure iOS app Settings:
- Host: your Tailscale MagicDNS name (or Tailscale IP)
- Port: `4732`
- Token: the daemon token

Full setup instructions are in **[DEPLOYMENT.md](DEPLOYMENT.md)**.

---

## Mobile features (what you can do)

### 1) Chat interface (Codex)
- Browse workspaces
- Open a workspace and run chat
- See streamed output + approvals (when supported)
- View token usage and model/session metadata (depending on daemon payloads)

### 2) Life OS dashboards (mobile-first logging)
The app has a “Domain” section and a dedicated Life workspace view:

- `Views/LifeWorkspaceView.swift`
- Domain dashboards include:
  - Delivery
  - Nutrition
  - Exercise
  - Media
  - YouTube
  - Finance

These dashboards call daemon RPC methods like:
- `get_delivery_dashboard`
- `get_nutrition_dashboard`
- `get_exercise_dashboard`
- …

> The **Life Stream card UI** is primarily a desktop feature. Mobile focuses on dashboards + remote control.

### 3) Git + terminal (remote control)
- Run shell commands on the Mac daemon side
- View terminal output streaming back to the phone/tablet

---

## Push notifications

No APNS / push notification implementation was found in this snapshot:
- no `UNUserNotificationCenter` usage
- no remote notification handlers

If you want push later, you’ll need:
- an APNS-capable backend (or a relay)
- user authentication beyond the shared daemon token
- careful scoping of what gets pushed (approvals, errors, daily reminders, etc.)

---

## Building the iOS app

### Tooling

This snapshot uses **XcodeGen** (because `project.yml` exists):

- `ios/CodexMonitorMobile/project.yml`

Notable settings (from `project.yml`):
- `deploymentTarget: iOS 26.0`
- `SWIFT_VERSION: 6.0`

If your local Xcode SDK doesn’t support iOS 26.0:
- lower the deployment target in `project.yml`
- regenerate the Xcode project

### Typical build workflow

```bash
cd CodexMonitor-lifeos/ios/CodexMonitorMobile

# (If you use XcodeGen)
xcodegen generate

# Then open the workspace/project in Xcode and run
```

### Signing & provisioning

Standard Apple flow:
- set a valid Team
- choose a bundle identifier
- configure capabilities

### TestFlight deployment (high level)

1) Archive in Xcode (Release)
2) Upload to App Store Connect
3) Create TestFlight build + add testers

---

## Troubleshooting mobile connection

| Symptom | Fix |
|---|---|
| “Host and token required” | Enter both; token must match daemon `--token` |
| Connect spins then fails | Confirm daemon is listening and reachable over Tailscale |
| Works on Wi‑Fi only | You’re not actually using Tailscale; use MagicDNS or Tailscale IP |
| Connects but no data | Check daemon logs; confirm workspaces exist on daemon side |

Also see **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** for deeper debugging steps.
