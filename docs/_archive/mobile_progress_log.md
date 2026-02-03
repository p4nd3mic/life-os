# 📱 CodexMonitor Mobile Progress Log

> Updated by automation during implementation. Each phase entry captures the completed work and follow-ups.

## 🟢 Phase 0 — Audit / Spec / Parity Check
- **Status:** Complete ✅
- **Notes:** Generated `docs/mobile_api_parity.md` + captured full plan in `docs/mobile_plan.md`.

## 🟢 Phase 1 — Backend: Daemon Full-Parity
- **Status:** Complete ✅
- **Notes:** Added full daemon RPC parity (git, prompts, terminal, settings, usage, codex helpers), remote delegation for desktop commands, integration test `daemon_rpc.rs`, and `local_usage_core` extraction. `cargo test` passes after installing `cmake`.

## 🟢 Phase 2 — Mac Mini Deployment (Tailscale + launchd)
- **Status:** Complete ✅
- **Notes:** Added `docs/mobile_backend_setup.md` and `scripts/com.codexmonitor.daemon.plist` launchd template.

## 🟢 Phase 3 — iOS Client Library (Network + Models + State)
- **Status:** Complete ✅
- **Notes:** Added Swift package `CodexMonitorRPC` (JSONValue, RPC client, API wrapper), `CodexMonitorModels`, `CodexMonitorRendering`, and tests (`swift test` passes).

## 🟢 Phase 4 — iOS/iPadOS UI (Native + Responsive)
- **Status:** Complete ✅
- **Notes:** Added SwiftUI app scaffolding, iPhone TabView + iPad NavigationSplitView, workspaces/threads/conversation/composer/git/files/prompts/terminal/debug screens, and dictation support.

## 🟢 Phase 5 — Liquid Glass UI
- **Status:** Complete ✅
- **Notes:** Added `GlassCard`, `GlassGroup`, `GlassBadge`, `GlassChip` with iOS 26 glassEffect styling and applied to key surfaces.

## 🟢 Phase 6 — Performance, Reliability, Security
- **Status:** Complete ✅
- **Notes:** Added auto reconnect with backoff, background disconnect + foreground reconnect, token Keychain storage, and safe event handling.

## 🟡 Phase 7 — Deliverables / Acceptance Tests
- **Status:** In progress 🟡
- **Notes:** Added `docs/mobile_acceptance_checklist.md` for on-device testing. Deliverables added (Xcode project, Swift package, daemon parity). Automated tests: `cargo test` + `swift test` passing. Manual iOS acceptance checklist still pending.

## 🟢 Phase 8 — UI Polish & Gradient Themes
- **Status:** Complete ✅
- **Notes:**
  - Added `ThemeGradient.swift` with 4 selectable gradient backgrounds (Midnight Blue, Ocean Deep, Cosmic Purple, Slate Dark)
  - Added `ColorExtension.swift` with hex color initializer
  - Added `GradientBackground.swift` — reusable gradient view with `@AppStorage` theme binding
  - Key insight: NavigationSplitView has opaque UIKit layers that can't be cleared from outside
  - **Solution:** Apply gradient INSIDE each column view using `.background { GradientBackground() }` + `.scrollContentBackground(.hidden)`
  - Updated all views (RootView, ThreadsListView, WorkspaceListView, ConversationTabView, GitView, FilesView, PromptsView, TerminalView, DebugLogView, ProjectsView) with gradient backgrounds
  - Updated `ThreadsListView.swift` with smart timestamp detection (ms vs seconds), UUID name fallback display, and selection visual feedback
  - Updated `SettingsView.swift` with new Appearance section for gradient picker
  - Fixed Xcode project.pbxproj to include new utility files in build

## 🧩 2026-01-24 — iOS Thread History Sync Fix
- **Status:** Complete ✅
- **Notes:**
  - Fixed iOS thread list names to use `preview` when `name/title` are missing.
  - Added decoding for `ThreadTurn` + `ThreadRecord.preview` + `ThreadRecord.turns`.
  - `resume_thread` now loads historical turns into `itemsByThread`.
  - Conversation history now renders `agentMessage` items as assistant messages.
  - Added debug-panel entry `thread_history_loaded` with turn/item counts.

## 🧩 2026-01-24 — iOS Stability & UX Fixes
- **Status:** Complete ✅
- **Notes:**
  - Added `NSLocalNetworkUsageDescription` to Info.plist for LAN/Tailscale connections.
  - Capped thread history in memory (max 500 items per thread) and added helper to trim buffers.
  - Capped terminal output per session (max 10,000 characters).
  - Fixed dictation audio session leak (deactivate on stop).
  - Fixed auto-scroll during streaming deltas by tracking last item signature.
  - Added image upload limits (resize to 1920px max, compress to JPEG 0.7, reject >2MB) with user-facing error alert.

## 🧩 2026-01-24 — GPT 5.2 Pro Review Fixes Pass
- **Status:** Complete ✅
- **Notes:**
  - Updated NSLocalNetworkUsageDescription copy to include "manage Codex sessions".
  - Enforced terminal buffer cap at 50k chars per session.
  - Updated dictation stop to deactivate audio session with error logging.
  - Adjusted auto-scroll to track last item hash (streaming deltas).
  - Added fallback image compression (0.4) when >2MB after primary compression.
  - Standardized path comparison via URL.standardized.path in `pathsEquivalent`.
  - Verified `swift build` for CodexMonitorRPC package.

## 🧩 2026-01-24 — Glass Effect & Color Unification
- **Status:** Complete ✅
- **Notes:**
  **iOS Changes:**
  - Fixed portrait orientation on iPad (Info.plist)
  - Fixed message bubble colors — now uses `.glassEffect(.regular.tint(bubbleColor))` for vibrant sky-blue (assistant) and green (user) glass bubbles
  - Chat-style layout with bubbles on opposite sides (assistant left, user right)
  - Dynamic bubble width (~68% max on compact, 60% on regular)
  - Replaced AppColors with direct hex colors via `Color(hex:)` extension

  **Mac/Desktop Changes:**
  - Unified glass theme with slate dark default
  - Matched iOS colors exactly (#0A84FF blue, #3DBD73 green → rgba versions)
  - Lowered panel opacity so gradient shows through
  - Fixed empty space on right/bottom with 100vw/100vh fill
  - Removed internal gradients from bubbles for cleaner look
