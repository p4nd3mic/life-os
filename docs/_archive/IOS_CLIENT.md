# iOS / iPadOS Client (SwiftUI)

This document explains the **SwiftUI mobile client** and the shared Swift package targets it depends on.

At a high level:
- The iOS app is a **thin remote client**: it does **not** run Codex locally.
- It connects to a **CodexMonitor daemon** over TCP using a small JSON-RPC-like protocol.
- Most UI state (threads, items, plans, token usage) is driven by **streamed `app-server-event` notifications**.

Key paths:
- App entry: `ios/CodexMonitorMobile/CodexMonitorMobile/CodexMonitorMobileApp.swift`
- Main state store: `ios/CodexMonitorMobile/CodexMonitorMobile/CodexStore.swift`
- Swift package (RPC + models + rendering): `ios/Packages/CodexMonitorRPC/`

---

## Swift package structure

The iOS app depends on the Swift package at:

- `ios/Packages/CodexMonitorRPC/Package.swift`

It defines **three targets**:

| Target | Path | Purpose |
|---|---|---|
| `CodexMonitorModels` | `Sources/CodexMonitorModels/` | Shared Codable models for RPC + app-server events + convenience helpers |
| `CodexMonitorRPC` | `Sources/CodexMonitorRPC/` | TCP JSON-RPC client (`RPCClient`) + typed API wrapper (`CodexMonitorAPI`) |
| `CodexMonitorRendering` | `Sources/CodexMonitorRendering/` | `AttributedString` helpers for rendering markdown / monospace text |

### CodexMonitorRPC target

Files:
- `RPCClient.swift` — TCP connection + newline-framed message send/receive + auth
- `RPCMessage.swift` — message envelope types (`RPCRequest`, `RPCResponse`, `RPCNotification`)
- `CodexMonitorAPI.swift` — convenience methods mirroring daemon RPC endpoints

### CodexMonitorModels target

Files:
- `Models.swift` — **the canonical Swift model layer** (workspaces, threads, items, approvals, git, prompts, usage, etc.)
- `ConversationHelpers.swift` — streaming merge + truncation utilities used by `CodexStore`
- `JSONValue.swift` — type-safe “any JSON” value for decoding Codex messages

### CodexMonitorRendering target

Files:
- `Rendering.swift` — converts markdown and monospaced strings into `AttributedString` for SwiftUI rendering.

---

## RPC client: connection lifecycle, framing, auth

### Transport and framing

The RPC client uses **raw TCP** via `Network.framework`:

- `NWConnection(host:port:using:.tcp)`
- **One JSON object per newline** framing (same as the daemon).

Core implementation: `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorRPC/RPCClient.swift`.

### Auth handshake

On successful TCP connect, `RPCClient.connect(...)` immediately sends:

```json
{ "id": 1, "method": "auth", "params": { "token": "<token>" } }
```

If the daemon returns an error response, `connect` throws and the client treats the connection as failed.

### Read loop and message dispatch

- Outbound requests are serialized with `JSONEncoder` and appended with `\n`.
- Inbound data is accumulated into a string buffer and split by `\n`.
- Each complete line is decoded as `RPCMessage` and passed to a caller-provided handler:

```swift
try await rpc.connect(host: host, port: port, token: token) { message in
  await self.handleRPCMessage(message)
}
```

The handler is used for both:
- responses to in-flight calls
- daemon notifications (`app-server-event`, `terminal-output`)

### Reconnection strategy

Reconnection is managed by `CodexStore` (not by `RPCClient` itself):

- A failed connect calls `scheduleReconnect()`
- Delay uses an exponential-ish backoff (`retryDelaySeconds`)
- On a clean connection, delay resets

---

## State management: CodexStore

Core file: `ios/CodexMonitorMobile/CodexMonitorMobile/CodexStore.swift`

### Actor and update model

`CodexStore` is annotated `@MainActor` and conforms to `ObservableObject`.

