# ClawdBot Feature Analysis for CodexMonitor

> Comprehensive analysis of ClawdBot's Automatic Memory, Browser Integration, and Skill System
> For use in designing similar features for CodexMonitor

---

## 1. Automatic Memory System

### Overview
ClawdBot uses a **Markdown-first** memory system stored locally, with optional vector search via embeddings.

### Memory Storage Structure
```
~/clawd/
├── memory/                    # Daily memory logs (one file per day)
│   ├── 2026-01-26.md
│   ├── 2026-01-25.md
│   └── ...
├── bank/                      # Curated long-term memory (stable pages)
└── instructions                # Agent operating instructions
```

### Automatic Memory Capture

#### Pre-Compaction Memory Flush (Key Innovation)
When a session nears context window limits, ClawdBot triggers **automatic memory save**:

1. **Soft Threshold Detection** - When token count crosses ~4000 tokens before limit
2. **Silent Agentic Turn** - Runs invisible to user with `NO_REPLY` directive
3. **LLM Prompted** - Model is reminded to write durable notes to disk
4. **Then Compaction** - After flush, normal compaction summarizes history

```json
// Configuration example
{
  "compaction": {
    "memoryFlush": {
      "enabled": true,
      "softThresholdTokens": 4000,
      "systemPrompt": "Write any important context to memory before compaction...",
      "userPrompt": "NO_REPLY"
    }
  }
}
```

#### Session Memory Hook
The `session-memory` bundled hook auto-saves when `/new` command is issued:
- Extracts last 15 lines of conversation
- Uses LLM to generate descriptive filename slug
- Saves session metadata to dated memory file

### Memory Structure Format

#### Daily Log (Append-Only)
```markdown
# 2026-01-26

## 10:30 AM - CodexMonitor work
Implemented MCP server for memory integration.
Tags: #project #codexmonitor #mcp

## 2:15 PM - Delivery shift
Took 5 orders, earned $87.
Tags: #delivery #earnings
```

#### Retain/Recall/Reflect Loop (v2 Architecture)
```
RETAIN → Normalize daily logs into narrative facts
         - Tagged with type: World, Experience, Opinion
         - Entity mentions extracted

RECALL → Query the derived index
         - Lexical search (FTS5)
         - Entity search
         - Temporal search
         - Opinion search

REFLECT → Automatic consolidation
         - Periodic summarization
         - Pattern detection
```

### Vector Search Implementation

| Component | Technology |
|-----------|------------|
| Storage | SQLite with FTS5 (lexical) |
| Vector Extension | `sqlite-vec` (accelerated) |
| Fallback | In-process cosine similarity |
| Embeddings | Local Gemma model (default) |
| Alternative | OpenAI, Gemini APIs |

#### Hybrid Search
Combines vector similarity with BM25 keyword relevance for better recall.

```javascript
// Conceptual search flow
const results = await hybridSearch({
  query: "CodexMonitor memory integration",
  vectorWeight: 0.7,
  bm25Weight: 0.3,
  limit: 10
});
```

### Memory Configuration
```json
{
  "memory": {
    "enabled": true,
    "provider": "local",           // local | openai | gemini
    "model": "gemma-2b-it",        // for local embeddings
    "watchFiles": true,            // auto-index on change
    "hybridSearch": true
  }
}
```

---

## 2. Browser Integration

### Architecture Overview
```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Chrome Extension│────▶│ Gateway Daemon  │────▶│ Agent Runtime   │
│ (Control UI)    │ WS  │ (clawdbot)      │     │ (LLM + Skills)  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

### Chrome Extension
- **Purpose**: Control UI for interacting with ClawdBot from browser
- **Communication**: WebSocket to local gateway daemon
- **Features**:
  - Send messages to agent
  - View conversation history
  - Trigger skills
  - Access settings

### Browser Automation (via Skills)
ClawdBot uses **MCP-based browser automation** rather than built-in browser control:

#### DevTools MCP Skill
```toml
# SKILL.toml
---
name: chrome-devtools-mcp
description: Control Chrome DevTools via MCP
---
```

Capabilities:
- Navigate to URLs
- Click elements
- Fill forms
- Take screenshots
- Extract data
- Execute JavaScript

#### Browserbase/Stagehand Integration
For complex automation:
```toml
---
name: browser-automation
description: Automate web browser interactions using Stagehand
---
Uses persistent Chrome instance for multi-step workflows:
- Login to sites
- Fill complex forms
- Visual verification via screenshots
```

### Browser CLI Command
```bash
clawdbot browser          # Open control UI in default browser
clawdbot browser --url    # Print URL instead of opening
```

---

## 3. Skill System

### Skill Definition Format

#### SKILL.toml Structure
```toml
---
name: skill-name
description: What this skill does
homepage: https://example.com          # Optional
user-invocable: true                   # Enable /skill-name slash command
disable-model-invocation: false        # Can model auto-invoke?
command-dispatch: tool-name            # Link to tool execution
command-tool: bash                     # Execution method
command-arg-mode: single               # Argument handling
metadata: {"bins": ["ffmpeg"], "env": ["API_KEY"], "os": ["darwin", "linux"]}
---

# Skill Instructions (Markdown)

This skill helps you do X by...

## Usage
/skill-name <arguments>

