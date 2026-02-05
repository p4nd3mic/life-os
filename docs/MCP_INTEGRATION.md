# MCP Integration Deep Dive (Life Stream ↔ life-mcp)

**Snapshot analyzed:** `life-os-codebase-20260202`

This doc focuses on:
- how the Rust Life Stream engine selects tools
- how it connects to the MCP server (Node)
- how tool outputs are parsed into cards + Obsidian entities
- where the “high frequency” tooling constraints come from

---

## Big picture

There are *two* relevant registries:

> **App Server is the primary harness.** MCP is used for tool execution and enrichment only. Core interactive flows (threads, turns, streaming UI events) should always go through the Codex app-server.

### 1) Rust-side whitelist (`ToolRegistry`)
**File:** `CodexMonitor-lifeos/src-tauri/src/life_stream/mcp_registry.rs`

Rust uses this to decide:
- which tools can be called from Life Stream
- which tools are “high frequency”
- which tools should appear in the decision prompt

Rust **does not** dynamically discover tools from MCP.

### 2) Node-side registry (`life-mcp ToolRegistry`)
**File:** `life-mcp/src/tool-registry.js`

Node maintains:
- the full set of tool definitions
- category definitions
- keyword index (`TOOL_KEYWORDS`)
- a high-frequency list (`HIGH_FREQUENCY_TOOL_NAMES`)

The Node MCP server registers only a subset of tools for stdio mode.

---

## Node MCP server: what’s registered vs what exists

**File:** `life-mcp/src/server/mcp.js`

In MCP stdio mode, it registers:
- **Meta tools** (4):
  - `list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`
- **High-frequency tools** (list length in this snapshot: 21)

Everything else still exists in the Node registry, but will not appear in `tools/list` and cannot be called directly unless you:
- add it to the registered set, or
- call it indirectly through `execute_tool` (meta tool)

---

## Category organization (Node)

**File:** `life-mcp/src/tool-registry.js`

Categories defined in this snapshot:

| Category | Description |
| --- | --- |
| `delivery` | Delivery session management, order advice, AR tracking. Config: life-os/systems/delivery.yaml |
| `nutrition` | Meal logging, food lookup, supplements, workouts. Config: life-os/systems/nutrition.yaml |
| `finance` | Bills, expenses, income, monthly summaries. Config: life-os/systems/finance.yaml |
| `youtube` | Video ideas pipeline, scripts, outlines. Config: life-os/systems/youtube.yaml |
| `media` | Movies, TV, games, books tracking. Config: life-os/systems/media.yaml |
| `creators` | Creators database (directors, authors, YouTubers) |
| `analysis` | Delivery analytics, patterns, suggestions |
| `tasks` | Todo list management |
| `agents` | Agent monitoring and status |
| `goals` | Goals, OKRs, projects, tasks tracking. Config: life-os/systems/goals.yaml |
| `relationships` | Contact management, interaction tracking, relationship health |
| `inbox` | Unified inbox for daily triage - tasks, follow-ups, captures |
| `rewards` | Time bank system - earn credits via activities, spend on rewards |
| `notes` | Quick note capture with auto-tagging and full-text search |
| `knowledge` | Semantic search and retrieval over notes (pgvector + embeddings) |
| `digest` | Daily and weekly summaries across all domains |

---

## High-frequency tools: mismatch you should know about

There are two “high frequency” lists:

### Node: `HIGH_FREQUENCY_TOOL_NAMES`
- Size: 21
- Source: `life-mcp/src/tool-registry.js`
- Used by: `life-mcp/src/server/mcp.js` to decide what tools are registered

### Rust: `ToolRegistry.high_frequency`
- Size: 10
- Source: `src-tauri/src/life_stream/mcp_registry.rs`
- Used by: Life Stream prompt/tool selection

**Impact:**
- Rust will only suggest/allow tools it knows about.
- Node might register more tools than Rust will ever call.
- Rust’s decision prompt might under-advertise tools that *are actually available* in MCP.

> If you add a new tool, you likely need to update both lists (or re-think the split).

---

## Intent → tool whitelist (Rust)

Rust allows calling tool sets based on an “intent” string. Current mapping:

- `log_meal` → `log_meal_quick`, `log_meal`
- `log_delivery` → `advise_order`, `add_delivery`
- `log_media` → `media_add`
- `log_youtube` → `yt_add_idea`
- `log_finance` → `log_expense`
- `log_fitness` → `log_workout`
- `log_thought` → `note_add`
- `query` → `knowledge_search`

Rule:
- If `intent` is missing → only tools in the Rust **high-frequency** set are allowed.
- If `intent` is present → allow tools for that intent.

This is a safety rail to prevent “tool explosion” in the decision prompt.

---

## Keyword-based search (Rust)

Rust has a small built-in keyword map for core Life Stream tools, used when:
- decision returns ambiguous tool selection
- user input needs keyword-driven routing

Examples:
- “doordash”, “ubereats” → `add_delivery`
- “spent $”, “bought” → `log_expense`
- “watched” + “movie” → `log_media`

Node has a richer `TOOL_KEYWORDS` map (48 entries in this snapshot) used by the meta tool `search_tools`.

---

## JSON extraction from MCP responses (Rust)

**File:** `src-tauri/src/life_stream/service.rs`

Life Stream tries to parse tool output in two channels:

1) **Text**
- concatenates all `content[]` items with `type: "text"`
- used for the card “analysis” and human-readable summary

2) **Structured JSON**
- looks for:
  - `content[]` items with `type: "json"` and a `json` field
  - JSON embedded in text (e.g. code fences)
  - nested wrappers like `{ result: {...} }`

This is why you’ll see “nested responses (result.result)” show up as a gotcha.

---

## Tool output parsing & enrichment

**File:** `src-tauri/src/life_stream/service.rs` (`apply_mcp_enrichment(...)`)

The enrichment layer is tool-aware. It applies structured fields into the `StreamCard`:

- `log_meal` / `log_meal_quick`
  - expects a `totals` object with calories/macros
  - may set `stats.calories`, `stats.protein_g`, etc

- `log_workout`
  - expects `duration_minutes` and optional `calories_burned`

- `log_media`
  - expects `media` object (title, year, tmdb_id, etc)
  - creates/updates Media entity files and triggers TMDB image fetch

- `log_youtube`
  - expects `video` metadata (title, channel, url)
  - can write YouTube entity entries

- `add_delivery`
  - expects `delivery` object with merchant, total, date/time
  - updates Delivery entities

If MCP returns only text, the card may still complete, but with fewer stats/entities.

---

## Where to change things

### Add a new tool for Life Stream
1) Implement tool in `life-mcp/src/tools/<category>.js`
2) Decide registration strategy:
   - add to Node high-frequency list **OR** expose via `execute_tool`
3) Update Rust whitelist / prompt list:
   - `src-tauri/src/life_stream/mcp_registry.rs`
4) Add parsing/enrichment (optional but recommended):
   - `src-tauri/src/life_stream/service.rs` in `apply_mcp_enrichment(...)`
5) Update TS/Rust shared types if new card fields are introduced:
   - TS: `src/features/life-stream/types.ts`
   - Rust: `src-tauri/src/life_stream/types.rs`

---

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [API_REFERENCE.md](API_REFERENCE.md)
- [DATA_MODELS.md](DATA_MODELS.md)
- [LIFE_STREAM.md](LIFE_STREAM.md)
- [GOTCHAS.md](GOTCHAS.md)
