# Research Note — Codex Harness / App Server (2026‑02‑05)

**Source:** “Unlocking the Codex harness: how we built the App Server” (OpenAI, Feb 4, 2026)

## ✅ Core takeaways (actionable)
- **App Server is the first‑class integration surface.** Codex clients (CLI, VS Code, Desktop) are built on the same harness.
- **Protocol is JSONL over stdio** (JSON‑RPC‑lite, no `jsonrpc` header).
- **Initialize handshake is mandatory** before any other request; it returns negotiated capabilities.
- **Streaming UI events** are the primary data model (items/turns/threads) — not single responses.
- **Clients should pin or bundle the Codex binary** to avoid protocol drift.

## 🔧 Why it matters for LifeOS / CodexMonitor
- **Item/Turn/Thread semantics** map cleanly onto LifeOS streaming cards, especially the Delivery Session “live card” (single turn with multiple item deltas).
- **Handshake enforcement** prevents subtle protocol mismatch bugs.
- **Pinned binaries** reduce version drift when the Codex desktop app updates.
- **Schema generation tools** (`generate-ts`, `generate-json-schema`) help keep types in sync with app-server changes.

## 🧭 Integration follow‑ups
- Align event routing with `item/started` → `item/*/delta` → `item/completed`.
- Enforce initialize-before-anything on the app-server client.
- Keep JSONL framing consistent across local + remote transports.
- Add schema sync tooling + docs.