This has two important consequences:
1. All `@Published` state updates occur on the main actor (safe for SwiftUI).
2. Networking can still happen off-main because `RPCClient` runs the socket on `.global()` queue, but state mutation is marshaled back through awaited `@MainActor` methods.

### Persisted connection settings

The iOS app persists connection info using a mix of:
- `UserDefaults` for host/port (simple non-secret values)
- **Keychain** for the token (`KeychainHelper.swift`)

Relevant types:
- `KeychainHelper` — wrapper around `SecItemAdd` / `SecItemCopyMatching` / `SecItemUpdate`
- `CodexStore.loadSavedSettings()` / `saveSettings()`

### Primary published properties (high signal)

`CodexStore` is large; these are the key “public” state surfaces the UI binds to:

- Connection:
  - `isConnected: Bool`
  - `connectionState: ConnectionState` (idle/connecting/connected/error)
  - `debugLog: [String]` (used by `DebugLogView`)
- Workspaces:
  - `workspaces: [WorkspaceInfo]`
  - `selectedWorkspaceId: String?`
  - `connectedWorkspaceId: String?` (daemon-side session connection)
- Threads:
  - `threads: [ThreadSummary]`
  - `selectedThreadId: String?`
  - `threadItems: [ConversationItem]` for the selected thread (plus caches)
- Terminal:
  - `terminalSessions: [String: TerminalSessionState]`
  - `terminalOutputBuffer` per session
- Git:
  - `gitStatus`, `gitDiffs`, `gitLog`, etc. (populated via explicit refresh calls)

### Notification handling

The daemon pushes notifications (no `id`) which `CodexStore` routes by `method`:

- `app-server-event`:
  - contains `{ workspace_id, message }`
  - `message` is arbitrary JSON emitted by Codex `app-server`
  - the store updates the in-memory conversation by interpreting common event patterns:
    - streaming deltas (`item/agentMessage/delta`, `item/reasoning/*Delta`)
    - tool output (`item/commandExecution/outputDelta`, `item/fileChange/outputDelta`)
    - turn lifecycle (`turn/started`, `turn/completed`, `turn/plan/updated`)
    - token usage (`thread/tokenUsage/updated`)
    - rate limit updates (`account/rateLimits/updated`)
    - approval requests (method names containing `requestApproval`)
- `terminal-output`:
  - contains `{ workspaceId, terminalId, data }`
  - appended to the relevant terminal session buffer

To keep the UI performant, `ConversationHelpers` applies:
- maximum items per thread (`maxItemsPerThread`)
- truncation of very long text (`maxItemText`)
- aggressive trimming of old tool output (`toolOutputRecentItems`)

See: `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/ConversationHelpers.swift`.

---

## View hierarchy (SwiftUI)

Entry: `CodexMonitorMobileApp.swift`

At launch:
- creates `@StateObject var store = CodexStore()`
- calls `store.loadSavedSettings()` and `store.connect()` in `.task`

The top-level view is:

- `RootView`
  - chooses between phone/tablet layout using horizontal size class
  - provides a "glass" background style

### Primary screens

| View | File | Purpose |
|---|---|---|
| Root container | `Views/RootView.swift` | Layout switcher: `PhoneRootView` vs `TabletRootView` |
| Projects list | `Views/ProjectsView.swift` | Select workspace; navigate to workspace detail |
| Workspace list | `Views/WorkspaceListView.swift` | Lists workspaces, connect/remove/add |
| Threads list | `Views/ThreadsListView.swift` | Shows threads for selected workspace |
| Conversation | `Views/ConversationView.swift` | Renders conversation items and plan/status |
| Composer | `Views/ComposerView.swift` | Message input, access mode, model, image attachments, dictation |
| Git | `Views/GitView.swift` | Status/diffs/log operations |
| Files | `Views/FilesView.swift` | Workspace file browser + file read |
| Terminal | `Views/TerminalView.swift` | PTY session UI (basic) |
| Prompts | `Views/PromptsView.swift` | List/create/update prompts |
| Settings | `Views/SettingsView.swift` | Host/port/token + reconnect |
| Debug log | `Views/DebugLogView.swift` | Shows internal log buffer |
| Connection indicator | `Views/ConnectionStatusView.swift` | Small UI state indicator |
| **Life Workspace** | `Views/LifeWorkspaceView.swift` | **Unified life assistant container** |
| **Domain Tab Bar** | `Views/DomainTabBar.swift` | **Domain selector tabs** |

