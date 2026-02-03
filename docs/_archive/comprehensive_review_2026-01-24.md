# 📋 CodexMonitor Comprehensive Review (2026‑01‑24)

> Scope: repo architecture, docs, tests, backend/daemon, and **iOS/mobile implementation**.  
> Sources reviewed: `docs/mobile_progress_log.md`, iOS app + packages, Rust daemon + remote backend, desktop app structure, docs/tests.

---

# ✅ Executive Summary (Main Findings)

| Severity | Area | Finding | Why it matters |
|---|---|---|---|
| 🟡 | iOS Build | **Deployment target set to iOS 26.0** | **OK if you only run iOS 26**, but blocks installs on older devices. |
| 🔴 | Remote Daemon Security | **Plain TCP + static token + terminal shell** | Any token leak = full remote shell access. Requires strong transport controls. |
| 🟠 | iOS Memory/UX | **Unbounded terminal + conversation buffers** | Long sessions can spike memory & crash. |
| 🟠 | Architecture | **IPC contract drift risk** (TS vs Rust duplication) | Runtime bugs when surfaces diverge; hard to diagnose. |
| 🟠 | Docs | **Remote backend + mobile setup docs are outdated** | Onboarding fails or misleads. |
| 🟡 | Tests | **Sparse backend + iOS tests** | High‑risk areas not covered. |

---

# 📱 iOS / Mobile Review (Key Issues & Gaps)

## 🟡 Compatibility Note (Not a blocker if you only use iOS 26)
- **Deployment target is iOS 26.0** (XcodeGen + Xcode project).  
  ✅ **If you only run iOS 26**, this is fine and not a problem.  
  ⚠️ If you ever want broader device support, lower the target in `ios/CodexMonitorMobile/project.yml` and regenerate.

## 🟠 High Impact
- **Transport security:** RPC over raw TCP with token auth. No TLS, no server identity, token transmitted in cleartext.  
  **Fix:** enforce Tailscale/SSH tunnel OR add TLS/mTLS; at minimum guard daemon to localhost + require Tailscale.
- **Unbounded buffers:** `itemsByThread` + `terminalOutputBySession` grow forever.  
  **Fix:** implement paging/trim (e.g., keep last N items / N KB per terminal).
- **Dictation audio session never deactivated.**  
  **Fix:** call `AVAudioSession.setActive(false)` on stop.
- **Streaming auto‑scroll doesn’t update on delta text** (only on count changes).  
  **Fix:** scroll on item text change or when receiving deltas.
- **Image attachments are base64’d raw with no size limits.**  
  **Fix:** resize/compress, enforce cap, surface errors.

## 🟡 Medium
- **No local‑network usage description.** iOS 14+ may block local LAN access without `NSLocalNetworkUsageDescription`.  
- **Terminal session lifecycle:** uses a random `terminalId` per view; if workspace changes, session is not re‑opened/closed.  
- **Thread filtering uses strict path equality** but helper `pathsEquivalent` exists and isn’t used (risk: threads missing for symlinked or nested paths).
- **Composer missing advanced parity features** (queue mode, model picker, prompt/file autocomplete, non‑image attachments).

---

# 🦀 Backend / Daemon Review (Key Issues)

## 🔴 Critical
- **Full shell access once authenticated** (`terminal_open` + `terminal_write`). Any token leak = RCE on host.  
  **Mitigation:** enforce local‑only bind or require Tailscale/SSH; consider additional command‑level authorization.
- **Plain TCP + static token.** No TLS/mTLS.  
  **Mitigation:** transport‑level protection or explicit TLS.

## 🟠 High Impact
- **No request timeouts** → hung calls + pending growth.  
- **Unbounded channels + line reads** → memory / DoS risk under large payloads or slow readers.

## 🟡 Medium
- **Token stored in plain JSON settings** (`settings.json`).  
- **No capability/version negotiation**; mismatched clients fail at runtime.
- **File listing includes hidden files** (`.env`, `.ssh`, etc.).

---

# 🧩 Architecture / Repo Review

