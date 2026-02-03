# Domain Support Implementation for CodexMonitor

## Background

This document provides three implementation options for adding **domain support** to CodexMonitor. The goal is to enable each workspace to have automatic context injection when threads start, similar to the life-chat `PageContextProvider` pattern.

**Core concept**: A domain defines a system prompt and default settings that are automatically applied when starting threads in associated workspaces.

---

## Current State

### Existing Architecture

Looking at the current codebase:

**WorkspaceSettings** (all platforms):
```typescript
// src/types.ts
type WorkspaceSettings = {
  sidebarCollapsed: boolean;
  sortOrder?: number | null;
  groupId?: string | null;
  gitRoot?: string | null;
};
```

**WorkspaceGroup** exists but is purely for UI organization:
```typescript
type WorkspaceGroup = {
  id: string;
  name: string;
  sortOrder?: number | null;
  copiesFolder?: string | null;
};
```

**Thread initialization** in daemon (`src-tauri/src/bin/codex_monitor_daemon.rs`):
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

The `thread/start` endpoint accepts a `systemPrompt` parameter that we're not currently using.

---

## Option A: Minimal - Domain Metadata Only

**Effort**: 2-3 days
**Complexity**: Low
**iOS Impact**: Minimal (model changes only)

### What It Does

- Add domain metadata fields to `WorkspaceSettings`
- Create domain prompt files in `~/.codex/prompts/domains/`
- Modify `start_thread` to inject domain prompt as `systemPrompt`
- No UI for domain management - manual file creation

### Type Changes

**TypeScript (`src/types.ts`):**
```typescript
export type WorkspaceSettings = {
  sidebarCollapsed: boolean;
  sortOrder?: number | null;
  groupId?: string | null;
  gitRoot?: string | null;
  // NEW: Domain support
  domain?: string | null;           // Domain identifier (e.g., "life", "coding")
  domainPromptPath?: string | null; // Override path to domain prompt file
  autoApplyDomainPrompt?: boolean;  // Whether to auto-inject on thread start
};
```

**Rust (`src-tauri/src/types.rs`):**
```rust
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct WorkspaceSettings {
    #[serde(default, rename = "sidebarCollapsed")]
    pub(crate) sidebar_collapsed: bool,
    #[serde(default, rename = "sortOrder")]
    pub(crate) sort_order: Option<u32>,
    #[serde(default, rename = "groupId")]
    pub(crate) group_id: Option<String>,
    #[serde(default, rename = "gitRoot")]
    pub(crate) git_root: Option<String>,
    // NEW
    #[serde(default)]
    pub(crate) domain: Option<String>,
    #[serde(default, rename = "domainPromptPath")]
    pub(crate) domain_prompt_path: Option<String>,
    #[serde(default, rename = "autoApplyDomainPrompt")]
    pub(crate) auto_apply_domain_prompt: Option<bool>,
}
```

**Swift (`ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift`):**
```swift
public struct WorkspaceSettings: Codable, Hashable, Sendable {
    public var sidebarCollapsed: Bool
    public var sortOrder: Int?
    public var groupId: String?
    public var gitRoot: String?
    // NEW
    public var domain: String?
    public var domainPromptPath: String?
    public var autoApplyDomainPrompt: Bool?

    public init(
        sidebarCollapsed: Bool = false,
        sortOrder: Int? = nil,
        groupId: String? = nil,
        gitRoot: String? = nil,
        domain: String? = nil,
        domainPromptPath: String? = nil,
        autoApplyDomainPrompt: Bool? = nil
    ) {
        self.sidebarCollapsed = sidebarCollapsed
        self.sortOrder = sortOrder
        self.groupId = groupId
        self.gitRoot = gitRoot
        self.domain = domain
        self.domainPromptPath = domainPromptPath
        self.autoApplyDomainPrompt = autoApplyDomainPrompt
    }

    enum CodingKeys: String, CodingKey {
        case sidebarCollapsed
        case sortOrder
        case groupId
        case gitRoot
        case domain
        case domainPromptPath
        case autoApplyDomainPrompt
    }
}
```

### Daemon Changes

**File**: `src-tauri/src/bin/codex_monitor_daemon.rs`