---

## Life Workspace Views (iOS)

The Life workspace provides a unified hub for all life domains with domain-specific dashboard views.

### View Structure

```
ios/CodexMonitorMobile/CodexMonitorMobile/Views/
├── LifeWorkspaceView.swift         # Main life workspace container
├── DomainTabBar.swift              # Domain selector tabs
├── domains/
│   ├── DeliveryDashboardView.swift # Earnings, orders, hourly rates
│   ├── NutritionDashboardView.swift # Macros, meals, trends
│   ├── ExerciseDashboardView.swift  # Workouts, streaks
│   ├── MediaDashboardView.swift     # Bookshelf covers, ratings
│   ├── YouTubeDashboardView.swift   # Pipeline by tier
│   └── FinanceDashboardView.swift   # Bills, due dates
└── shared/
    ├── StatCardView.swift           # Reusable stat card
    ├── CoverImageView.swift         # Async image with fallback
    └── TimeRangePicker.swift        # Today/Week/Month/Lifetime
```

### LifeWorkspaceView

The main container that switches between chat and domain dashboard views:

```swift
struct LifeWorkspaceView: View {
    @EnvironmentObject var store: CodexStore
    @State private var activeDomain: LifeDomain? = nil
    @State private var timeRange: TimeRange = .today

    var body: some View {
        HStack(spacing: 0) {
            // Center panel - chat or domain dashboard
            if let domain = activeDomain {
                DomainDashboardContainer(
                    domain: domain,
                    timeRange: $timeRange,
                    onBack: { activeDomain = nil }
                )
            } else {
                ConversationView()
            }

            // Right panel - domain tabs
            DomainTabBar(
                activeDomain: $activeDomain
            )
        }
    }
}
```

### DomainTabBar

Displays domain tabs in the right panel:

```swift
struct DomainTabBar: View {
    @Binding var activeDomain: LifeDomain?

    var body: some View {
        VStack(spacing: 12) {
            ForEach(LifeDomain.allCases) { domain in
                DomainTab(
                    domain: domain,
                    isActive: activeDomain == domain,
                    onTap: { activeDomain = domain }
                )
            }
        }
        .padding()
        .background(.ultraThinMaterial)
    }
}
```

### Domain Dashboard Views

Each domain has a dedicated SwiftUI view:

```swift
struct DeliveryDashboardView: View {
    @EnvironmentObject var store: CodexStore
    @Binding var timeRange: TimeRange
    @State private var dashboard: DeliveryDashboard?

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Stats row
                HStack(spacing: 12) {
                    StatCardView(title: "Earnings", value: dashboard?.stats.totalEarnings.formatted(.currency(code: "USD")) ?? "--")
                    StatCardView(title: "Orders", value: "\(dashboard?.stats.orderCount ?? 0)")
                    StatCardView(title: "$/hr", value: dashboard?.stats.hourlyRate.formatted() ?? "--")
                }

                // Time range picker
                TimeRangePicker(selection: $timeRange)

                // Orders table
                if let orders = dashboard?.orders {
                    OrdersListView(orders: orders)
                }
            }
            .padding()
        }
        .task(id: timeRange) {
            dashboard = await store.fetchDeliveryDashboard(range: timeRange.rawValue)
        }
    }
}
```

### CodexStore Additions

New methods for fetching domain dashboard data:

