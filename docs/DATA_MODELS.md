# Life OS Data Models

**Snapshot analyzed:** `life-os-codebase-20260202`

This doc covers the shared “contracts” across:
- React/TypeScript UI
- Rust backend
- MCP tool schemas (life-mcp)
- Obsidian vault persistence (markdown + YAML + JSON indexes)

---

## Naming conventions & serialization rules

### JSON casing rules you’ll see in practice
- **Rust struct fields** are `snake_case` in code…
- …but most Life Stream structs serialize as **`camelCase` JSON** via `#[serde(rename_all = "camelCase")]`.
- **Enums** often serialize as **`snake_case` strings** (`pending`, `awaiting_input`, etc).

This means:
- Rust: `occurred_at` ↔ JSON/TS: `occurredAt`
- Rust enum variant: `AwaitingInput` ↔ JSON/TS: `"awaiting_input"`

---

## Life Stream core types

### Cross-platform mapping: `StreamCard`

| Rust field | JSON / TS key | Rust type | TS type | Required? |
|---|---|---|---|---|
| `id` | `id` | `String` | `string` | required |
| `occurred_at` | `occurredAt` | `String` | `string` | required |
| `created_at` | `createdAt` | `String` | `string` | required |
| `updated_at` | `updatedAt` | `String` | `string` | required |
| `version` | `version` | `u32` | `number` | required |
| `card_type` | `cardType` | `CardType` | `CardType` | required |
| `domain` | `domain` | `DomainId` | `DomainId` | required |
| `emoji` | `emoji` | `String` | `string` | required |
| `state` | `state` | `CardState` | `CardState` | required |
| `processing_step` | `processingStep` | `Option<String>` | `string` | optional |
| `processing_steps` | `processingSteps` | `Option<Vec<String>>` | `string[]` | optional |
| `title` | `title` | `String` | `string` | required |
| `subtitle` | `subtitle` | `Option<String>` | `string` | optional |
| `summary` | `summary` | `Option<String>` | `string` | optional |
| `duration_ms` | `durationMs` | `Option<i64>` | `number` | optional |
| `image` | `image` | `Option<CardImage>` | `CardImage` | optional |
| `stats` | `stats` | `Option<HashMap<String, CardStatValue>>` | `Record<string, CardStatValue>` | optional |
| `entities` | `entities` | `Option<Vec<EntityRef>>` | `EntityRef[]` | optional |
| `original_input` | `originalInput` | `Option<String>` | `string` | optional |
| `source` | `source` | `Option<CardSource>` | `{ streamFile?: string; streamAnchor?: string }` | optional |
| `expanded` | `expanded` | `Option<ExpandedContent>` | `ExpandedContent` | optional |
| `clarification_options` | `clarificationOptions` | `Option<Vec<ClarificationOption>>` | `ClarificationOption[]` | optional |
| `error_message` | `errorMessage` | `Option<String>` | `string` | optional |

**Source files**
- TS: `src/features/life-stream/types.ts`
- Rust: `src-tauri/src/life_stream/types.rs`

### Card type enums

**TS (`CardType`)**:  
`"meal" | "delivery" | "media" | "youtube" | "expense" | "workout" | "note" | "misc"`

**Rust (`CardType`)** (serialized `snake_case`):  
`meal | delivery | media | youtube | expense | workout | note | misc`

> The TypeScript and Rust enums must stay in sync (see [GOTCHAS.md](GOTCHAS.md)).

### Stats / Entities

- `stats`: a loose key/value map (`Record<string, number | string>`)
- `entities`: structured references `{ type, name, id?, metadata? }`

These are mostly produced by:
- the Codex decision JSON
- the MCP tool JSON payload (if a tool is called)

---

## life-mcp tool schemas

Tools define their input schemas in two styles:

### 1) Zod param-map style (most tools)

Example pattern:

```js
inputSchema: {
  merchant: z.string().describe('Restaurant name'),
  total: z.number().describe('Total amount'),
  date: z.string().optional().describe('ISO date'),
}
```

### 2) JSON Schema style (notes / inbox / knowledge)

Example pattern:

```js
inputSchema: {
  type: 'object',
  properties: {
    query: { type: 'string' },
    limit: { type: 'number', default: 20 },
  },
  required: ['query'],
}
```

### Tool output format (dual channel)

Tools return an MCP response like:

```json
{
  "content": [
    { "type": "text", "text": "Human readable summary..." },
    { "type": "json", "json": { "structured": "data" } }
  ]
}
```

The Rust Life Stream parser supports multiple variants (embedded JSON, nested `{ result: ... }`, etc). See:
- `src-tauri/src/life_stream/service.rs` → `extract_mcp_json(...)` / `extract_mcp_text(...)`

---

## Obsidian Vault formats

> The snapshot includes `Obsidian-samples/` to illustrate structure and schemas.

### Stream log files

**Expected structure (real vault):**
- `Stream/YYYY-MM.md` (monthly)
  - daily headings `## YYYY-MM-DD` or similar
  - rows/cards appended under the day

In snapshot:
- `Obsidian-samples/2026-02.md` shows the style and metadata blocks used.

**Life Stream persistence code:**
- `src-tauri/src/life_stream/obsidian.rs`  
  (append entries, ensure day headers, manage entity files)

### Entity files (YAML frontmatter)

#### Media entity example

```md
---
id: "2648182D-E72E-4A05-8B3F-ED48A06D32A4"
title: "28 Days Later"
type: "Film"
status: "Completed"
rating: 8
created_at: "2026-01-07T21:30:35.611Z"
updated_at: "2026-01-07T22:00:36.009Z"
completed_at: "2026-01-07T21:30:35.611Z"
---

# 28 Days Later

## Notes
Best zombie movie. Would be higher without digital camera look.

## Mentions
<!-- Auto-updated when referenced in Stream -->
```

#### YouTube entity example

```md
---
id: "3e595dd9-ce1e-48c0-aebc-dbaa00e61e42"
type: "youtube"
title: "A Love Letter To Bad Games"
slug: "a-love-letter-to-bad-games"
tier: "C"
stage: "idea"
created_at: "2025-12-11T00:00:00Z"
updated_at: "2025-12-11T00:00:00Z"
aliases: []
airtable_id: "recXECmbqUcoQZKXF"
---
# A Love Letter To Bad Games
## Status
- Tier: **C**
- Stage: **idea**

## Thesis

Some games are objectively flawed but personally beloved. That's valid.
## Pillars


## Hooks


## Research


## Script


## Log

Personal, relatable.
## Mentions
<!-- Auto-updated by Claude -->
```

#### Delivery entity example

```md
# Corner Bakery

## Type
- Restaurant
- DoorDash merchant

## First Seen
- [[2026-01-11]]

## Mentions
<!-- Auto-updated by Claude -->
- 2026-01-11 - Accepted 12:22pm, completed ~12:32pm: $8.00 / 1.3 mi (~10 min)
```

### `_config/categories.yml`

Defines the mapping of category → emoji/color used for consistent UI + tags.

```yml
# Category Emoji and Color Mappings
categories:
  wake:
    emoji: "☀️"
    color: "textMuted"
  food:
    emoji: "🍽️"
    color: "domainFood"
  money:
    emoji: "💰"
    color: "success"
  purchases:
    emoji: "🛒"
    color: "syntaxOrange"
  tech:
    emoji: "💻"
    color: "syntaxPurple"
  thoughts:
    emoji: "💡"
    color: "syntaxYellow"
  fitness:
    emoji: "🏃"
    color: "domainMove"
  decisions:
    emoji: "🎯"
    color: "headingText"
  home:
    emoji: "🏠"
    color: "textSecondary"
  finance:
    emoji: "💳"
    color: "syntaxOrange"
  work:
    emoji: "💼"
    color: "domainGig"
  media:
    emoji: "🎬"
    color: "domainMedia"
  delivery:
    emoji: "🚗"
    color: "domainGig"
  behaviors:
    emoji: "📊"
    color: "info"
  misc:
    emoji: "📝"
    color: "textSecondary"
```

### Indexes (machine-readable JSON)

Examples in snapshot:
- `Indexes/media_images.json` — TMDB/image cache metadata
- `Indexes/people_zones.json` — people/location zoning
- `Indexes/stats_*.json` — aggregated stats snapshots

These are primarily for **fast lookup** and **render-time enrichment**.

---

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [API_REFERENCE.md](API_REFERENCE.md)
- [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
- [LIFE_STREAM.md](LIFE_STREAM.md)
- [STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)
- [GOTCHAS.md](GOTCHAS.md)
