# Domain Architecture Research

> **Purpose:** Synthesis of domain-driven architecture patterns from life-chat/life-os for potential integration into CodexMonitor.
> **Created:** 2026-01-26

---

## Table of Contents

1. [Life-OS Domain Architecture](#1-life-os-domain-architecture)
2. [Current CodexMonitor Architecture](#2-current-codexmonitor-architecture)
3. [Gap Analysis](#3-gap-analysis)
4. [Integration Points](#4-integration-points)
5. [Recommended Implementation Path](#5-recommended-implementation-path)

---

## 1. Life-OS Domain Architecture

The life-os/life-chat project implements a sophisticated domain-driven architecture that isolates conversation contexts and injects domain-specific knowledge at thread creation time.

### 1.1 Domain Definition

Seven domains are defined via the `LifeOSSection` enum:

| Domain | Display Name | Icon | Page Key |
|--------|--------------|------|----------|
| `chat` | Chat | bubble.left.and.bubble.right | life |
| `codex` | Codex | terminal | life |
| `stream` | Stream | waveform | life |
| `delivery` | Delivery | car | delivery |
| `media` | Media | tv | life |
| `finance` | Finance | dollarsign.circle | finance |
| `youtube` | YouTube | play.rectangle | youtube |

Each domain has:
- `displayName` - Human-readable label
- `icon` - SF Symbol name
- `pageKey` - Maps to a dedicated thread context

### 1.2 PageKey System (Thread Isolation)

The `PageKey` enum provides thread isolation per domain:

```swift
enum PageKey: String, CaseIterable {
    case life
    case finance
    case delivery
    case youtube
    case hub
}
```

**Key behaviors:**
- Each `PageKey` maps to a dedicated Codex thread ID
- Thread IDs are persisted via `PageThreadLinkStore` (UserDefaults)
- Multiple domains can share a `PageKey` (e.g., chat, codex, stream all use `life`)
- Threads are cached in `CodexChatStore.threadCache[threadId]`

### 1.3 Context Injection Pattern

The `PageContextProvider` protocol enables domain-specific context injection:

```swift
protocol PageContextProvider {
    var pageKey: PageKey { get }
    func buildContext() async -> String
}
```

**Critical insight:** Context is injected at **thread creation**, not per-message.

Each domain has a specific context provider:
- `LifePageContext` - General assistant context
- `FinancePageContext` - Bills, budget data
- `DeliveryPageContext` - Active session, earnings data
- `YouTubePageContext` - Video pipeline, ideas list

**Context block structure (~2KB per domain):**
```markdown
# Domain Context

## Data Snapshot
[Current state of relevant data]

## Data Sources
[File paths, API endpoints]

## Available Actions
[What Claude can do in this domain]

## Guardrails
[What Claude should NOT do]
```

### 1.4 Per-Page Chat UI (PageChatPanel)

The `PageChatPanel` component provides a reusable chat overlay:

| Device | Presentation |
|--------|--------------|
| iPhone | Floating button → bottom sheet |
| iPad/Mac | Sidebar → always visible |

Thread state is managed by `CodexChatStore` with caching via `ThreadCacheEntry`.

### 1.5 Custom Views Per Domain

Beyond chat, each domain has specialized data views:

| Domain | View | Content |
|--------|------|---------|
| Stream | `StreamView` | Cards with categories (not chat bubbles) |
| Delivery | `DeliveryHubView` | Table of sessions, grouped by date |
| Finance | `FinanceHubView` | Bills list with due dates |
| YouTube | `YouTubeView` | Grid/list of video ideas |

### 1.6 Key Files in life-chat

| File | Purpose |
|------|---------|
| `LifeOSSection.swift` | Domain enum definition |
| `PageContextProvider.swift` | Context injection protocol |
| `PageChatPanel.swift` | Reusable chat overlay |
| `CodexChatStore.swift` | Thread state management |
| `PageThreadLinkStore.swift` | Thread ID persistence |
| `MainContainerView.swift` | Tab bar assembly |

---

## 2. Current CodexMonitor Architecture

CodexMonitor has a workspace-centric architecture that can serve as a foundation for domain support.

### 2.1 Workspace Model (Fully Implemented)

```typescript
interface WorkspaceInfo {
    id: string;
    name: string;
    path: string;
    connected: boolean;
    kind: 'main' | 'worktree';
    settings: WorkspaceSettings;
}

interface WorkspaceGroup {
    id: string;
    name: string;
    sortOrder: number;
    copiesFolder?: string;
}
```

**Key insight:** `WorkspaceGroup` is conceptually similar to a domain - it groups related workspaces.

### 2.2 Thread Model

Current thread handling:

| Aspect | Implementation |
|--------|----------------|
| Scope | Per-workspace |
| Data | `ThreadSummary`: id, name, updatedAt |
| Initialization | Passes only: cwd, approvalPolicy |
| System prompt | **NOT SUPPORTED** |

### 2.3 Prompts System

The existing prompts system is workspace-scoped:

- Storage: `$CODEX_HOME/prompts/`
- Scope: Global + per-workspace
- Selection: Manual in composer
- Auto-apply: **NOT SUPPORTED** at thread start

### 2.4 Settings Persistence

```typescript
interface AppSettings {
    workspaceGroups: WorkspaceGroup[];
    // ...other settings
}

interface WorkspaceSettings {
    // Per-workspace configuration
}
```

Storage: `settings.json` in app data directory.

### 2.5 Key Files in CodexMonitor

| File | Purpose |
|------|---------|
| `src/types.ts` | TypeScript type definitions |
| `src-tauri/src/types.rs` | Rust type definitions |
| `src-tauri/src/bin/codex_monitor_daemon.rs` | Daemon with `start_thread` |
| `src/features/workspaces/` | Workspace management |
| `src/features/prompts/` | Prompt library UI |
| `src/features/settings/` | Settings panels |

---

## 3. Gap Analysis

### 3.1 Feature Comparison

| Feature | Life-Chat | CodexMonitor | Gap |
|---------|-----------|--------------|-----|
| Domain definition | `LifeOSSection` enum | `WorkspaceGroup` | Need domain-specific fields |
| Thread isolation | Per-page threads | Per-workspace threads | Already implemented, different scope |
| Context injection | At thread creation | None | **Critical gap** - need `systemPrompt` |
| Custom views | Per-domain SwiftUI | Chat-only | Major UI effort |
| Thread caching | `ThreadCacheEntry` | Basic caching | Similar |
| Prompt library | N/A | Full implementation | CodexMonitor ahead |

### 3.2 Critical Gaps

1. **No context injection at thread start**
   - Current: Thread starts with only `cwd` and `approvalPolicy`
   - Needed: `systemPrompt` parameter for domain context

2. **No domain-to-workspace mapping**
   - Current: Workspaces are just paths
   - Needed: Domain assignment determines context

3. **No domain-specific UI**
   - Current: All workspaces use the same chat view
   - Needed: Domain-specific panels/overlays

### 3.3 Existing Advantages

CodexMonitor has features life-chat lacks:
- **Prompt library** - Reusable prompts with scoping
- **Multi-workspace sessions** - Connect to many Codex instances
- **Worktree support** - Git worktree integration
- **Desktop app** - Full Tauri desktop experience

---

## 4. Integration Points

### 4.1 Thread Initialization (Primary Integration)

**Current daemon code:**
```rust
async fn start_thread(&self, workspace_id: String) -> Result<Value, String> {
    let session = self.get_session(&workspace_id).await?;
    let params = json!({
        "cwd": session.entry.path,
        "approvalPolicy": "on-request"
    });
    session.send_request("thread/start", params).await
}
```

**Modified for domain support:**
```rust
async fn start_thread(&self, workspace_id: String) -> Result<Value, String> {
    let session = self.get_session(&workspace_id).await?;

    // NEW: Get domain context for this workspace
    let domain_prompt = self.get_domain_prompt(&workspace_id).await;

    let mut params = json!({
        "cwd": session.entry.path,
        "approvalPolicy": "on-request"
    });

    // NEW: Inject system prompt if domain has one
    if let Some(prompt) = domain_prompt {
        params["systemPrompt"] = json!(prompt);
    }

    session.send_request("thread/start", params).await
}
```

### 4.2 Domain Type Definition

**New Rust type:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub context_template: Option<String>,
    pub workspace_ids: Vec<String>,
}
```

**New TypeScript type:**
```typescript
interface Domain {
    id: string;
    name: string;
    icon: string;
    contextTemplate?: string;
    workspaceIds: string[];
}
```

### 4.3 Settings Extension

**Extended AppSettings:**
```typescript
interface AppSettings {
    workspaceGroups: WorkspaceGroup[];
    domains: Domain[];  // NEW
    domainAssignments: Record<string, string>;  // workspaceId -> domainId
}
```

### 4.4 RPC Methods Needed

| Method | Purpose |
|--------|---------|
| `domain/list` | Get all domains |
| `domain/create` | Create new domain |
| `domain/update` | Update domain settings |
| `domain/delete` | Remove domain |
| `domain/assign` | Assign workspace to domain |
| `domain/context` | Get domain context for thread |

### 4.5 iOS Changes

**CodexStore additions:**
```swift
@Published var domains: [Domain] = []
@Published var domainAssignments: [String: String] = [:]

func loadDomains() async { ... }
func createDomain(_ domain: Domain) async { ... }
func assignWorkspace(_ workspaceId: String, toDomain domainId: String) async { ... }
```

**UI changes:**
- Domain list in sidebar
- Domain assignment picker in workspace settings
- Domain badge in thread view header

---

## 5. Recommended Implementation Path

### Phase 1: Core Domain Support

1. **Add Domain type** to Rust and TypeScript
2. **Extend settings** with domains array
3. **Add domain CRUD** RPC methods
4. **Modify start_thread** to accept systemPrompt

### Phase 2: Context Injection

1. **Create context templates** per domain
2. **Implement context builder** in daemon
3. **Wire up** workspace → domain → context flow

### Phase 3: UI Integration

1. **Add domain management** UI in settings
2. **Add domain assignment** to workspace settings
3. **Show domain context** in thread creation

### Phase 4: iOS Support

1. **Add Domain model** to Swift
2. **Extend CodexStore** with domain state
3. **Add domain views** to iOS app

---

## Appendix: Reference Patterns

### A. Context Template Example

```markdown
# {{domain_name}} Context

## Your Role
You are assisting with {{domain_description}}.

## Data Sources
{{data_sources}}

## Available Actions
{{available_actions}}

## Constraints
{{constraints}}

## Current State
{{current_state}}
```

### B. Domain Assignment Flow

```
User creates workspace
    ↓
User assigns to domain (or auto-assign by path pattern)
    ↓
Domain context template loaded
    ↓
Template variables populated (data sources, state)
    ↓
Thread started with systemPrompt = populated context
    ↓
Codex has full domain awareness
```

### C. WorkspaceGroup → Domain Migration

If `WorkspaceGroup` is extended rather than creating `Domain`:

```typescript
interface WorkspaceGroup {
    id: string;
    name: string;
    sortOrder: number;
    copiesFolder?: string;
    // NEW domain fields:
    icon?: string;
    contextTemplate?: string;
    isDomain?: boolean;  // Flag for domain behavior
}
```

This preserves backward compatibility while adding domain features.

---

## Document History

| Date | Author | Changes |
|------|--------|---------|
| 2026-01-26 | Claude | Initial synthesis from life-os research |