Modify `start_thread`:
```rust
async fn start_thread(&self, workspace_id: String) -> Result<Value, String> {
    let session = self.get_session(&workspace_id).await?;

    // Load domain prompt if configured
    let system_prompt = self.resolve_domain_prompt(&session.entry).await;

    let mut params = json!({
        "cwd": session.entry.path,
        "approvalPolicy": "on-request"
    });

    if let Some(prompt) = system_prompt {
        params["systemPrompt"] = json!(prompt);
    }

    session.send_request("thread/start", params).await
}

async fn resolve_domain_prompt(&self, entry: &WorkspaceEntry) -> Option<String> {
    // Check if auto-apply is enabled (default: true if domain is set)
    let auto_apply = entry.settings.auto_apply_domain_prompt.unwrap_or(true);
    if !auto_apply {
        return None;
    }

    // Try explicit path first
    if let Some(path) = &entry.settings.domain_prompt_path {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            return Some(content);
        }
    }

    // Try domain name lookup
    if let Some(domain) = &entry.settings.domain {
        let codex_home = std::env::var("CODEX_HOME")
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|h| h.join(".codex").to_string_lossy().into_owned())
                    .unwrap_or_else(|| "~/.codex".to_string())
            });

        let prompt_path = PathBuf::from(&codex_home)
            .join("prompts")
            .join("domains")
            .join(format!("{}.md", domain));

        if let Ok(content) = tokio::fs::read_to_string(&prompt_path).await {
            return Some(content);
        }
    }

    None
}
```

### File Structure

```
~/.codex/prompts/domains/
├── life.md           # Life OS domain prompt
├── coding.md         # General coding domain
├── food-delivery.md  # Delivery work domain
└── youtube.md        # Content creation domain
```

**Example domain prompt** (`~/.codex/prompts/domains/life.md`):
```markdown
# Life Domain Context

You are an assistant helping JMWillis with life management tasks.

## Key Context
- User is a 37-year-old food delivery driver in Harbor City, LA
- Primary goal: weight loss (235 lbs -> 180-185 lbs)
- Works delivery shifts 11am-2pm and 4:30-8:30pm
- Has genetic factors affecting metabolism (FTO, MTNR1B)

## Response Style
- Use emojis for visual scanning
- Keep responses mobile-friendly
- Respond warmly and conversationally
- Handle messy speech-to-text input gracefully

## Available Data
- Life stream in Obsidian vault: /Volumes/YouTube 4TB/Obsidian/Stream/
- Entity files: /Volumes/YouTube 4TB/Obsidian/Entities/
```

### Desktop UI Changes

Add domain field to workspace settings panel:

**File**: `src/features/workspaces/components/WorkspaceSettings.tsx`

Add a simple text input for domain name:
```tsx
<label>
  Domain
  <input
    type="text"
    value={settings.domain || ''}
    onChange={(e) => onUpdate({ domain: e.target.value || null })}
    placeholder="e.g., life, coding, youtube"
  />
</label>
```

### Pros
- Minimal code changes (~100 lines)
- Backward compatible (existing workspaces unaffected)
- Uses existing Codex infrastructure
- No UI complexity

### Cons
- No UI for domain management (prompts are files)
- Manual prompt file creation required
- No domain defaults for model/access mode
- No domain-scoped features (memory, settings)

---

## Option B: First-Class Domains (Recommended)

**Effort**: 5-7 days
**Complexity**: Medium
**iOS Impact**: Moderate (new views, CodexStore changes)

### What It Does

- Create `Domain` as a top-level entity in `AppSettings`
- Full domain CRUD UI in Settings
- Domain assignment dropdown in workspace settings
- Domain defaults: model, access mode, reasoning effort, approval policy
- iOS sync for domains

### Type Changes

**TypeScript (`src/types.ts`):**
```typescript
export type Domain = {
  id: string;
  name: string;
  description?: string;
  systemPrompt: string;
  // Defaults for threads in this domain
  defaultModel?: string | null;
  defaultAccessMode?: AccessMode | null;
  defaultReasoningEffort?: string | null;
  defaultApprovalPolicy?: 'on-request' | 'never' | null;
  // Visual
  color?: string | null;  // Hex color for sidebar badge
  icon?: string | null;   // Emoji or icon name
  // Ordering
  sortOrder?: number | null;
};

export type AppSettings = {
  // ... existing fields ...
  workspaceGroups: WorkspaceGroup[];
  // NEW
  domains: Domain[];
};

export type WorkspaceSettings = {
  sidebarCollapsed: boolean;
  sortOrder?: number | null;
  groupId?: string | null;
  gitRoot?: string | null;
  // NEW
  domainId?: string | null;
};
```