```swift
extension CodexStore {
    func fetchDeliveryDashboard(range: String) async -> DeliveryDashboard? {
        guard let workspaceId = activeWorkspaceId else { return nil }
        return try? await api.call(
            method: "get_delivery_dashboard",
            params: ["workspaceId": workspaceId, "range": range]
        )
    }

    func fetchNutritionDashboard(range: String) async -> NutritionDashboard?
    func fetchMediaDashboard(range: String) async -> MediaDashboard?
    func fetchYouTubeDashboard(range: String) async -> YouTubeDashboard?
    func fetchFinanceDashboard(range: String) async -> FinanceDashboard?
}
```

### Domain Types (Swift)

```swift
enum LifeDomain: String, CaseIterable, Identifiable {
    case delivery
    case nutrition
    case exercise
    case media
    case youtube
    case finance

    var id: String { rawValue }

    var icon: String {
        switch self {
        case .delivery: return "car.fill"
        case .nutrition: return "fork.knife"
        case .exercise: return "figure.walk"
        case .media: return "tv.fill"
        case .youtube: return "play.rectangle.fill"
        case .finance: return "dollarsign.circle.fill"
        }
    }

    var accentColor: Color {
        switch self {
        case .delivery: return .blue
        case .nutrition: return .green
        case .exercise: return .orange
        case .media: return .purple
        case .youtube: return .red
        case .finance: return .yellow
        }
    }
}

enum TimeRange: String, CaseIterable {
    case today
    case week
    case month
    case lifetime
}
```

### Visual Design (iOS)

The iOS Life workspace follows the same dark grid aesthetic:

```swift
struct LifeDashboardBackground: View {
    var body: some View {
        ZStack {
            LinearGradient(
                colors: [Color(hex: "1a1a2e"), Color(hex: "16213e")],
                startPoint: .top,
                endPoint: .bottom
            )
            GridPattern()
                .stroke(Color.white.opacity(0.03), lineWidth: 1)
        }
        .ignoresSafeArea()
    }
}

struct StatCardView: View {
    let title: String
    let value: String
    var accentColor: Color = .blue

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
                .foregroundColor(.gray)
            Text(value)
                .font(.title2.bold())
                .foregroundColor(accentColor)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding()
        .background(Color.black.opacity(0.3))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(0.1), lineWidth: 1)
        )
    }
}
```

### Shared “glass” components

- `Views/GlassComponents.swift`
- `Utilities/GradientBackground.swift`, `ThemeGradient.swift`, `BackgroundClearer.swift`

These implement the translucent gradient UI background.

---

## Threading model (MainActor vs background)

- `CodexStore` is `@MainActor`:
  - mutations are safe for UI
  - long work should be pushed into background tasks where possible
- `RPCClient`:
  - socket is started with `connection.start(queue: .global())`
  - receive callbacks occur off-main, but message handling is awaited back into the store
- Image encoding:
  - `ComposerView.send()` base64-encodes image data before sending
  - this happens in the UI task and can be heavy for very large images; consider moving to a background task if it becomes an issue

---

## “Every model” quick index (CodexMonitorModels)

The authoritative Swift type definitions are in:
- `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift`

For a full cross-platform mapping (Swift + TypeScript + Rust), see:
- `docs/DATA_MODELS.md`

High-level buckets of models in `Models.swift`:
- Workspace: `WorkspaceInfo`, `WorkspaceSettings`, `WorktreeInfo`, `WorkspaceKind`, `WorkspaceGroup`
- Threads: `ThreadSummary`, `ThreadListResponse`, `ThreadStartResponse`, `ThreadResumeResponse`
- Conversation items: `ConversationItem` + related enums (`ConversationItemKind`, `ConversationItemRole`, ...)
- Approvals: `ApprovalRequest` + `ApprovalAction` + `ApprovalDecision`
- Git: `GitStatusResponse`, `GitFileStatus`, `GitFileDiff`, `GitLogResponse`, `BranchInfo`
- GitHub: `GitHubIssue`, `GitHubPullRequest`, `GitHubPullRequestDiffResponse`, `GitHubPullRequestCommentsResponse`
- Prompts: `CustomPromptOption`, `PromptScope`
- Account: `RateLimitSnapshot`, `CreditsSnapshot`, `ModelOption`, `CollaborationModeOption`, `SkillOption`
- Usage: `LocalUsageSnapshot` + day/totals/model breakdowns
- App-server event wrapper: `AppServerEvent`

