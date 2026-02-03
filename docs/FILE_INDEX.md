# File Index / Codebase Map

**Snapshot analyzed:** `life-os-codebase-20260202`
**Snapshot root:** `/Volumes/YouTube 4TB/`

This file maps the major directories and the “where do I change X?” hotspots.

---

## Project roots

| Project | Snapshot path | Key entrypoints |
|---|---|---|
| CodexMonitor-lifeos | `/Volumes/YouTube 4TB/CodexMonitor-lifeos` | `src/App.tsx`, `src/features/life-stream/**`, `src-tauri/src/lib.rs`, `src-tauri/src/life_stream/**` |
| life-mcp | `/Volumes/YouTube 4TB/code/_archive/life-mcp` | `index.js`, `src/server/mcp.js`, `src/tool-registry.js`, `src/tools/*.js` |
| Obsidian Vault | `/Volumes/YouTube 4TB/Obsidian` | `Stream/2026-*.md`, `Entities/*`, `Indexes/*`, `_config/categories.yml` |
| life-os | `/Volumes/YouTube 4TB/code/life-os` | `systems/*.yaml`, `justfile` |

---

## CodexMonitor-lifeos

### Top-level
- `package.json` — front-end deps + scripts
- `src/` — React UI
- `src-tauri/` — Rust backend + Tauri commands
- `src/styles/` — global + feature CSS

> Note: The prompt mentions `src/stores/` and Zustand stores. **In this snapshot there is no Zustand usage**; Life Stream state is implemented as a lightweight custom store using `useSyncExternalStore`.

### Life Stream UI (`src/features/life-stream/`)

All files in this feature folder (42 files):

- `src/features/life-stream/__tests__/streamStore.test.ts`
- `src/features/life-stream/components/LifeMessageRow.css`
- `src/features/life-stream/components/LifeMessageRow.tsx`
- `src/features/life-stream/components/LifeStreamLogPanel.css`
- `src/features/life-stream/components/LifeStreamLogPanel.tsx`
- `src/features/life-stream/components/LifeStreamMessageView.css`
- `src/features/life-stream/components/LifeStreamMessageView.tsx`
- `src/features/life-stream/components/LifeStreamTracePanel.css`
- `src/features/life-stream/components/LifeStreamTracePanel.tsx`
- `src/features/life-stream/components/LifeStreamView.css`
- `src/features/life-stream/components/LifeStreamView.tsx`
- `src/features/life-stream/components/composer/LifeStreamComposer.css`
- `src/features/life-stream/components/composer/LifeStreamComposer.test.tsx`
- `src/features/life-stream/components/composer/LifeStreamComposer.tsx`
- `src/features/life-stream/components/composer/StreamComposer.css`
- `src/features/life-stream/components/composer/StreamComposer.tsx`
- `src/features/life-stream/components/navigation/DayPicker.tsx`
- `src/features/life-stream/components/navigation/EmojiFilters.tsx`
- `src/features/life-stream/components/navigation/LifeStreamHeaderControls.css`
- `src/features/life-stream/components/navigation/LifeStreamHeaderControls.tsx`
- `src/features/life-stream/components/navigation/LifeTopbar.css`
- `src/features/life-stream/components/navigation/LifeTopbar.test.tsx`
- `src/features/life-stream/components/navigation/LifeTopbar.tsx`
- `src/features/life-stream/components/stream/CardBubble.click.test.tsx`
- `src/features/life-stream/components/stream/CardBubble.css`
- `src/features/life-stream/components/stream/CardBubble.test.tsx`
- `src/features/life-stream/components/stream/CardBubble.tsx`
- `src/features/life-stream/components/stream/CardErrorBoundary.tsx`
- `src/features/life-stream/components/stream/CardImage.tsx`
- `src/features/life-stream/components/stream/CardItem.test.tsx`
- `src/features/life-stream/components/stream/CardItem.tsx`
- `src/features/life-stream/components/stream/CardList.tsx`
- `src/features/life-stream/components/stream/ExpandedCard.test.tsx`
- `src/features/life-stream/components/stream/ExpandedCard.tsx`
- `src/features/life-stream/components/stream/ProcessingIndicator.tsx`
- `src/features/life-stream/components/stream/StreamCardExtras.css`
- `src/features/life-stream/context/LifeStreamContext.tsx`
- `src/features/life-stream/hooks/useLifeStream.ts`
- `src/features/life-stream/state/streamStore.ts`
- `src/features/life-stream/types.ts`
- `src/features/life-stream/utils/cardHighlights.ts`
- `src/features/life-stream/utils/cardTitle.ts`

**Most important UI files**
- `src/features/life-stream/LifeStreamView.tsx` — main view
- `src/features/life-stream/components/LifeStreamComposer.tsx` — input composer
- `src/features/life-stream/components/stream/CardBubble.tsx` — card renderer
- `src/features/life-stream/hooks/useLifeStream.ts` — bridge: invoke + event listener
- `src/features/life-stream/state/streamStore.ts` — state container + patching logic
- `src/features/life-stream/utils/cardHighlights.ts` — highlight chip rules per domain