**Rust (`src-tauri/src/types.rs`):**
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Domain {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(rename = "systemPrompt")]
    pub(crate) system_prompt: String,
    #[serde(default, rename = "defaultModel")]
    pub(crate) default_model: Option<String>,
    #[serde(default, rename = "defaultAccessMode")]
    pub(crate) default_access_mode: Option<String>,
    #[serde(default, rename = "defaultReasoningEffort")]
    pub(crate) default_reasoning_effort: Option<String>,
    #[serde(default, rename = "defaultApprovalPolicy")]
    pub(crate) default_approval_policy: Option<String>,
    #[serde(default)]
    pub(crate) color: Option<String>,
    #[serde(default)]
    pub(crate) icon: Option<String>,
    #[serde(default, rename = "sortOrder")]
    pub(crate) sort_order: Option<u32>,
}

impl Default for Domain {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "New Domain".to_string(),
            description: None,
            system_prompt: String::new(),
            default_model: None,
            default_access_mode: None,
            default_reasoning_effort: None,
            default_approval_policy: None,
            color: None,
            icon: None,
            sort_order: None,
        }
    }
}

pub(crate) struct AppSettings {
    // ... existing fields ...
    #[serde(default = "default_workspace_groups", rename = "workspaceGroups")]
    pub(crate) workspace_groups: Vec<WorkspaceGroup>,
    // NEW
    #[serde(default, rename = "domains")]
    pub(crate) domains: Vec<Domain>,
}

pub(crate) struct WorkspaceSettings {
    // ... existing fields ...
    #[serde(default, rename = "domainId")]
    pub(crate) domain_id: Option<String>,
}
```

**Swift (`ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift`):**
```swift
public struct Domain: Codable, Hashable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var description: String?
    public var systemPrompt: String
    public var defaultModel: String?
    public var defaultAccessMode: String?
    public var defaultReasoningEffort: String?
    public var defaultApprovalPolicy: String?
    public var color: String?
    public var icon: String?
    public var sortOrder: Int?

    public init(
        id: String = UUID().uuidString,
        name: String,
        description: String? = nil,
        systemPrompt: String = "",
        defaultModel: String? = nil,
        defaultAccessMode: String? = nil,
        defaultReasoningEffort: String? = nil,
        defaultApprovalPolicy: String? = nil,
        color: String? = nil,
        icon: String? = nil,
        sortOrder: Int? = nil
    ) {
        self.id = id
        self.name = name
        self.description = description
        self.systemPrompt = systemPrompt
        self.defaultModel = defaultModel
        self.defaultAccessMode = defaultAccessMode
        self.defaultReasoningEffort = defaultReasoningEffort
        self.defaultApprovalPolicy = defaultApprovalPolicy
        self.color = color
        self.icon = icon
        self.sortOrder = sortOrder
    }
}

public struct AppSettings: Codable, Hashable, Sendable {
    // ... existing fields ...
    public var workspaceGroups: [WorkspaceGroup]
    // NEW
    public var domains: [Domain]
}

public struct WorkspaceSettings: Codable, Hashable, Sendable {
    // ... existing fields ...
    public var domainId: String?
}
```

### New RPC Methods

**Daemon additions** (`src-tauri/src/bin/codex_monitor_daemon.rs`):

```rust
// Domain CRUD operations
async fn domains_list(&self) -> Result<Vec<Domain>, String> {
    let settings = self.load_settings().await?;
    Ok(settings.domains)
}

async fn domains_create(&self, domain: Domain) -> Result<Domain, String> {
    let mut settings = self.load_settings().await?;
    let domain = Domain {
        id: uuid::Uuid::new_v4().to_string(),
        ..domain
    };
    settings.domains.push(domain.clone());
    self.save_settings(&settings).await?;
    Ok(domain)
}

async fn domains_update(&self, domain: Domain) -> Result<Domain, String> {
    let mut settings = self.load_settings().await?;
    if let Some(idx) = settings.domains.iter().position(|d| d.id == domain.id) {
        settings.domains[idx] = domain.clone();
        self.save_settings(&settings).await?;
        Ok(domain)
    } else {
        Err(format!("Domain not found: {}", domain.id))
    }
}

async fn domains_delete(&self, domain_id: String) -> Result<(), String> {
    let mut settings = self.load_settings().await?;
    settings.domains.retain(|d| d.id != domain_id);
    self.save_settings(&settings).await?;
    Ok(())
}

