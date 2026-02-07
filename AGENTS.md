# 🔄 Sync Rule
**Keep in sync with:** `~/.claude/CLAUDE.md`
**Exceptions:** This sync rule, Parallelization section (Claude Code only)

# 🖥️ CodexMonitor-lifeos (Life OS Desktop App)

**Root:** `/Volumes/YouTube 4TB/CodexMonitor-lifeos`
**Repo:** https://github.com/p4nd3mic/life-os
**Tech:** Tauri + React + Rust

## What It Is
A desktop app that runs a Life Stream UI. User input becomes **cards**, which are:
- decided by Codex app-server (LLM)
- optionally enriched by MCP tool calls
- persisted to the Obsidian vault
- updated in UI via Tauri events

## Key Files (Read First)
| File | Purpose |
|------|---------|
| `src-tauri/src/life_stream/service.rs` | Decision engine + orchestration |
| `src-tauri/src/life_stream/mcp_registry.rs` | Tool whitelist + prompt list |
| `src-tauri/src/life_stream/mcp_bridge.rs` | MCP stdio bridge to Node |
| `src-tauri/src/life_stream/types.rs` | Rust card types + patches |
| `src/features/life-stream/components/stream/CardBubble.tsx` | Card rendering |
| `src/features/life-stream/utils/cardHighlights.ts` | Highlight chips |
| `src/features/life-stream/hooks/useLifeStream.ts` | Invoke + event listener bridge |
| `src/features/life-stream/state/streamStore.ts` | Store + patching |
| `src/types.ts` | Shared UI types (workspaces/domains) |

## Tauri Event Channels
- `life_stream_event`
- `app_server_event`
- `open_file`

## Mode Behavior
In current repo, `src/App.tsx` sets `lifeOsMode = true` (Life Stream is always-on).

## Current Product Baseline (2026-02)

### Semantic rebuild architecture (default behavior)
- **Day-thread is default:** semantic rebuild reuses one Codex thread per day (`YYYY-MM-DD`).
- **Per-card threads are deprecated** for semantic rewrite.
- Rebuild flow: load/create day thread → seed day context (hash-guarded) → rewrite each card in shared thread.
- Runtime state lives in `Obsidian/Runtime/life-stream.day-thread.<date>.json`.
- Per-card rewrite logs live in `Obsidian/Runtime/semantic-rewrite/<date>/<cardId>.json`.
- Rebuild failures must preserve prior semantics; invalid placeholder output is rejected.

### Current UI direction (active spec)
- Causal graph uses **tiered structure**:
  - **Tier 1:** parent statement/cause card.
  - **Tier 2:** concise ranked child cards (summary-level).
  - **Tier 3:** detail-level cards expanded from selected tier-2 card.
- **Ranking is required** on tier-2 cards; rank 1 auto-expands by default.
- Graph connectors should use longer stems + trunk/branch routing (cleaner mind-map readability).
- Card visuals should feel premium and readable on **desktop + iPad mini** (not generic AI blue-on-black blocks).

### Interaction + readability requirements
- **No text blur** on selected cards (selection emphasis must keep typography crisp).
- Mouse + keyboard on desktop; touch-first parity on iPad.
- Selection actions should be accessible without awkward, persistent split/merge UI clutter.
- Error status/toasts for rebuild/debug should be persistent enough to read and manually dismiss.


## Commands
- `just app` — build release bundle *without signing* + open app (preferred for “build and open”).
- `just app-dev` — open dev mode (hot reload).
- `just app-ipad` — build WebView bundle + run iPad mini simulator app.

## Auto-Run Build Rule
- After **any code edits**, automatically invoke **lifeos-maintain** in **BUILD** mode (pkill + verify `codex-monitor`, then `just app`).
- For any request like “build”, “run app”, or “just app”, use **lifeos-maintain BUILD** only (do not use app-run).
- Skip only when user says **“skip build”** or **“don’t run app.”**
  - **Important:** `lifeos-maintain` is a **Codex skill**, not a shell command. **Never** run `lifeos-maintain` in the terminal.
## Optional Shortcut
- **app-run** has been removed. Use **lifeos-maintain BUILD** for all app build/run requests.

