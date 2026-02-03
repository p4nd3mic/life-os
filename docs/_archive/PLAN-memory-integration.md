# CodexMonitor Memory Integration Plan (Supabase Edition)

**Created:** 2026-01-25
**Updated:** 2026-01-25
**Status:** Ready for Implementation
**Approach:** Reuse existing Supabase + pgvector + MiniMax embeddings

---

## ⚠️ CRITICAL: Implementation Rules

> **READ THIS BEFORE STARTING ANY PHASE**

1. **PHASES ARE MANDATORY** - Complete Phase A entirely before starting Phase B
2. **REUSE EXISTING CODE** - Port from `_archive/life-mcp/`, don't rebuild
3. **BUILD AFTER EACH SECTION** - Run `cargo build` and `npm run build` after each section
4. **TEST BEFORE COMMIT** - Each phase gets its own commit after verification
5. **LOG EVERYTHING** - Update `docs/assistant-log-YYYY-MM-DD.md`

---

## Overview

### What We're Building

Memory system for CodexMonitor that reuses the existing Life OS infrastructure:

- **Supabase PostgreSQL** with pgvector (already enabled)
- **MiniMax `embo-01` embeddings** (already working)
- **Semantic search** via existing RPC functions
- **New `memory` table** for CodexMonitor-specific entries
- **iOS Memory tab** for search/browse/append

### Why Supabase Instead of SQLite

| Aspect | SQLite (Original Plan) | Supabase (This Plan) |
|--------|------------------------|----------------------|
| **Embeddings** | V2 (not implemented) | ✅ Already working |
| **Semantic search** | BM25 only | ✅ pgvector cosine |
| **Cloud sync** | Manual | ✅ Built-in |
| **iOS access** | Via daemon only | ✅ Direct API |
| **Effort** | ~1600 lines new | ~400 lines (mostly wiring) |

### Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Supabase (Existing)                         │
│  ├── notes table (life-mcp)                                         │
│  ├── memory table (NEW for CodexMonitor)                            │
│  ├── pgvector extension ✅                                          │
│  └── search_memory_by_embedding() (NEW RPC)                         │
└─────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│              Daemon (Rust) - Uses Supabase REST API                 │
│  ├── MemoryService (calls Supabase, not SQLite)                     │
│  ├── EmbeddingsClient (calls MiniMax API)                           │
│  └── JSON-RPC endpoints (memory_*)                                  │
└─────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│   iOS App     │     │  Desktop App  │     │    Codex      │
│  Memory Tab   │     │  Memory Tab   │     │  MCP Tools    │
└───────────────┘     └───────────────┘     └───────────────┘
```

---

## Existing Code Locations (REFERENCE THESE)

### Life-MCP (Archived but Working)

| File | Purpose | Port To |
|------|---------|---------|
| `_archive/life-mcp/src/clients/minimax-embeddings.js` | Embeddings client | Rust equivalent |
| `_archive/life-mcp/src/supabase/client.js` | Supabase client | Rust reqwest |
| `_archive/life-mcp/src/supabase/note-embeddings.js` | Embed pipeline | Rust async |
| `_archive/life-mcp/src/tools/knowledge.js` | MCP tools | Reference for tool design |
| `_archive/life-mcp/migrations/notes_table.sql` | Schema reference | Copy pattern for memory |
| `_archive/life-mcp/migrations/knowledge_vector.sql` | Vector schema | Copy for memory |

### Life-Chat (Active)

| File | Purpose | Reference For |
|------|---------|---------------|
| `life-chat/LifeChat/Sources/Services/SupabaseService.swift` | Swift Supabase client | iOS memory calls |
| `life-chat/SUPABASE.md` | Setup docs | Connection details |

### Full Paths

```
/Volumes/YouTube 4TB/code/_archive/life-mcp/
/Volumes/YouTube 4TB/code/life-os/apps/life-chat/
```

---

## Phase A: Supabase Memory Table + RPC

**Goal:** Create `memory` table in Supabase with embedding support.

### A.1: Create Memory Table Migration

**File to create:** `migrations/memory_table.sql` (run in Supabase SQL Editor)

```sql
-- CodexMonitor Memory Table
-- Based on existing notes table pattern from life-mcp