---

## iOS entitlements / permissions required

See `ios/CodexMonitorMobile/CodexMonitorMobile/Info.plist`:

- `NSMicrophoneUsageDescription` (dictation)
- `NSSpeechRecognitionUsageDescription` (dictation)
- `NSPhotoLibraryUsageDescription` (image attachments)
- `NSLocalNetworkUsageDescription` (connecting to a local daemon)

---

## BrowserView (2026-01-26)

- **Path**: `ios/CodexMonitorMobile/CodexMonitorMobile/Views/BrowserView.swift`

### Features

| Feature | Description |
|---------|-------------|
| Session management | Create, list, close browser sessions |
| Navigation | Enter URL, navigate to pages |
| Screenshot | Display page screenshots |
| Auto-refresh | Configurable interval (3s/5s/10s) |
| Interactive controls | Click coordinates, type text |

### Auto-refresh Pause Behavior

Uses `scenePhase` environment variable to detect when the app is backgrounded:

```swift
@Environment(\.scenePhase) private var scenePhase

.task(id: "\(selectedSession ?? "")-\(autoRefresh)-\(refreshInterval)-\(scenePhase)") {
    guard autoRefresh, selectedSession != nil, scenePhase == .active else { return }
    while !Task.isCancelled {
        await refreshScreenshot()
        try? await Task.sleep(nanoseconds: UInt64(refreshInterval) * 1_000_000_000)
    }
}
```

When `scenePhase` is not `.active`, the auto-refresh task exits early. The `task(id:)` modifier causes the task to restart when `scenePhase` changes back to active.

---

## SkillsView (2026-01-26)

- **Path**: `ios/CodexMonitorMobile/CodexMonitorMobile/Views/SkillsView.swift`

### Features

| Feature | Description |
|---------|-------------|
| Skill listing | Shows all installed skills with name and description |
| Enable/disable toggle | Toggle to enable or disable each skill |
| Validation status | Shows issues (missing binaries, env vars, OS incompatibility) |
| Git installation | Install skills from git repository URL |
| Persistence | Config saved to `{CODEX_HOME}/skills/config.json` |

### State Management

```swift
@State private var validations: [SkillValidationResult] = []
@State private var skills: [SkillOption] = []
@State private var enabledSkills: Set<String> = []
@State private var installUrl: String = ""
```

### Configuration Persistence

On toggle change, writes to config via `skillsConfigWrite`:

```swift
private func persistSkillConfig() async {
    guard let workspaceId = store.activeWorkspaceId else { return }
    let enabled = skills.filter { enabledSkills.contains($0.id) }
    let disabled = skills.filter { !enabledSkills.contains($0.id) }
    await store.skillsConfigWrite(workspaceId: workspaceId, enabled: enabled, disabled: disabled)
}
```

### Skill Resolution Logic

Skills enabled/disabled state is resolved from config using:
1. If `config.enabled` is non-empty, use that list
2. Else if `config.disabled` is non-empty, enable all except those
3. Else enable all skills by default

```swift
private func resolveEnabledSkills(skills: [SkillOption], config: JSONValue) -> Set<String> {
    let enabledEntries = config["enabled"]?.arrayValue ?? []
    let disabledEntries = config["disabled"]?.arrayValue ?? []

    // Build key sets from entries
    let enabledKeys = Set(enabledEntries.compactMap { ... })
    let disabledKeys = Set(disabledEntries.compactMap { ... })

    if !enabledKeys.isEmpty { return enabledKeys }
    if !disabledKeys.isEmpty { return Set(skills.map { $0.id }).subtracting(disabledKeys) }
    return Set(skills.map { $0.id })
}
```