## Examples
- /skill-name "create thumbnail"
```

### Skill Loading Locations
```
1. Bundled     → Shipped with ClawdBot install
2. Managed     → ~/.clawdbot/skills/
3. Workspace   → <workspace>/skills/
```

Priority: Workspace > Managed > Bundled (later overrides earlier)

### Skill Gating (Requirements)
```toml
metadata: {
  "bins": ["ffmpeg", "imagemagick"],   # Required binaries
  "env": ["OPENAI_API_KEY"],           # Required env vars
  "os": ["darwin", "linux"],           # Supported OS
  "install": {                          # Auto-install spec
    "brew": "ffmpeg",
    "apt": "ffmpeg"
  }
}
```

### Skill Discovery & Registry

#### ClawdHub (Public Registry)
- Browse skills at https://clawdhub.com
- Install: `clawdbot skills install <skill-name>`
- Skills are just folders with SKILL.toml + supporting files

#### CLI Commands
```bash
clawdbot skills              # List installed skills
clawdbot skills install X    # Install from ClawdHub
clawdbot skills uninstall X  # Remove skill
clawdbot skills search X     # Search registry
```

### Skill-Memory Integration
Skills can read/write to agent memory:
```markdown
# In skill instructions
After completing the task, write a summary to memory:
- Use `memory/YYYY-MM-DD.md` for daily logs
- Include relevant tags for searchability
```

### Example Complex Skill

#### Image Generation Skill
```toml
---
name: nano-banana-pro
description: Generate or edit images via Gemini 3 Pro Image
user-invocable: true
metadata: {"env": ["GOOGLE_API_KEY"]}
---

# Nano Banana Pro

Generate images using Google's Gemini 3 Pro Image model.

## Commands
/nano-banana-pro generate "prompt"
/nano-banana-pro edit <image-path> "edit instructions"

## How it works
1. Receives user prompt
2. Calls Gemini 3 Pro Image API
3. Saves result to workspace
4. Returns path to generated image
```

---

## 4. Technical Architecture

### Tech Stack
| Component | Technology |
|-----------|------------|
| Core Runtime | TypeScript/Node.js (Bun) |
| Gateway Daemon | Node.js service |
| Database | SQLite + FTS5 + sqlite-vec |
| Embeddings | Local (Gemma) or Cloud (OpenAI/Gemini) |
| Config Format | JSON5 |
| IPC | Unix sockets, WebSocket |

### LLM Integration
```json
{
  "agents": {
    "defaults": {
      "model": {
        "primary": "anthropic/claude-opus-4-5",
        "fallbacks": ["anthropic/claude-sonnet-4-5"]
      }
    }
  }
}
```

Supported providers:
- **Anthropic** (API key or Claude Code CLI OAuth)
- **OpenAI** (API key or Codex subscription)
- **Google Gemini**
- **Local models** (Ollama, vLLM via OpenAI-compatible endpoint)

### Configuration Format
```json5
// ~/.clawdbot/config.json5
{
  // Environment variables
  env: {
    ANTHROPIC_API_KEY: "sk-...",
  },

  // Agent configuration
  agents: {
    defaults: {
      model: { primary: "anthropic/claude-opus-4-5" },
      workspace: "~/clawd",
    }
  },

  // Memory configuration
  memory: {
    enabled: true,
    provider: "local",
  },

  // Channel configuration (Telegram, Discord, etc.)
  channels: { ... },

  // Skills configuration
  skills: {
    disabled: ["skill-to-disable"],
  }
}
```

---

## 5. Key Features to Steal for CodexMonitor

### Priority 1: Automatic Memory (Pre-Compaction Flush)
**Why**: Prevents context loss during long sessions
**Implementation**:
1. Monitor token count approaching limit
2. Trigger silent memory save turn
3. LLM writes important context to Supabase
4. Then proceed with compaction

### Priority 2: Markdown Memory Format
**Why**: Human-readable, searchable, versionable
**Implementation**:
- Store daily logs as markdown in Obsidian vault
- Parse and index into Supabase for search
- Keep markdown as source of truth

### Priority 3: Skill System with SKILL.toml
**Why**: Extensible, shareable, discoverable
**Implementation**:
```
/Volumes/YouTube 4TB/CodexMonitor/skills/
├── order-evaluation/
│   ├── SKILL.toml
│   └── instructions.md
├── life-logging/
│   ├── SKILL.toml
│   └── instructions.md
└── ...
```

### Priority 4: Browser Control via MCP
**Why**: Already have claude-in-chrome MCP
**Implementation**:
- Create skill that wraps browser MCP tools
- Enable web automation from iOS/desktop

---

## 6. Comparison: CodexMonitor vs ClawdBot

| Feature | ClawdBot | CodexMonitor (Current) | Gap |
|---------|----------|------------------------|-----|
| Memory Storage | SQLite + Local files | Supabase | Similar |
| Auto Memory | Pre-compaction flush | Manual only | **Need** |
| Embeddings | Local Gemma | MiniMax API | Similar |
| Browser | MCP skills | claude-in-chrome MCP | **Have** |
| Skills | SKILL.toml + ClawdHub | Claude Code skills | Similar |
| Mobile | Telegram/WhatsApp | Native iOS app | **Better** |

---

## 7. Recommended Implementation Order

1. **Auto Memory Flush** (1-2 days)
   - Hook into Codex compaction events
   - Add daemon RPC for memory flush trigger
   - Configure soft threshold

2. **Daily Log Markdown Sync** (1 day)
   - Sync Supabase entries to Obsidian markdown
   - Keep dual storage (Supabase + MD)

3. **SKILL.toml Format** (2-3 days)
   - Define CodexMonitor skill format
   - Create skill loader in daemon
   - Port existing skills to new format

4. **Skill Registry** (Future)
   - Public skill sharing
   - Install from URL/registry

---

*Document generated from ClawdBot docs analysis - 2026-01-26*
*For use with ChatGPT 5.2 Pro spec generation*