- **IPC contract duplication:** TS `src/services/tauri.ts` vs Rust `src-tauri/src/lib.rs` + duplicated types (`src/types.ts`, `src-tauri/src/types.rs`).  
  → High risk of drift and silent bugs.
- **Large monoliths:** `SettingsView.tsx`, `useThreads.ts`, `App.tsx`, and daemon file are huge (1–4k LOC).  
  → Hard to review, easy to regress.

---

# 📚 Docs Review

- `REMOTE_BACKEND_POC.md` is outdated (still written as POC).  
- `docs/mobile_backend_setup.md` has **wrong paths** (points to `.../code/CodexMonitor`).  
- README doesn’t surface mobile/daemon workflow → discoverability gap.

---

# ✅ Test Coverage Gaps (Summary)

| Area | Current | Gap |
|---|---|---|
| Rust daemon | Minimal (`daemon_rpc.rs` smoke) | No auth/terminal/git/prompts/settings/usage tests |
| iOS | JSONValue tests only | No RPC/notification/UI smoke tests |
| Frontend | ~30 tests | No approval/terminal/markdown/dictation flow tests |

---

# 🧪 Recommended Tests to Add

## 🦀 Rust (Daemon + Remote Backend)
1. **Auth tests:** invalid/missing token should reject; valid token should pass.
2. **Terminal event test:** open terminal → write → verify `terminal-output` notification.
3. **Settings round‑trip:** `get_app_settings` + `update_app_settings`.
4. **Prompts CRUD:** create/update/move/delete in temp dirs.
5. **Git ops:** init repo → status/diff/stage/commit/revert.
6. **Remote backend client:** pending request resolution + event fan‑out.

## 📱 iOS (Swift)
1. **RPC client:** auth handshake + reconnect/backoff; decode errors.
2. **Notification parsing:** `app-server-event`, `terminal-output`.
3. **Model decode:** `ThreadSummary`, `GitStatus`, `RateLimitSnapshot`.
4. **UI smoke:** RootView → Projects → Threads → Conversation.

## 🧑‍💻 Frontend (Vitest)
1. **Markdown + diff rendering** (code fences, tool outputs).
2. **Approval flow** (request + respond).
3. **Terminal output append** behavior.
4. **Settings remote backend validation** (host/token required).

---

# 🛠️ Proposed Fix Plan (Prioritized)

## Phase 1 — 🔴 Safety / Build Blockers
- [ ] **Confirm** iOS 26‑only support is intentional. (If not, lower deployment target.)
- [ ] Add `NSLocalNetworkUsageDescription` to iOS Info.plist.
- [ ] Enforce daemon security: require Tailscale/SSH or TLS; hard‑fail insecure mode unless loopback.

## Phase 2 — 🟠 Stability / UX
- [ ] Add paging/trimming for conversation + terminal output.
- [ ] Fix streaming auto‑scroll on delta updates.
- [ ] Compress/resize images before base64 encode; enforce size limits.
- [ ] Deactivate audio session after dictation stop.

## Phase 3 — 🟡 Architecture + Docs
- [ ] Add typed IPC schema or shared contract generation.
- [ ] Split monoliths (SettingsView, useThreads, daemon modules).
- [ ] Update docs: remote backend + mobile setup + README discoverability.

## Phase 4 — 🧪 Test Coverage
- [ ] Add daemon auth/terminal/git tests.
- [ ] Add iOS RPC + notification unit tests.
- [ ] Add frontend approval/terminal/markdown tests.

---

# 🧭 Notes from `docs/mobile_progress_log.md`
- Progress log claims all phases complete, but **iOS build target + missing parity features** mean mobile is not truly production‑ready yet.  
- Acceptance checklist exists, but it is **manual only** — no automated iOS integration tests yet.

---

# ✅ Next Actions (Suggested)

1) Confirm intended **iOS minimum version**.  
2) Decide security posture (Tailscale‑only vs TLS).  
3) I can implement Phase 1 fixes + scaffold tests if you want.