### Life Stream Rust backend (`src-tauri/src/life_stream/`)

All files in the Rust Life Stream subsystem:

- `src-tauri/src/life_stream/events.rs`
- `src-tauri/src/life_stream/handlers/code_task.rs`
- `src-tauri/src/life_stream/handlers/delivery.rs`
- `src-tauri/src/life_stream/handlers/media.rs`
- `src-tauri/src/life_stream/handlers/mod.rs`
- `src-tauri/src/life_stream/handlers/nutrition.rs`
- `src-tauri/src/life_stream/handlers/query.rs`
- `src-tauri/src/life_stream/handlers/thought.rs`
- `src-tauri/src/life_stream/images/cache.rs`
- `src-tauri/src/life_stream/images/fetchers/food.rs`
- `src-tauri/src/life_stream/images/fetchers/mod.rs`
- `src-tauri/src/life_stream/images/fetchers/tmdb.rs`
- `src-tauri/src/life_stream/images/mod.rs`
- `src-tauri/src/life_stream/logging.rs`
- `src-tauri/src/life_stream/mcp_bridge.rs`
- `src-tauri/src/life_stream/mcp_registry.rs`
- `src-tauri/src/life_stream/mod.rs`
- `src-tauri/src/life_stream/obsidian.rs`
- `src-tauri/src/life_stream/service.rs`
- `src-tauri/src/life_stream/service_test.rs`
- `src-tauri/src/life_stream/tests.rs`
- `src-tauri/src/life_stream/types.rs`

**Most important backend files**
- `service.rs` — decision engine + orchestration + enrichment
- `mcp_registry.rs` — tool whitelist + tool prompt list + keyword mapping
- `mcp_bridge.rs` — JSON-RPC MCP stdio bridge to Node `life-mcp`
- `obsidian.rs` — persistence (append stream entries, write entity files)
- `images/service.rs` — TMDB fetch + local cache writing
- `logging.rs` — runtime logs (`Runtime/life-stream.log`)

### Other important UI subsystems
- `src/features/life/` — dashboards (delivery/nutrition/finance/media/youtube/exercise)
- `src/features/workspaces/` — workspace create/switch + routing
- `src/services/tauri.ts` — typed wrappers around `invoke(...)`

---

## life-mcp

### Top-level
- `index.js` — Node entrypoint (MCP stdio server)
- `src/server/mcp.js` — MCP server setup + tool registration
- `src/tool-registry.js` — categories, keyword index, meta tools, registry
- `src/tools/*.js` — tool implementations (by domain/category)
- `src/supabase/` — DB integration
- `src/clients/` — external API clients (TMDB, etc)
- `src/config/` — config loader (uses `LIFE_OS_PATH`)

### Tool modules (`src/tools/`)
All tool modules present in snapshot:

- `src/tools/advisor.js`
- `src/tools/agents.js`
- `src/tools/analysis.js`
- `src/tools/creators.js`
- `src/tools/delivery.js`
- `src/tools/digest.js`
- `src/tools/finance.js`
- `src/tools/goals.js`
- `src/tools/inbox.js`
- `src/tools/knowledge.js`
- `src/tools/media.js`
- `src/tools/meta.js`
- `src/tools/notes.js`
- `src/tools/nutrition.js`
- `src/tools/relationships.js`
- `src/tools/rewards.js`
- `src/tools/tasks.js`
- `src/tools/youtube.js`

---

## Obsidian Vault (sample)

Path: `/Volumes/YouTube 4TB/Obsidian`

The primary Obsidian vault with stream logs, entity files, and indexes.

Useful example files:

- `2026-02.md`
- `_config/categories.yml`
- `Entities/Delivery/Corner Bakery.md`
- `Entities/Media/28 Days Later.md`
- `Entities/YouTube/A Love Letter To Bad Games.md`
- `Indexes/media_images.json`
- `Indexes/stats_daily.json`

---

## life-os (system root)

Path: `/Volumes/YouTube 4TB/code/life-os`

Key components:
- `systems/*.yaml` — domain configuration (categories, schemas, dashboards)
- `justfile` — command runner / task shortcuts
- `docs/ARCHITECTURE.md` — system docs (life-os-level)

Systems present in snapshot:

- `systems/delivery.yaml`
- `systems/finance.yaml`
- `systems/goals.yaml`
- `systems/knowledge.yaml`
- `systems/media.yaml`
- `systems/nutrition.yaml`
- `systems/youtube.yaml`

---

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [API_REFERENCE.md](API_REFERENCE.md)
- [DATA_MODELS.md](DATA_MODELS.md)
- [STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)
- [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
- [LIFE_STREAM.md](LIFE_STREAM.md)
- [GOTCHAS.md](GOTCHAS.md)