-- Enable pgvector if not already (should already be enabled)
CREATE EXTENSION IF NOT EXISTS vector;

-- Memory table for CodexMonitor
CREATE TABLE IF NOT EXISTS memory (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  -- Content
  content TEXT NOT NULL,

  -- Classification
  memory_type TEXT NOT NULL DEFAULT 'daily',  -- 'daily' or 'curated'
  tags TEXT[] DEFAULT ARRAY[]::TEXT[],

  -- Source tracking
  source TEXT NOT NULL DEFAULT 'codexmonitor',  -- 'codexmonitor', 'ios', 'daemon'
  workspace_id TEXT,  -- Optional: link to CodexMonitor workspace

  -- Embedding columns (same pattern as notes table)
  embedding vector,
  embedding_model TEXT,
  embedding_dim INT,
  embedding_status TEXT DEFAULT 'pending',
  embedding_updated_at TIMESTAMPTZ,
  embedding_error TEXT,

  -- Timestamps
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),

  -- Constraints
  CONSTRAINT memory_type_check CHECK (memory_type IN ('daily', 'curated')),
  CONSTRAINT memory_embedding_status_check CHECK (embedding_status IN ('pending', 'ready', 'error'))
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_memory_created_at ON memory(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_type ON memory(memory_type);
CREATE INDEX IF NOT EXISTS idx_memory_tags ON memory USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_memory_workspace ON memory(workspace_id);
CREATE INDEX IF NOT EXISTS idx_memory_embedding_status ON memory(embedding_status);

-- Full-text search index (BM25 fallback)
CREATE INDEX IF NOT EXISTS idx_memory_content_fts ON memory USING GIN(to_tsvector('english', content));

-- RLS (allow all for now - single user)
ALTER TABLE memory ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Allow all for memory" ON memory FOR ALL USING (true);

-- Updated_at trigger
CREATE OR REPLACE FUNCTION update_memory_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER memory_updated_at_trigger
  BEFORE UPDATE ON memory
  FOR EACH ROW
  EXECUTE FUNCTION update_memory_updated_at();
```

### A.2: Create Semantic Search RPC

**File to create:** `migrations/memory_search_rpc.sql` (run in Supabase SQL Editor)

```sql
-- Semantic search for memory table
-- Based on existing search_notes_by_embedding from life-mcp

CREATE OR REPLACE FUNCTION search_memory_by_embedding(
  query_embedding vector,
  match_count int DEFAULT 10,
  max_distance float8 DEFAULT NULL,
  filter_type text DEFAULT NULL,
  filter_workspace text DEFAULT NULL
)
RETURNS TABLE (
  id uuid,
  content text,
  memory_type text,
  tags text[],
  workspace_id text,
  created_at timestamptz,
  distance float8,
  score float8
)
LANGUAGE sql
STABLE
AS $$
  SELECT
    m.id,
    m.content,
    m.memory_type,
    m.tags,
    m.workspace_id,
    m.created_at,
    (m.embedding <=> query_embedding) AS distance,
    (1 - (m.embedding <=> query_embedding)::float8) AS score
  FROM memory m
  WHERE m.embedding IS NOT NULL
    AND m.embedding_status = 'ready'
    AND (max_distance IS NULL OR (m.embedding <=> query_embedding) <= max_distance)
    AND (filter_type IS NULL OR m.memory_type = filter_type)
    AND (filter_workspace IS NULL OR m.workspace_id = filter_workspace)
  ORDER BY m.embedding <=> query_embedding
  LIMIT match_count;
$$;

-- Full-text search fallback (when no embedding available)
CREATE OR REPLACE FUNCTION search_memory_by_text(
  search_query text,
  match_count int DEFAULT 10,
  filter_type text DEFAULT NULL
)
RETURNS TABLE (
  id uuid,
  content text,
  memory_type text,
  tags text[],
  created_at timestamptz,
  rank float4
)
LANGUAGE sql
STABLE
AS $$
  SELECT
    m.id,
    m.content,
    m.memory_type,
    m.tags,
    m.created_at,
    ts_rank(to_tsvector('english', m.content), plainto_tsquery('english', search_query)) AS rank
  FROM memory m
  WHERE to_tsvector('english', m.content) @@ plainto_tsquery('english', search_query)
    AND (filter_type IS NULL OR m.memory_type = filter_type)
  ORDER BY rank DESC
  LIMIT match_count;
$$;

-- Bootstrap function (get recent memory for context)
CREATE OR REPLACE FUNCTION get_memory_bootstrap(
  curated_limit int DEFAULT 50,
  daily_limit int DEFAULT 20
)
RETURNS TABLE (
  id uuid,
  content text,
  memory_type text,
  tags text[],
  created_at timestamptz
)
LANGUAGE sql
STABLE
AS $$
  (
    SELECT id, content, memory_type, tags, created_at
    FROM memory
    WHERE memory_type = 'curated'
    ORDER BY created_at DESC
    LIMIT curated_limit
  )
  UNION ALL
  (
    SELECT id, content, memory_type, tags, created_at
    FROM memory
    WHERE memory_type = 'daily'
      AND created_at > now() - interval '7 days'
    ORDER BY created_at DESC
    LIMIT daily_limit
  );
$$;
```

### A.3: Test in Supabase

```sql
-- Test insert
INSERT INTO memory (content, memory_type, tags)
VALUES ('Test memory entry', 'daily', ARRAY['test']);

-- Test text search
SELECT * FROM search_memory_by_text('test', 10);

-- Test bootstrap
SELECT * FROM get_memory_bootstrap();

-- Cleanup
DELETE FROM memory WHERE content = 'Test memory entry';
```

### A.4: Phase A Commit

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor"

mkdir -p migrations

# Save migrations locally for reference
cat > migrations/001_memory_table.sql << 'EOF'
-- (paste memory table SQL here)
EOF

cat > migrations/002_memory_search_rpc.sql << 'EOF'
-- (paste search RPC SQL here)
EOF

# Log
echo "## Phase A: Supabase Memory Schema - $(date)" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created memory table with embedding columns" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created search_memory_by_embedding RPC" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created search_memory_by_text fallback RPC" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created get_memory_bootstrap RPC" >> docs/assistant-log-$(date +%Y-%m-%d).md

git add migrations/ docs/
git commit -m "feat(memory): Phase A - Supabase memory schema

- Add memory table with embedding columns (pgvector)
- Add search_memory_by_embedding RPC (semantic search)
- Add search_memory_by_text RPC (BM25 fallback)
- Add get_memory_bootstrap RPC (context loading)

Reuses existing Supabase + pgvector infrastructure from life-mcp"

git push origin main
```

---

## Phase B: Rust Memory Service (Supabase Client)

**Goal:** Add Rust code to call Supabase from the daemon.

### B.1: Add Dependencies

**File:** `src-tauri/Cargo.toml`

```toml
[dependencies]
# Already have reqwest, add these if missing:
reqwest = { version = "0.12", features = ["json"] }
```

### B.2: Add Supabase Config to AppSettings

**File:** `src-tauri/src/types.rs`

Add to `AppSettings`:

```rust
// Memory settings (Supabase-backed)
#[serde(default)]
pub memory_enabled: bool,

#[serde(default)]
pub supabase_url: String,

#[serde(default)]
pub supabase_anon_key: String,

#[serde(default)]
pub minimax_api_key: String,

#[serde(default = "default_memory_embedding_enabled")]
pub memory_embedding_enabled: bool,
```

Add defaults:

```rust
fn default_memory_embedding_enabled() -> bool {
    false  // Off by default, user must provide MINIMAX_API_KEY
}
```

**File:** `src/types.ts`

```typescript
// Memory settings
export interface AppSettings {
  // ... existing ...
  memory_enabled?: boolean;
  supabase_url?: string;
  supabase_anon_key?: string;
  minimax_api_key?: string;
  memory_embedding_enabled?: boolean;
}
```

### B.3: Create Memory Module

**File:** `src-tauri/src/memory/mod.rs`

```rust
pub mod supabase;
pub mod embeddings;
pub mod service;

pub use service::MemoryService;
```

**File:** `src-tauri/src/memory/supabase.rs`

```rust
//! Supabase client for memory operations
//! Reference: /Volumes/YouTube 4TB/code/_archive/life-mcp/src/supabase/client.js

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct SupabaseClient {
    client: Client,
    url: String,
    anon_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: Option<String>,
    pub content: String,
    pub memory_type: String,
    pub tags: Vec<String>,
    pub workspace_id: Option<String>,
    pub embedding_status: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub id: String,
    pub content: String,
    pub memory_type: String,
    pub tags: Vec<String>,
    pub workspace_id: Option<String>,
    pub created_at: String,
    pub distance: Option<f64>,
    pub score: Option<f64>,
    pub rank: Option<f32>,
}

impl SupabaseClient {
    pub fn new(url: &str, anon_key: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.trim_end_matches('/').to_string(),
            anon_key: anon_key.to_string(),
        }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("apikey", self.anon_key.parse().unwrap());
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.anon_key).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }

    /// Insert a new memory entry
    pub async fn insert_memory(&self, entry: &MemoryEntry) -> Result<MemoryEntry, String> {
        let url = format!("{}/rest/v1/memory", self.url);

        let body = json!({
            "content": entry.content,
            "memory_type": entry.memory_type,
            "tags": entry.tags,
            "workspace_id": entry.workspace_id,
            "embedding_status": "pending"
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .header("Prefer", "return=representation")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase insert failed: {}", text));
        }

        let entries: Vec<MemoryEntry> = resp.json().await.map_err(|e| e.to_string())?;
        entries.into_iter().next().ok_or("No entry returned".to_string())
    }

    /// Update memory with embedding
    pub async fn update_memory_embedding(
        &self,
        id: &str,
        embedding: &[f32],
        model: &str,
        dim: usize,
    ) -> Result<(), String> {
        let url = format!("{}/rest/v1/memory?id=eq.{}", self.url, id);

        // Format embedding as pgvector literal
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let body = json!({
            "embedding": embedding_str,
            "embedding_model": model,
            "embedding_dim": dim,
            "embedding_status": "ready",
            "embedding_updated_at": chrono::Utc::now().to_rfc3339()
        });

        let resp = self
            .client
            .patch(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase update failed: {}", text));
        }

        Ok(())
    }

    /// Search memory by embedding (semantic search)
    pub async fn search_by_embedding(
        &self,
        embedding: &[f32],
        limit: usize,
        max_distance: Option<f64>,
    ) -> Result<Vec<MemorySearchResult>, String> {
        let url = format!("{}/rest/v1/rpc/search_memory_by_embedding", self.url);

        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let body = json!({
            "query_embedding": embedding_str,
            "match_count": limit,
            "max_distance": max_distance
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase search failed: {}", text));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    /// Search memory by text (BM25 fallback)
    pub async fn search_by_text(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>, String> {
        let url = format!("{}/rest/v1/rpc/search_memory_by_text", self.url);

        let body = json!({
            "search_query": query,
            "match_count": limit
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase text search failed: {}", text));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    /// Get memory bootstrap (recent curated + daily)
    pub async fn get_bootstrap(&self) -> Result<Vec<MemorySearchResult>, String> {
        let url = format!("{}/rest/v1/rpc/get_memory_bootstrap", self.url);

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase bootstrap failed: {}", text));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    /// Get memory status (counts by status)
    pub async fn get_status(&self) -> Result<Value, String> {
        // Count total, pending, ready, error
        let url = format!(
            "{}/rest/v1/memory?select=embedding_status",
            self.url
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase status failed: {}", text));
        }

        let entries: Vec<Value> = resp.json().await.map_err(|e| e.to_string())?;

        let mut pending = 0;
        let mut ready = 0;
        let mut error = 0;

        for entry in &entries {
            match entry.get("embedding_status").and_then(|v| v.as_str()) {
                Some("pending") => pending += 1,
                Some("ready") => ready += 1,
                Some("error") => error += 1,
                _ => {}
            }
        }

        Ok(json!({
            "total": entries.len(),
            "pending": pending,
            "ready": ready,
            "error": error,
            "enabled": true
        }))
    }
}
```

**File:** `src-tauri/src/memory/embeddings.rs`

```rust
//! MiniMax embeddings client
//! Reference: /Volumes/YouTube 4TB/code/_archive/life-mcp/src/clients/minimax-embeddings.js

use reqwest::Client;
use serde::{Deserialize, Serialize};

const MINIMAX_API_URL: &str = "https://api.minimax.chat/v1/embeddings";
const DEFAULT_MODEL: &str = "embo-01";

#[derive(Clone)]
pub struct EmbeddingsClient {
    client: Client,
    api_key: String,
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    texts: Vec<String>,
    #[serde(rename = "type")]
    embed_type: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    vectors: Vec<Vec<f32>>,
    model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    pub vector: Vec<f32>,
    pub model: String,
    pub dim: usize,
}

impl EmbeddingsClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    pub async fn generate(
        &self,
        text: &str,
        embed_type: &str, // "db" or "query"
    ) -> Result<EmbeddingResult, String> {
        if self.api_key.is_empty() {
            return Err("MINIMAX_API_KEY not set".to_string());
        }

        // Truncate to ~8000 chars like the JS client
        let truncated = if text.len() > 8000 {
            &text[..8000]
        } else {
            text
        };

        let request = EmbeddingRequest {
            model: DEFAULT_MODEL.to_string(),
            texts: vec![truncated.to_string()],
            embed_type: embed_type.to_string(),
        };

        let resp = self
            .client
            .post(MINIMAX_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("MiniMax API error: {}", text));
        }

        let response: EmbeddingResponse = resp.json().await.map_err(|e| e.to_string())?;

        let vector = response
            .vectors
            .into_iter()
            .next()
            .ok_or("No vector returned")?;

        let dim = vector.len();

        Ok(EmbeddingResult {
            vector,
            model: response.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            dim,
        })
    }
}
```

**File:** `src-tauri/src/memory/service.rs`

```rust
//! Memory service combining Supabase + MiniMax
//! Reference: /Volumes/YouTube 4TB/code/_archive/life-mcp/src/supabase/note-embeddings.js

use super::embeddings::EmbeddingsClient;
use super::supabase::{MemoryEntry, MemorySearchResult, SupabaseClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub struct MemoryService {
    supabase: SupabaseClient,
    embeddings: Option<EmbeddingsClient>,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub enabled: bool,
    pub embeddings_enabled: bool,
    pub total: usize,
    pub pending: usize,
    pub ready: usize,
    pub error: usize,
}

impl MemoryService {
    pub fn new(
        supabase_url: &str,
        supabase_anon_key: &str,
        minimax_api_key: Option<&str>,
        enabled: bool,
    ) -> Self {
        let embeddings = minimax_api_key
            .filter(|k| !k.is_empty())
            .map(EmbeddingsClient::new);

        Self {
            supabase: SupabaseClient::new(supabase_url, supabase_anon_key),
            embeddings,
            enabled,
        }
    }

    pub async fn status(&self) -> Result<MemoryStatus, String> {
        if !self.enabled {
            return Ok(MemoryStatus {
                enabled: false,
                embeddings_enabled: false,
                total: 0,
                pending: 0,
                ready: 0,
                error: 0,
            });
        }

        let status = self.supabase.get_status().await?;

        Ok(MemoryStatus {
            enabled: true,
            embeddings_enabled: self.embeddings.is_some(),
            total: status.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            pending: status.get("pending").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            ready: status.get("ready").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            error: status.get("error").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        })
    }

    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>, String> {
        if !self.enabled {
            return Err("Memory not enabled".to_string());
        }

        // If embeddings available, use semantic search
        if let Some(ref embeddings) = self.embeddings {
            let result = embeddings.generate(query, "query").await?;
            self.supabase
                .search_by_embedding(&result.vector, limit, Some(0.5))
                .await
        } else {
            // Fall back to text search
            self.supabase.search_by_text(query, limit).await
        }
    }

    pub async fn append(
        &self,
        memory_type: &str,
        content: &str,
        tags: Vec<String>,
        workspace_id: Option<String>,
    ) -> Result<MemoryEntry, String> {
        if !self.enabled {
            return Err("Memory not enabled".to_string());
        }

        let entry = MemoryEntry {
            id: None,
            content: content.to_string(),
            memory_type: memory_type.to_string(),
            tags,
            workspace_id,
            embedding_status: Some("pending".to_string()),
            created_at: None,
        };

        let inserted = self.supabase.insert_memory(&entry).await?;

        // Queue embedding generation (fire and forget)
        if let (Some(ref embeddings), Some(ref id)) = (&self.embeddings, &inserted.id) {
            let embeddings = embeddings.clone();
            let supabase = self.supabase.clone();
            let id = id.clone();
            let content = content.to_string();

            tokio::spawn(async move {
                match embeddings.generate(&content, "db").await {
                    Ok(result) => {
                        if let Err(e) = supabase
                            .update_memory_embedding(&id, &result.vector, &result.model, result.dim)
                            .await
                        {
                            eprintln!("Failed to update embedding: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to generate embedding: {}", e);
                    }
                }
            });
        }

        Ok(inserted)
    }

    pub async fn bootstrap(&self) -> Result<Vec<MemorySearchResult>, String> {
        if !self.enabled {
            return Err("Memory not enabled".to_string());
        }

        self.supabase.get_bootstrap().await
    }
}
```

### B.4: Wire to Daemon

**File:** `src-tauri/src/bin/codex_monitor_daemon.rs`

Add to `DaemonState`:

```rust
use codex_monitor::memory::MemoryService;

pub struct DaemonState {
    // ... existing ...
    memory: Option<MemoryService>,
}
```

Initialize:

```rust
let memory = if app_settings.memory_enabled
    && !app_settings.supabase_url.is_empty()
    && !app_settings.supabase_anon_key.is_empty()
{
    Some(MemoryService::new(
        &app_settings.supabase_url,
        &app_settings.supabase_anon_key,
        if app_settings.memory_embedding_enabled {
            Some(&app_settings.minimax_api_key)
        } else {
            None
        },
        true,
    ))
} else {
    None
};
```

Add RPC handlers:

```rust
"memory_status" => {
    match &state.memory {
        Some(mem) => mem.status().await.map(|s| serde_json::to_value(s).unwrap()),
        None => Err("Memory not enabled".to_string()),
    }
}

"memory_search" => {
    let query = params.get("query").and_then(|v| v.as_str()).ok_or("Missing query")?;
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    match &state.memory {
        Some(mem) => mem.search(query, limit).await.map(|r| serde_json::to_value(r).unwrap()),
        None => Err("Memory not enabled".to_string()),
    }
}

"memory_append" => {
    let memory_type = params.get("type").and_then(|v| v.as_str()).unwrap_or("daily");
    let content = params.get("content").and_then(|v| v.as_str()).ok_or("Missing content")?;
    let tags: Vec<String> = params.get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let workspace_id = params.get("workspace_id").and_then(|v| v.as_str()).map(String::from);

    match &state.memory {
        Some(mem) => mem.append(memory_type, content, tags, workspace_id)
            .await
            .map(|e| serde_json::to_value(e).unwrap()),
        None => Err("Memory not enabled".to_string()),
    }
}

"memory_bootstrap" => {
    match &state.memory {
        Some(mem) => mem.bootstrap().await.map(|r| serde_json::to_value(r).unwrap()),
        None => Err("Memory not enabled".to_string()),
    }
}
```

### B.5: Phase B Testing

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor"

# Build
cargo build --manifest-path src-tauri/Cargo.toml

# Test (manual - start daemon with memory config)
# Set in settings:
#   memory_enabled: true
#   supabase_url: https://your-project.supabase.co
#   supabase_anon_key: your-anon-key
#   minimax_api_key: your-minimax-key (optional)
#   memory_embedding_enabled: true (if minimax key provided)
```

### B.6: Phase B Commit

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor"

echo "## Phase B: Rust Memory Service - $(date)" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created SupabaseClient for memory operations" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created EmbeddingsClient for MiniMax API" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Created MemoryService combining both" >> docs/assistant-log-$(date +%Y-%m-%d).md
echo "- Added daemon RPC endpoints: memory_status, memory_search, memory_append, memory_bootstrap" >> docs/assistant-log-$(date +%Y-%m-%d).md

git add src-tauri/src/memory/
git add src-tauri/src/types.rs
git add src-tauri/src/bin/codex_monitor_daemon.rs
git add src/types.ts
git add docs/

git commit -m "feat(memory): Phase B - Rust Memory Service with Supabase

- Add SupabaseClient for memory table operations
- Add EmbeddingsClient for MiniMax embo-01 API
- Add MemoryService combining Supabase + embeddings
- Add daemon RPC: memory_status, memory_search, memory_append, memory_bootstrap
- Async embedding generation (fire-and-forget)
- Falls back to text search when embeddings unavailable

Ported from life-mcp infrastructure"

git push origin main
```

---

## Phase C: iOS Memory Tab

**Goal:** Add Memory UI to iOS app.

### C.1: Add Swift Types

**File:** `ios/Packages/CodexMonitorModels/Sources/CodexMonitorModels/Models.swift`

```swift
// MARK: - Memory Types

public struct MemoryStatus: Codable, Sendable {
    public let enabled: Bool
    public let embeddingsEnabled: Bool
    public let total: Int
    public let pending: Int
    public let ready: Int
    public let error: Int

    enum CodingKeys: String, CodingKey {
        case enabled
        case embeddingsEnabled = "embeddings_enabled"
        case total, pending, ready, error
    }
}

public struct MemorySearchResult: Codable, Sendable, Identifiable {
    public let id: String
    public let content: String
    public let memoryType: String
    public let tags: [String]
    public let workspaceId: String?
    public let createdAt: String
    public let distance: Double?
    public let score: Double?
    public let rank: Float?

    enum CodingKeys: String, CodingKey {
        case id, content, tags
        case memoryType = "memory_type"
        case workspaceId = "workspace_id"
        case createdAt = "created_at"
        case distance, score, rank
    }
}

public struct MemoryEntry: Codable, Sendable {
    public let id: String?
    public let content: String
    public let memoryType: String
    public let tags: [String]
    public let workspaceId: String?
    public let embeddingStatus: String?
    public let createdAt: String?

    enum CodingKeys: String, CodingKey {
        case id, content, tags
        case memoryType = "memory_type"
        case workspaceId = "workspace_id"
        case embeddingStatus = "embedding_status"
        case createdAt = "created_at"
    }
}

public enum MemoryType: String, Codable, Sendable, CaseIterable {
    case daily
    case curated
}
```

### C.2: Add Swift RPC Methods

**File:** `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorRPC/CodexMonitorAPI.swift`

```swift
// MARK: - Memory Methods

extension CodexMonitorAPI {
    public func memoryStatus() async throws -> MemoryStatus {
        try await call(method: "memory_status", params: [:])
    }

    public func memorySearch(query: String, limit: Int = 10) async throws -> [MemorySearchResult] {
        try await call(method: "memory_search", params: [
            "query": query,
            "limit": limit
        ])
    }

    public func memoryAppend(
        type: MemoryType,
        content: String,
        tags: [String] = [],
        workspaceId: String? = nil
    ) async throws -> MemoryEntry {
        var params: [String: Any] = [
            "type": type.rawValue,
            "content": content,
            "tags": tags
        ]
        if let workspaceId = workspaceId {
            params["workspace_id"] = workspaceId
        }
        return try await call(method: "memory_append", params: params)
    }

    public func memoryBootstrap() async throws -> [MemorySearchResult] {
        try await call(method: "memory_bootstrap", params: [:])
    }
}
```

### C.3: Create MemoryView

**File:** `ios/CodexMonitorMobile/CodexMonitorMobile/Views/MemoryView.swift`

(Same as original plan - see Phase C.3 in previous version)

### C.4: Add to Tab Bar

Add Memory tab to main navigation.

### C.5: Phase C Commit

```bash
cd "/Volumes/YouTube 4TB/CodexMonitor"

git add ios/
git commit -m "feat(memory): Phase C - iOS Memory Tab

- Add MemoryStatus, MemorySearchResult, MemoryEntry Swift types
- Add memoryStatus, memorySearch, memoryAppend, memoryBootstrap RPC methods
- Add MemoryView with search, results, and compose UI
- Add Memory tab to main navigation

Phase C of Supabase memory integration"

git push origin main
```

---

## Phase D: MCP Server for Codex

**Goal:** Create MCP server so Codex can use memory tools.

(Same as original plan Phase B, but update to call MemoryService which uses Supabase)

### D.1: Create MCP Server

**File:** `src-tauri/src/bin/codex_monitor_memory_mcp.rs`

Same structure as before, but import MemoryService and use its methods.

### D.2: Phase D Commit

```bash
git commit -m "feat(memory): Phase D - MCP Server for Codex

- Add codex_monitor_memory_mcp binary
- Expose memory_bootstrap, memory_search, memory_append tools
- Uses MemoryService (Supabase-backed)"

git push origin main
```

---

## Configuration Reference

### Daemon Settings

```json
{
  "memory_enabled": true,
  "supabase_url": "https://your-project.supabase.co",
  "supabase_anon_key": "eyJ...",
  "minimax_api_key": "your-key-here",
  "memory_embedding_enabled": true
}
```

### Environment Variables

```bash
# Or set via env instead of settings
SUPABASE_URL=https://your-project.supabase.co
SUPABASE_ANON_KEY=eyJ...
MINIMAX_API_KEY=your-key
```

---

## Post-Implementation Checklist

- [ ] Phase A: Memory table + RPCs created in Supabase
- [ ] Phase B: Rust MemoryService builds and connects
- [ ] Phase C: iOS Memory tab searches and displays
- [ ] Phase D: MCP server responds to Codex tool calls
- [ ] Documentation updated
- [ ] All phases committed and pushed

---

## Comparison: SQLite vs Supabase Approach

| Aspect | SQLite (Original) | Supabase (This Plan) |
|--------|-------------------|----------------------|
| **Lines of code** | ~1600 | ~600 |
| **Embeddings** | V2 only | ✅ Day 1 |
| **Search quality** | BM25 only | BM25 + semantic |
| **Offline** | ✅ Yes | ❌ Needs internet |
| **iOS direct access** | Via daemon | ✅ Can call Supabase directly |
| **Cloud sync** | Manual | ✅ Automatic |
| **Reuses existing** | Nothing | ✅ life-mcp infrastructure |
