# State Management (React + Rust)

**Snapshot analyzed:** `life-os-codebase-20260202`

This doc explains how “state” flows across:
- React components (UI state)
- Life Stream data store (cards list)
- Tauri event bus (Rust → UI)
- Rust services (LifeStreamService, Codex sessions)

---

## React state (Life Stream)

### 1) `streamStore` — persistent card data

**File:** `src/features/life-stream/state/streamStore.ts`

This is a minimal custom store backed by:
- an in-memory `state` object
- a `Set<listener>` subscription list
- `useSyncExternalStore(...)` for React-safe subscription

It stores only the durable Life Stream data:
- `cards: StreamCard[]`

Key actions:
- `setCards(cards)`
- `upsertCard(card)`
- `applyCardPatch(patch, newVersion)`
- `removeCard(cardId)`

> There is **no Zustand** in this snapshot; `streamStore` is a lightweight equivalent.

### 2) `useLifeStream` — orchestration + ephemeral UI state

**File:** `src/features/life-stream/hooks/useLifeStream.ts`

This hook owns:
- Loading state: `isLoading`, `loadError`
- UI state: `currentDate`, `expandedCardId`, `emojiFilters`
- Side-effects:
  - `invoke(...)` calls to the Rust backend
  - `listen("life_stream_event", ...)` to receive patches

---

## Rust state (backend)

### AppState owns the long-lived services

**File:** `src-tauri/src/state.rs`

`AppState` holds:
- `workspaces` (persisted json)
- `sessions` (Codex app-server sessions per workspace)
- `app_settings` (includes `life_mcp_path`, `life_os_root`, TMDB key, etc.)
- `life_stream_service: Mutex<LifeStreamService>`

This is the canonical place where the Life Stream service is constructed, including:
- MCP path (`life_mcp_path`) and `LIFE_MCP_PATH`
- life-os root (`life_os_root`) and `LIFE_OS_PATH`
- TMDB API key (optional)

### `LifeStreamService` — the “stateful brain”

**File:** `src-tauri/src/life_stream/service.rs`

Key internal state:
- `cards: HashMap<String, StreamCard>`
- `worker_semaphore: Semaphore` — caps concurrent card processing (set to 5)
- `write_locks: HashMap<String, Arc<Mutex<()>>>` — serialize Obsidian writes per day/workspace
- `codex_locks: HashMap<String, Arc<Mutex<()>>>` — serialize Codex turns per thread/day
- `cancellation: HashSet<String>` — cancel card processing by id
- `tool_registry: ToolRegistry` — whitelist + category + keyword mapping
- `mcp_bridge: LifeMcpBridge` — stdio JSON-RPC to Node MCP
- `images: ImageService` — TMDB fetch + vault cache

---

## Workspace / model / session flow

### Workspace selection
- UI selects a workspace (`workspaces` feature)
- Life Stream calls include `workspaceId` on every Tauri command

### “One thread per day”
**File:** `src-tauri/src/life_stream/mod.rs`

Life Stream uses a day-based mapping:
- Runtime file: `Runtime/life-os-threads.json`
- Key: `"<workspace_id>:<YYYY-MM-DD>"`
- Value: Codex `thread_id`

This keeps day context consistent and avoids mixing unrelated days.

### Model & effort selection
Life Stream `SubmitCardRequest` includes:
- `model` (string)
- `effort` (string)
- `approval_policy` (string)

These are fed into the app-server `turn/start` parameters.

---

## Event emission patterns (Tauri)

### Backend → UI updates

Rust emits a **single event channel** for Life Stream:
- event name: `life_stream_event`

Payload is a `LifeStreamEvent` union, including:
- `CardCreated`
- `CardUpdated` (patch + newVersion)
- `CardCompleted`
- `CardError`
- `CardStep` (status/progress)

Front-end handles these in `useLifeStream`, and routes them into:
- `streamStore.applyCardPatch(...)`
- local hook state for “current date”, “expanded card”, etc.

### Versioning (stale update protection)
Each card has `version` (integer). Rust increments, then sends:
- patch + `newVersion`

UI only applies patches when:
- `newVersion > current.version`

This prevents out-of-order async tasks (MCP, images) from overwriting newer state.

---

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [API_REFERENCE.md](API_REFERENCE.md)
- [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
- [LIFE_STREAM.md](LIFE_STREAM.md)
- [GOTCHAS.md](GOTCHAS.md)