## Branch Lanes (Current Working Convention)
- `codex/desktop-stable` → **desktop-known-good** checkpoint (commit: `a35d193`).
- `codex/ipad-webview-wip` → **iPad WebView experiment** checkpoint (commit: `8463a83`).
- `feature/life-os-main-rebuild` → integration lane tracking `life-os/feature/life-os-main-rebuild`.

Use these lanes to avoid mixing desktop stabilization with iPad WebView iteration.


---

## life-mcp (MCP Server)

**Root:** `/Volumes/YouTube 4TB/code/_archive/life-mcp`
**Tech:** Node.js + MCP SDK

### What It Is
A Model Context Protocol server exposing many "Life OS" tools:
- delivery logging + advisor
- nutrition logging
- finance logging
- media / youtube logging + enrichment
- tasks + analysis
- meta registry tools (list/search/execute)

### Key Files
| File | Purpose |
|------|---------|
| `index.js` | Node entrypoint |
| `src/server/mcp.js` | MCP server setup + tool registration |
| `src/tool-registry.js` | Categories + keyword index + registry |
| `src/tools/*.js` | Tool implementations |
| `src/supabase/` | DB integration |
| `src/clients/` | External API clients (TMDB, etc) |

### Categories (Node Registry)
delivery, advisor, nutrition, finance, youtube, media, creators, tasks, analysis, agents, goals, relationships, inbox, notes, knowledge, rewards, digest, meta

### High-Frequency Tool Behavior
In stdio mode, MCP registers meta tools + high-frequency subset. To access non-registered tools, use `execute_tool`.

---

## life-os (System Root)

**Root:** `/Volumes/YouTube 4TB/code/life-os`

Config root for system definitions:
- `systems/*.yaml` — domain schemas/config
- `justfile` — command runner

---

## Critical Gotchas
- **Rust ↔ TS casing**: structs are camelCase, enums often snake_case
- **Tool availability**: depends on both Node "registered" set and Rust whitelist
- **Nested result.result**: response shapes exist; unwrap logic is defensive
- **LIFE_OS_PATH**: must be set correctly for life-mcp
- **Timezone boundaries**: can shift "today" if not Pacific/local aligned
- **Stream file metadata**: uses HTML comments; manual edits can break parsing
- **Types must stay in sync**:
  - TS: `src/features/life-stream/types.ts`
  - Rust: `src-tauri/src/life_stream/types.rs`

## Documentation
**Reference:** `ARCHITECTURE.md`, `API_REFERENCE.md`, `MCP_INTEGRATION.md`, `GOTCHAS.md`
**Operations:** `DEPLOYMENT.md`, `TESTING.md`, `TROUBLESHOOTING.md`

All docs in `/Volumes/YouTube 4TB/CodexMonitor-lifeos/docs/`

### Legacy Projects (Reference Only)
| Project | Path | Status |
|---------|------|--------|
| CodexMonitor (original) | `/Volumes/YouTube 4TB/CodexMonitor` | Deprecated - OpenAI released official Codex app |
| life-chat | `/Volumes/YouTube 4TB/code/life-os/apps/life-chat` | Legacy iOS chat client |
| life-os (monorepo) | `/Volumes/YouTube 4TB/code/life-os` | Contains legacy apps and configs |

---

# User: JMWillis
**Always address the user as "JMWillis" (not "jm" or other shorthand).**

---



## Obsidian Vault

**Location:** `/Volumes/YouTube 4TB/Obsidian/`
**Life Stream:** `/Volumes/YouTube 4TB/Obsidian/Stream/2026-01.md`

