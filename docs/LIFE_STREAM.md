# Life Stream (Life OS feature)

**Snapshot analyzed:** `life-os-codebase-20260202`

Life Stream is the Life OS “timeline” UI:
- you type natural language
- the system decides what domain it belongs to
- optional MCP tool calls enrich the entry
- the card is persisted into Obsidian (and optionally Supabase)
- the UI updates via patch events

---

## Card lifecycle

```mermaid
stateDiagram-v2
  [*] --> pending: submit
  pending --> processing: decision turn/start
  processing --> awaiting_input: needs clarification
  processing --> completed: no tool needed OR tool success
  processing --> error: tool failure OR decision parse failure
  awaiting_input --> processing: user clarifies
  error --> processing: retry
  pending --> cancelled: cancel
  processing --> cancelled: cancel
```

### States (TS + Rust)
- `pending`
- `processing`
- `awaiting_input`
- `complete`
- `error`
- `cancelled`

**Source files**
- TS: `src/features/life-stream/types.ts`
- Rust: `src-tauri/src/life_stream/types.rs`

---

## Card types & domains

### CardType

`meal | delivery | media | youtube | expense | workout | note | misc`

### Domains (UI highlight rules)
**File:** `src/features/life-stream/utils/cardHighlights.ts`

Domains drive:
- highlight chip labels (“$35.20”, “520 cal”, “27 min”, etc)
- dashboard routing
- icon / emoji conventions

---

## Decision prompt structure (Rust)

**File:** `src-tauri/src/life_stream/service.rs` (`build_lifestream_decision_prompt`)

Key characteristics:
- instructs Codex to return a JSON decision envelope
- encourages bulk tools when possible
- includes a tool list “you may call” (from Rust ToolRegistry prompt list)
- includes constraints about timestamps, merchants, etc

> The decision prompt is the contract between Life Stream and the Codex app-server.  
> Changes to tool lists or desired JSON structure should be made intentionally.

---

## Obsidian persistence format

**File:** `src-tauri/src/life_stream/obsidian.rs`

### Stream file path

Life Stream writes to:

```
<vault>/Stream/YYYY-MM.md
```

### Day section + row insertion

For a given date:
- ensures header: `## Tue Feb 02` (weekday + month + day)
- ensures a table exists (Plan/Actual/Delta)
- inserts a new row just before the `---` separator

Example entry row pattern:

```
| -- | 12:34 Ate sushi | + | <!--task:YYYY-MM-DD-HHMM-<card_id>--> 
```

### Note block metadata (HTML comments)

After the day section separator, Life Stream writes a note block:

- `<!--note:<task_id>-->`
- `<!--prompt_b64:<base64>-->` (optional)
- `<!--response_b64:<base64>-->` (optional)
- `<!--duration_ms:<n>-->` (optional)
- then human-readable body text

This enables:
- reload/parsing existing cards
- recovering original input + assistant response
- matching rows to cards even if titles change

**Gotcha:** editing these HTML comments manually can break parsing.

---

## Image handling

### When images are fetched
Image enrichment is primarily used for:
- **media** cards (TMDB posters)
- potentially other entities with known image sources

**Rust image service**
- `src-tauri/src/life_stream/images/service.rs`

### Cache locations
Images are cached under the vault, typically:

```
Entities/Media/<Title>/cover.jpg
Indexes/media_images.json
```

The UI reads the cached local path and displays it in cards.

---

## Highlight chips per domain

**File:** `src/features/life-stream/utils/cardHighlights.ts`

Examples:
- Nutrition: calories + macros
- Finance: amount + category
- Delivery: total + merchant
- Media: rating + year + runtime
- Exercise: duration + calories burned

This is driven entirely off `card.stats` keys.

---

## Tauri commands used by Life Stream

Life Stream interacts with the backend via these `invoke(...)` targets:
- `life_stream_load_day`
- `life_stream_submit`
- `life_stream_cancel`
- `life_stream_retry`
- `life_stream_clarify`
- `life_stream_set_obsidian_root`

Full command list: [API_REFERENCE.md](API_REFERENCE.md)

---

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [API_REFERENCE.md](API_REFERENCE.md)
- [DATA_MODELS.md](DATA_MODELS.md)
- [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
- [STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)
- [GOTCHAS.md](GOTCHAS.md)

---

## 2026-02 hardening update: image pipeline + semantic tooling

### New image workflow modes (header actions)

Life Stream now supports two image-fetch workflows for the current date:

- `🖼️ Fetch images (ask)` → queues review tasks, does not auto-attach.
- `⚡ Auto-apply images` → imports top candidate, attaches to card/node, updates catalog.
- Cause/effect cards render in a vertical-first graph flow (source on top, outcomes below).

These are available from:

- `src/features/life-stream/components/navigation/LifeStreamHeaderControls.tsx`

The native cause/effect renderer now defaults to a top-down orientation:

- source/statement card on top
- responses/effects in compact rows below
- curved arrows routed downward between rows

and backed by:

- `life_stream_image_autofetch` (Tauri command)
- `LifeStreamService::auto_fetch_images_for_cards` (Rust service)

### Runtime + persistence paths used by image workflows

- Catalog: `Obsidian/Indexes/images.catalog.v1.json`
- Candidate cache inbox: `Obsidian/Runtime/ImageInbox/<entity_type>/<entity_slug>/`
- Managed assets: `Obsidian/Assets/Entities/<entity_type>/<entity_slug>/`
- Task reminders: `Obsidian/Runtime/life-stream.tasks.v1.json`

### Entity sync behavior after auto/manual attach

When `updateEntityFile=true`, Life Stream updates entity markdown frontmatter:

- `image: "Assets/Entities/..."`

When `updateEntityEmbed=true`, it also upserts:

- `## Image`

---

## 2026-02 layered causal UI v2 (ranked expansion)

Cause/effect cards now support a **3-tier reading model**:

1. **Tier 1**: source statement/question
2. **Tier 2**: concise ranked outcomes (`headline` + `summaryLine`)
3. **Tier 3**: expanded detail bullets for the currently selected Tier 2 card

### Default behavior

- Tier 2 card with `rank = 1` auto-expands first.
- Clicking another Tier 2 card switches Tier 3 details to that card.
- Clicking the selected Tier 2 card collapses Tier 3.
- Desktop keyboard:
  - `1/2/3...` select ranked Tier 2 cards
  - `Esc` collapses Tier 3

### Visual + layout upgrades

- longer connector stems and cleaner trunk/branch fanout
- increased vertical spacing between source and outcome lanes
- aligned child cards for easier scan patterns
- premium card surface tokens (reduced “flat AI box” look)
- “Most useful” badge on top-ranked Tier 2 card

### Key implementation files

- `/Volumes/YouTube 4TB/CodexMonitor-lifeos/src/features/life-stream/components/stream/CauseEffectCard.tsx`
- `/Volumes/YouTube 4TB/CodexMonitor-lifeos/src/features/life-stream/components/stream/CauseEffectCard.css`
- `/Volumes/YouTube 4TB/CodexMonitor-lifeos/src/features/life-stream/components/stream/GraphArrowLayer.tsx`
- `/Volumes/YouTube 4TB/CodexMonitor-lifeos/src/features/life-stream/components/stream/CauseEffectCard.orientation.test.tsx`
- `<!--life-stream:image-embed-->`
- `![[Assets/Entities/...]]`

### External photo roots (local-first candidate scan)

Candidate scanning now supports both common drive spellings:

- `/Volumes/YouTube 4TB/photos`
- `/Volumes/YouTube 4TB/Photos`

This avoids missing files when folder casing differs across setup scripts.