async fn domains_get(&self, domain_id: String) -> Result<Option<Domain>, String> {
    let settings = self.load_settings().await?;
    Ok(settings.domains.into_iter().find(|d| d.id == domain_id))
}
```

**RPC handler registration**:
```rust
"domains_list" => {
    let domains = state.domains_list().await?;
    serde_json::to_value(domains).map_err(|e| e.to_string())
}
"domains_create" => {
    let domain: Domain = serde_json::from_value(params.clone())
        .map_err(|e| format!("Invalid domain: {}", e))?;
    let created = state.domains_create(domain).await?;
    serde_json::to_value(created).map_err(|e| e.to_string())
}
"domains_update" => {
    let domain: Domain = serde_json::from_value(params.clone())
        .map_err(|e| format!("Invalid domain: {}", e))?;
    let updated = state.domains_update(domain).await?;
    serde_json::to_value(updated).map_err(|e| e.to_string())
}
"domains_delete" => {
    let domain_id = parse_string(&params, "domainId")?;
    state.domains_delete(domain_id).await?;
    Ok(json!({"ok": true}))
}
```

### Thread Initialization with Domain

**Modify `start_thread`**:
```rust
async fn start_thread(&self, workspace_id: String) -> Result<Value, String> {
    let session = self.get_session(&workspace_id).await?;
    let settings = self.load_settings().await?;

    // Resolve domain from workspace settings
    let domain = session.entry.settings.domain_id.as_ref()
        .and_then(|id| settings.domains.iter().find(|d| &d.id == id));

    let mut params = json!({
        "cwd": session.entry.path,
    });

    if let Some(d) = domain {
        // Apply domain system prompt
        if !d.system_prompt.is_empty() {
            params["systemPrompt"] = json!(d.system_prompt);
        }

        // Apply domain defaults
        if let Some(policy) = &d.default_approval_policy {
            params["approvalPolicy"] = json!(policy);
        } else {
            params["approvalPolicy"] = json!("on-request");
        }

        // Note: model and reasoning effort are applied at turn/start, not thread/start
    } else {
        params["approvalPolicy"] = json!("on-request");
    }

    session.send_request("thread/start", params).await
}
```

### Desktop UI Components

**New directory**: `src/features/domains/`

```
src/features/domains/
├── components/
│   ├── DomainList.tsx          # List view for settings
│   ├── DomainEditor.tsx        # Create/edit form
│   ├── DomainPicker.tsx        # Dropdown for workspace settings
│   └── DomainBadge.tsx         # Small badge showing domain
├── hooks/
│   └── useDomains.ts           # Domain state management
└── index.ts
```

**DomainList.tsx**:
```tsx
import { useState } from 'react';
import { useDomains } from '../hooks/useDomains';
import { DomainEditor } from './DomainEditor';
import type { Domain } from '@/types';

export function DomainList() {
  const { domains, createDomain, updateDomain, deleteDomain } = useDomains();
  const [editing, setEditing] = useState<Domain | null>(null);

  return (
    <div className="domain-list">
      <div className="domain-list-header">
        <h3>Domains</h3>
        <button onClick={() => setEditing({ id: '', name: '', systemPrompt: '' })}>
          Add Domain
        </button>
      </div>

      {domains.map((domain) => (
        <div key={domain.id} className="domain-item">
          <span className="domain-icon">{domain.icon || '📁'}</span>
          <span className="domain-name">{domain.name}</span>
          <span className="domain-description">{domain.description}</span>
          <button onClick={() => setEditing(domain)}>Edit</button>
          <button onClick={() => deleteDomain(domain.id)}>Delete</button>
        </div>
      ))}

      {editing && (
        <DomainEditor
          domain={editing}
          onSave={(d) => {
            if (d.id) {
              updateDomain(d);
            } else {
              createDomain(d);
            }
            setEditing(null);
          }}
          onCancel={() => setEditing(null)}
        />
      )}
    </div>
  );
}
```

**DomainPicker.tsx**:
```tsx
import { useDomains } from '../hooks/useDomains';

interface DomainPickerProps {
  value: string | null;
  onChange: (domainId: string | null) => void;
}

export function DomainPicker({ value, onChange }: DomainPickerProps) {
  const { domains } = useDomains();

  return (
    <select
      value={value || ''}
      onChange={(e) => onChange(e.target.value || null)}
    >
      <option value="">No domain</option>
      {domains.map((domain) => (
        <option key={domain.id} value={domain.id}>
          {domain.icon || '📁'} {domain.name}
        </option>
      ))}
    </select>
  );
}
```

**useDomains.ts**:
```typescript
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Domain } from '@/types';