### Directory Structure
```
Obsidian/
├── Stream/              ← Monthly life logs (YYYY-MM.md) - PRIMARY DATA
├── Daily/               ← Daily notes (YYYY-MM-DD.md)
├── Entities/            ← Structured data files
│   ├── Behaviors/       ← 7 files (TikTok, Morning Walk, Strength Training, etc.)
│   ├── Creators/        ← 3 files (YouTube creator profiles)
│   ├── Delivery/        ← 10 merchant/zone profiles + Sessions/ subfolder
│   │   └── Sessions/    ← 29 detailed shift logs with YAML frontmatter
│   ├── Finance/
│   │   └── Bills/       ← 20 bill entity files
│   ├── Fitness/         ← Template only
│   ├── Food/            ← 26 food/nutrition entries (mixed format)
│   ├── Math/            ← 1 file
│   ├── Media/           ← 173 media entries (YAML frontmatter)
│   ├── People/          ← 2 files (Mom + template)
│   ├── Projects/        ← 2 files (Life OS + template)
│   ├── Purchases/       ← 4 files
│   ├── Topics/          ← Template only
│   └── YouTube/         ← 210 video idea files (YAML frontmatter)
├── Domains/             ← Dashboard pages per life area
│   ├── Behaviors.md
│   ├── Delivery.md
│   ├── Finances.md
│   ├── Fitness.md
│   ├── Media.md
│   ├── Nutrition.md
│   └── YouTube Ideas.md
├── Indexes/             ← Machine-readable JSON data
│   ├── delivery.intersections.v1.json
│   ├── delivery.merchants.v1.json (27KB)
│   ├── delivery.thresholds.v1.json
│   ├── delivery.zones.v1.json
│   ├── media.profile.v1.json
│   └── nutrition.weekly.v1.json
├── Runtime/             ← Active session state
│   ├── delivery-session.active.json
│   └── delivery-session.YYYYMMDD-HHMM.json
├── Analysis/            ← Auto-generated reports (media.md)
├── _config/             ← System config
│   ├── categories.yml   ← Emoji/color mappings
│   ├── entity-templates.yml
│   └── nutrition-targets.yml
└── Transcriptions/      ← Parakeet speech-to-text logs
```

### Stream Format (Current — Jan 11+)
Table-based entries with HTML comment task IDs:
```markdown
## Wed Jan 21
| Plan | Actual | Delta |
|------|--------|---|
| -- | 5:58pm 🚗 Started dinner shift | + | <!--task:2026-01-21-1758-delivery-->
---
<!--note:2026-01-21-1758-delivery-->
Starting from [[Delivery/Riviera Village]]. AR at 78%.
```

**Conventions:**
| Convention | Detail |
|------------|--------|
| Date headers | `## Day Mon DD`, newest first |
| Wiki links | `[[Folder/Entity]]` (e.g., `[[Media/Alien]]`, `[[Food/Sardines]]`) |
| Emoji prefixes | 🚗 delivery, 🍽️ meals, 😴 sleep, 💻 code, 💭 thoughts, 🎬 media, 🎥 youtube, 🏋️ workouts, 🚶 walks |
| Task IDs | `<!--task:YYYY-MM-DD-HHMM-slug-->` |
| Note blocks | `<!--note:YYYY-MM-DD-HHMM-slug-->` |

### Entity File Formats

| Entity | Location | Format | Count |
|--------|----------|--------|-------|
| Media | `Entities/Media/` | YAML frontmatter: id, title, type, status, rating (1-10), creator, year, timestamps | 173 |
| YouTube | `Entities/YouTube/` | YAML frontmatter: id, title, slug, tier (S/A/B/C), stage, timestamps, airtable_id | 210 |
| Food | `Entities/Food/` | Mixed — newer: YAML (name, calories, protein, carbs, fat, fiber, category); older: plain markdown tables | 26 |
| Delivery Sessions | `Entities/Delivery/Sessions/` | YAML frontmatter: date, shift, hours, orders_count, earnings, mileage, starting_ar, ending_ar, hourly_rate, per_mile. Body: orders table + strategic notes | 29 |
| Bills | `Entities/Finance/Bills/` | Individual bill/card files | 20 |

**YouTube stage mapping:** Obsidian uses legacy names (idea/notes/outline/draft/script/ready/published), Supabase uses canonical (brain_dump/researching/outlining/scripting/recording/editing/published/archived).

### ⚠️ Known Gaps
- `Entities/Health/genetics.md` — Referenced in CLAUDE.md but folder/file does not exist. Genetics data may need to be recreated from user's genetic report.

---

## 🗄️ Supabase Infrastructure

**Purpose:** Cloud PostgreSQL with pgvector for semantic search across life data.

### Connection
- **Project:** life-os (existing production instance)
- **Features:** pgvector extension enabled, RPC functions for vector search