export function useDomains() {
  const [domains, setDomains] = useState<Domain[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchDomains = useCallback(async () => {
    try {
      const result = await invoke<Domain[]>('domains_list');
      setDomains(result);
    } catch (error) {
      console.error('Failed to fetch domains:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchDomains();
  }, [fetchDomains]);

  const createDomain = async (domain: Omit<Domain, 'id'>) => {
    const created = await invoke<Domain>('domains_create', { ...domain });
    setDomains((prev) => [...prev, created]);
    return created;
  };

  const updateDomain = async (domain: Domain) => {
    const updated = await invoke<Domain>('domains_update', { ...domain });
    setDomains((prev) => prev.map((d) => (d.id === updated.id ? updated : d)));
    return updated;
  };

  const deleteDomain = async (domainId: string) => {
    await invoke('domains_delete', { domainId });
    setDomains((prev) => prev.filter((d) => d.id !== domainId));
  };

  return { domains, loading, createDomain, updateDomain, deleteDomain, refetch: fetchDomains };
}
```

### iOS Changes

**CodexStore.swift additions**:
```swift
@MainActor
public class CodexStore: ObservableObject {
    // ... existing properties ...

    @Published public var domains: [Domain] = []

    // Domain operations
    public func fetchDomains() async {
        do {
            domains = try await api.domainsList()
        } catch {
            print("Failed to fetch domains: \(error)")
        }
    }

    public func createDomain(_ domain: Domain) async throws -> Domain {
        let created = try await api.domainsCreate(domain)
        await fetchDomains()
        return created
    }

    public func updateDomain(_ domain: Domain) async throws -> Domain {
        let updated = try await api.domainsUpdate(domain)
        await fetchDomains()
        return updated
    }

    public func deleteDomain(_ domainId: String) async throws {
        try await api.domainsDelete(domainId)
        await fetchDomains()
    }

    public func domainFor(workspace: WorkspaceInfo) -> Domain? {
        guard let domainId = workspace.settings.domainId else { return nil }
        return domains.first { $0.id == domainId }
    }
}
```

**CodexMonitorAPI.swift additions**:
```swift
public func domainsList() async throws -> [Domain] {
    try await call(method: "domains_list", params: [:])
}

public func domainsCreate(_ domain: Domain) async throws -> Domain {
    try await call(method: "domains_create", params: [
        "name": domain.name,
        "description": domain.description as Any,
        "systemPrompt": domain.systemPrompt,
        "defaultModel": domain.defaultModel as Any,
        "defaultAccessMode": domain.defaultAccessMode as Any,
        "defaultReasoningEffort": domain.defaultReasoningEffort as Any,
        "color": domain.color as Any,
        "icon": domain.icon as Any
    ])
}

public func domainsUpdate(_ domain: Domain) async throws -> Domain {
    try await call(method: "domains_update", params: [
        "id": domain.id,
        "name": domain.name,
        "description": domain.description as Any,
        "systemPrompt": domain.systemPrompt,
        "defaultModel": domain.defaultModel as Any,
        "defaultAccessMode": domain.defaultAccessMode as Any,
        "defaultReasoningEffort": domain.defaultReasoningEffort as Any,
        "color": domain.color as Any,
        "icon": domain.icon as Any
    ])
}

public func domainsDelete(_ domainId: String) async throws {
    let _: [String: String] = try await call(method: "domains_delete", params: ["domainId": domainId])
}
```

**New View: DomainsView.swift**:
```swift
import SwiftUI
import CodexMonitorModels

struct DomainsView: View {
    @EnvironmentObject var store: CodexStore
    @State private var editingDomain: Domain?
    @State private var showingEditor = false

    var body: some View {
        List {
            ForEach(store.domains) { domain in
                DomainRow(domain: domain) {
                    editingDomain = domain
                    showingEditor = true
                }
            }
            .onDelete(perform: deleteDomains)
        }
        .navigationTitle("Domains")
        .toolbar {
            Button("Add") {
                editingDomain = nil
                showingEditor = true
            }
        }
        .sheet(isPresented: $showingEditor) {
            DomainEditorView(domain: editingDomain)
        }
        .task {
            await store.fetchDomains()
        }
    }

    private func deleteDomains(at offsets: IndexSet) {
        Task {
            for index in offsets {
                try? await store.deleteDomain(store.domains[index].id)
            }
        }
    }
}

struct DomainRow: View {
    let domain: Domain
    let onEdit: () -> Void

    var body: some View {
        HStack {
            Text(domain.icon ?? "📁")
            VStack(alignment: .leading) {
                Text(domain.name)
                    .font(.headline)
                if let desc = domain.description {
                    Text(desc)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            Spacer()
            Button("Edit", action: onEdit)
        }
    }
}
```

### Pros

- Clean domain abstraction with full CRUD
- UI for domain management on all platforms
- Domain defaults for model, access mode, reasoning effort
- Visual indicators (color, icon) for domains
- Scalable to future features (domain-scoped memory)
- Type-safe domain resolution

### Cons

- More code to write (~500-700 lines)
- Migration consideration for existing installations
- Settings file grows larger
- More UI complexity to maintain

---

## Option C: Custom Views Per Domain

**Effort**: 2-3 weeks
**Complexity**: High
**iOS Impact**: Significant (new view architecture)

### What It Does

Everything from Option B, PLUS:
- Custom view components per domain
- Domain-specific data fetching
- Non-chat layouts (cards, tables, summaries)
- Data source abstraction layer

### Additional Type Changes

```typescript
export type DomainViewType = 'chat' | 'cards' | 'table' | 'hybrid';

export type DomainDataSource = {
  type: 'obsidian' | 'supabase' | 'file' | 'memory';
  path?: string;
  query?: string;
};

export type Domain = {
  // ... from Option B
  viewType: DomainViewType;
  dataSources?: DomainDataSource[];
  viewConfig?: Record<string, unknown>;
};
```

### View Architecture

**Desktop (`src/features/domains/views/`):**
```
├── ChatView.tsx           # Default Codex conversation
├── CardsView.tsx          # Life stream style cards
├── TableView.tsx          # Delivery/finance tabular
├── HybridView.tsx         # Cards + chat overlay
├── DomainViewRouter.tsx   # Selects view based on domain
└── data/
    ├── DataSourceProvider.tsx
    ├── ObsidianDataSource.ts
    ├── MemoryDataSource.ts
    └── useDataSource.ts
```

**iOS:**
```
Views/Domains/
├── DomainChatView.swift
├── DomainCardsView.swift
├── DomainTableView.swift
├── DomainHybridView.swift
├── DomainViewRouter.swift
└── DataSource/
    ├── DataSourceProtocol.swift
    ├── ObsidianDataSource.swift
    └── MemoryDataSource.swift
```

### Data Source Layer

```typescript
// src/features/domains/data/types.ts
export interface DataSourceEntry {
  id: string;
  type: string;
  content: string;
  timestamp: number;
  metadata?: Record<string, unknown>;
}

export interface DataSource {
  type: string;
  fetch(): Promise<DataSourceEntry[]>;
  subscribe?(callback: (entries: DataSourceEntry[]) => void): () => void;
}
```

**ObsidianDataSource.ts**:
```typescript
export class ObsidianDataSource implements DataSource {
  type = 'obsidian';

  constructor(private path: string) {}

  async fetch(): Promise<DataSourceEntry[]> {
    const content = await invoke<string>('read_file', { path: this.path });
    return this.parseStreamEntries(content);
  }

  private parseStreamEntries(markdown: string): DataSourceEntry[] {
    // Parse Obsidian stream format
    const entries: DataSourceEntry[] = [];
    // ... parsing logic
    return entries;
  }
}
```

### Example: Life Domain Hybrid View

```tsx
// src/features/domains/views/HybridView.tsx
import { useDataSource } from '../data/useDataSource';
import { StreamCard } from './components/StreamCard';
import { ChatOverlay } from './components/ChatOverlay';

interface HybridViewProps {
  domain: Domain;
  workspaceId: string;
}

export function HybridView({ domain, workspaceId }: HybridViewProps) {
  const { entries, loading, error } = useDataSource(domain.dataSources?.[0]);
  const [showChat, setShowChat] = useState(false);

  if (loading) return <LoadingSpinner />;
  if (error) return <ErrorMessage error={error} />;

  return (
    <div className="hybrid-view">
      <div className="cards-grid">
        {entries.map((entry) => (
          <StreamCard
            key={entry.id}
            entry={entry}
            onAskAbout={() => {
              // Prefill chat with context about this entry
              setShowChat(true);
            }}
          />
        ))}
      </div>

      <button
        className="chat-toggle"
        onClick={() => setShowChat(!showChat)}
      >
        {showChat ? 'Hide Chat' : 'Ask AI'}
      </button>

      {showChat && (
        <ChatOverlay
          domain={domain}
          workspaceId={workspaceId}
          onClose={() => setShowChat(false)}
        />
      )}
    </div>
  );
}
```

### Pros

- Full life-chat feature parity
- Rich domain-specific experiences
- Maximum flexibility for future domains
- Visual data exploration beyond chat

### Cons

- Significant development effort (2-3 weeks)
- Per-domain maintenance burden
- Complex data source abstraction
- May fragment the UX if poorly designed
- Risk of scope creep

---

## Recommendation

**Start with Option B (First-Class Domains).**

### Rationale

1. **Option A is too minimal** - The prompt injection alone doesn't provide enough value. Without UI for domain management, users can't easily create/edit domains, and there's no path to domain defaults.

2. **Option B hits the sweet spot** - It provides the core abstraction needed:
   - Full domain CRUD with UI
   - System prompt injection at thread start
   - Domain defaults for common settings
   - Foundation for future expansion

3. **Option C is premature** - Custom views are complex and risky:
   - Data source integration adds significant complexity
   - Each custom view is a maintenance burden
   - Better to validate the domain concept with Option B first

### Suggested Phase Plan

**Phase 1 (Option B)**: 5-7 days
- Domain type definitions
- Daemon CRUD operations
- Thread initialization with domain
- Desktop domain settings UI
- iOS domain sync

**Phase 2 (Future)**: After validation
- Domain-scoped memory (memories tagged with domain)
- Default workspace templates per domain
- Consider custom views for specific high-value domains

---

## Implementation Order

### Phase 1: Types and Persistence (Day 1-2)

1. Add `Domain` type to:
   - `src/types.ts`
   - `src-tauri/src/types.rs`
   - `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift`

2. Add `domains: Domain[]` to `AppSettings` (all platforms)

3. Add `domainId?: string` to `WorkspaceSettings` (all platforms)

4. Update settings serialization tests

### Phase 2: Daemon RPC (Day 2-3)

1. Implement `domains_list`, `domains_create`, `domains_update`, `domains_delete`

2. Modify `start_thread` to inject domain system prompt

3. Add RPC handler registrations

4. Test with curl/netcat

### Phase 3: Desktop UI (Day 3-5)

1. Create `src/features/domains/` directory structure

2. Implement `useDomains` hook

3. Build `DomainList`, `DomainEditor`, `DomainPicker` components

4. Add Domains section to Settings view

5. Add domain picker to workspace settings panel

### Phase 4: iOS Sync (Day 5-6)

1. Add domain methods to `CodexMonitorAPI`

2. Add domain state to `CodexStore`

3. Create `DomainsView` and `DomainEditorView`

4. Add domain indicator to workspace list

### Phase 5: Testing & Polish (Day 6-7)

1. End-to-end test: create domain, assign to workspace, start thread, verify prompt

2. Test domain defaults application

3. Handle edge cases (deleted domains, etc.)

4. UI polish and responsive design

---

## Files to Modify

### Core Types
- `src/types.ts`
- `src-tauri/src/types.rs`
- `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift`

### Settings & State
- `src-tauri/src/state.rs` (if AppSettings loaded there)
- `src/features/settings/hooks/useAppSettings.ts`
- `src/features/app/hooks/useAppSettingsController.ts`

### Thread Initialization
- `src-tauri/src/bin/codex_monitor_daemon.rs` (`start_thread`, RPC handlers)
- `src-tauri/src/lib.rs` (Tauri commands for local mode)

### New Desktop Files
- `src/features/domains/components/DomainList.tsx`
- `src/features/domains/components/DomainEditor.tsx`
- `src/features/domains/components/DomainPicker.tsx`
- `src/features/domains/components/DomainBadge.tsx`
- `src/features/domains/hooks/useDomains.ts`
- `src/features/domains/index.ts`

### Modified Desktop Files
- `src/features/settings/components/SettingsView.tsx` (add Domains section)
- `src/features/workspaces/components/WorkspaceSettings.tsx` (add domain picker)
- `src/features/workspaces/components/WorkspaceRow.tsx` (optional domain badge)

### New iOS Files
- `ios/CodexMonitorMobile/CodexMonitorMobile/Views/DomainsView.swift`
- `ios/CodexMonitorMobile/CodexMonitorMobile/Views/DomainEditorView.swift`
- `ios/CodexMonitorMobile/CodexMonitorMobile/Views/DomainPicker.swift`

### Modified iOS Files
- `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorRPC/CodexMonitorAPI.swift`
- `ios/CodexMonitorMobile/CodexMonitorMobile/CodexStore.swift`
- `ios/CodexMonitorMobile/CodexMonitorMobile/Views/SettingsView.swift`
- `ios/CodexMonitorMobile/CodexMonitorMobile/Views/WorkspaceRow.swift`

---

## Questions to Decide

Before implementation, clarify these design decisions:

### 1. Scope: Global or Per-Installation?

**Recommendation**: Global in `AppSettings`

Domains should be shared across the daemon and all clients. They're configuration, not per-workspace data.

### 2. Prompt Behavior: Override or Combine?

**Recommendation**: Domain is base, user-selected prompt appends

When a user selects a custom prompt from the library AND the workspace has a domain:
- Domain system prompt is injected first
- User prompt is appended after

This allows domain-specific context plus task-specific instructions.

### 3. Relationship to WorkspaceGroup

**Recommendation**: Keep separate for now

`WorkspaceGroup` is purely for sidebar organization. `Domain` is for behavior/context. A workspace in "Personal Projects" group might have the "coding" domain.

Later, we might allow domains to imply groups, but start decoupled.

### 4. Domain-Scoped Memory

**Recommendation**: Defer to Phase 2

Memory entries could be tagged with `domainId` for domain-specific retrieval. This is valuable but adds complexity. Get basic domains working first.

### 5. Default Domain

**Recommendation**: No default domain initially

New workspaces have no domain. Users explicitly assign domains. Later, we could add a "default domain for new workspaces" setting.

---

## Testing Checklist

### Domain CRUD
- [ ] Create a domain with all fields populated
- [ ] List domains returns the created domain
- [ ] Update domain name and system prompt
- [ ] Delete domain removes it from list
- [ ] Deleted domain doesn't crash workspace with that domainId

### Thread Initialization
- [ ] Start thread in workspace WITH domain - verify system prompt injected
- [ ] Start thread in workspace WITHOUT domain - verify no system prompt
- [ ] Domain with empty system prompt - verify no error

### Desktop UI
- [ ] Domains section appears in Settings
- [ ] Can create/edit/delete domains
- [ ] Domain picker appears in workspace settings
- [ ] Changing domain updates workspace settings
- [ ] Domain badge shows on workspace row (if implemented)

### iOS Sync
- [ ] Domains load on app start
- [ ] Can view domains list
- [ ] Can edit domains
- [ ] Domain shows for workspace
- [ ] Thread started from iOS uses domain prompt

### Edge Cases
- [ ] Long system prompt (10k+ chars) - verify no truncation
- [ ] Unicode in domain name/prompt
- [ ] Concurrent domain edits from Desktop + iOS
- [ ] Migration from installation without domains field

---

## Example Domains to Create

Once implemented, create these starter domains:

### Life Domain
```json
{
  "name": "Life",
  "description": "Life management, health, daily logging",
  "icon": "🏠",
  "color": "#4CAF50",
  "systemPrompt": "You are helping JMWillis with life management...",
  "defaultReasoningEffort": "medium"
}
```

### Coding Domain
```json
{
  "name": "Coding",
  "description": "Software development tasks",
  "icon": "💻",
  "color": "#2196F3",
  "systemPrompt": "You are a senior software engineer...",
  "defaultAccessMode": "full-access",
  "defaultReasoningEffort": "high"
}
```

### Delivery Domain
```json
{
  "name": "Delivery",
  "description": "Food delivery work and earnings",
  "icon": "🚗",
  "color": "#FF9800",
  "systemPrompt": "You are helping with food delivery optimization...",
  "defaultReasoningEffort": "low"
}
```

### YouTube Domain
```json
{
  "name": "YouTube",
  "description": "Content creation and video planning",
  "icon": "🎬",
  "color": "#F44336",
  "systemPrompt": "You are a YouTube content strategist...",
  "defaultReasoningEffort": "high"
}
```

---

## Summary

| Option | Effort | Value | Risk |
|--------|--------|-------|------|
| A: Minimal | 2-3 days | Low | Low |
| **B: First-Class** | 5-7 days | High | Medium |
| C: Custom Views | 2-3 weeks | Very High | High |

**Start with Option B.** It provides the domain abstraction needed for context injection, with full CRUD and UI, positioning for future expansion without over-engineering.