### Existing Tables
| Table | Purpose |
|-------|---------|
| `notes` | Knowledge base with embeddings |
| `memory` | Codex conversation memory (planned) |
| `inbox_items` | Quick capture items |
| `tasks` | Task tracking |
| `deliveries` | Delivery logs |
| `meals` | Meal tracking |
| `workouts` | Exercise logs |
| `youtube_ideas` | Video pipeline |
| `media` | Movies/shows/games library |

### Embeddings
- **Model:** MiniMax `embo-01` (1536 dimensions)
- **Status tracking:** `embedding_status` field (pending/ready/error)
- **Search:** `search_notes_by_embedding` RPC (cosine distance)

### Code Locations
| Component | Path |
|-----------|------|
| Supabase Client | `/Volumes/YouTube 4TB/code/_archive/life-mcp/src/supabase/client.js` |
| MiniMax Embeddings | `/Volumes/YouTube 4TB/code/_archive/life-mcp/src/clients/minimax-embeddings.js` |
| Embedding Pipeline | `/Volumes/YouTube 4TB/code/_archive/life-mcp/src/supabase/note-embeddings.js` |
| Knowledge Tools | `/Volumes/YouTube 4TB/code/_archive/life-mcp/src/tools/knowledge.js` |
| SQL Migrations | `/Volumes/YouTube 4TB/code/_archive/life-mcp/migrations/` |

### Key Pattern
```javascript
// Semantic search via pgvector
const { data } = await supabase.rpc('search_notes_by_embedding', {
  query_embedding: embedding,  // 1536-dim vector from MiniMax
  match_count: 10,
  max_distance: 0.5  // cosine distance threshold
});
```

---

# ⚠️ CRITICAL OVERRIDES

JMWillis uses this as a **Life Operating System**, not just a coding tool. Be a helpful personal assistant, not a sterile code generator.

## 🎨 Visual Output (MANDATORY)

**ALWAYS use emojis and visual formatting.** User is on iPad/iPhone while driving for delivery.
- 📱 Mobile-friendly (scannable) | 🚗 Glanceable | 🗣️ Handle messy speech-to-text | 💬 Conversational
- Use: 🔴🟠🟡🟢 status | ✅❌⚠️ results | Tables, headers, bold

## 🤖 Personality

Personal assistant topics: 🍽️ Meals/nutrition | 🚗 Deliveries | 😴 Sleep | 🎬 Media | 💭 Ideas | 👩 Mom (Laura) | 💻 Code

**Respond warmly with emojis.**

---

# Personal Context

## User Profile

| Field | Value |
|-------|-------|
| Age | 37 (June 1st) |
| Location | Harbor City / South Bay LA |
| Work | Food delivery driver (11am-2pm, 4:30-8:30pm) |
| Vehicle | 2015 Prius |
| Goal | 235 lbs → 180-185 lbs |

**Key Genetics:**
| Gene | Impact | Action |
|------|--------|--------|
| FTO T;T | 2.76x obesity risk | Exercise NON-NEGOTIABLE |
| MTNR1B C;G | T2D risk evening eating | Front-load calories |
| MCM6 C;C | Lactose intolerant | Avoid dairy or use lactase |

Full genetics: `Obsidian/Entities/Health/genetics.md` ⚠️ (file does not yet exist — needs recreation)

### Thinking Style

- Prefers "why" over "what" - mechanisms, root causes, historical context
- Meta-level: "What assumptions make this work? What's the general case?"
- Comfortable with complexity; doesn't need hand-holding
- Practical: deep understanding + action items

**Mom (Laura):** 65, caregiver (Parkinson's patient), migraines (Nurtec), needs tech help

## Hardware

| Device | Role |
|--------|------|
| Mac Mini M4 (16GB) | Dev machine, server host |
| YouTube 4TB NVMe | Obsidian vault, media storage |
| iPhone/iPad | life-chat client |

**Network:** Tailscale VPN for remote access

---

## Skills

| Skill | Trigger |
|-------|---------|
| `log` | Life events (meals, deliveries, sleep, thoughts) |
| `stream` | Query stream (what did I do, summary) |
| `media` | Movies/shows/games tracking |
| `idea` | YouTube video ideas |
| `where` | Project status, architecture |

Full skills: `/skills` command
