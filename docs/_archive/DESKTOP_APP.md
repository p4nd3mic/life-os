# Desktop App (Tauri + React)

This document describes the **desktop CodexMonitor application**.

The desktop app is split into:
- **React frontend** (`src/`)
- **Tauri Rust backend** (`src-tauri/src/`)

The Rust backend can run in two modes:
- **Local backend mode** (default): desktop app spawns `codex app-server` locally.
- **Remote backend mode**: desktop app proxies requests to a remote daemon over TCP (`src-tauri/src/remote_backend.rs`).

Key paths:
- React entry: `src/main.tsx`
- React root: `src/App.tsx`
- Tauri backend entry: `src-tauri/src/main.rs` → `codex_monitor_lib::run()`
- Tauri command registry: `src-tauri/src/lib.rs`

---

## React component tree (high level)

Entry:
- `src/main.tsx` mounts `<App />`.

Top level:
- `src/App.tsx`
  - Bootstraps app settings + backend mode
  - Wires together feature hooks (workspaces, threads, git, prompts, terminal, dictation)
  - Registers event listeners (app-server events, terminal output, dictation events)
  - Renders `AppLayout`

Layout:
- `src/features/app/components/AppLayout.tsx`
  - Chooses layout based on viewport/device profile:
    - `DesktopLayout`
    - `TabletLayout`
    - `PhoneLayout`

Conversation surface (core user journey):
- Workspace selection (sidebar / project list)
- Thread selection (threads list)
- Conversation view (items + plan)
- Composer (text + options + attachments)
- Optional side panels (Git, Prompts, Terminal)

Composer attachments:
- Drag/drop images to attach directly to the composer.
- Drag/drop non-image files to insert their paths into the prompt text.

The exact composition varies per layout, but those are the core conceptual nodes.

---

## Frontend module structure (`src/features/*`)

The frontend is organized by feature folders under `src/features/`:

| Feature | Folder | Responsibilities |
|---|---|---|
| app | `src/features/app/` | Layout shell, app-level event routing (`useAppServerEvents`) |
| workspaces | `src/features/workspaces/` | Workspace list, connect/add/remove, workspace settings |
| threads | `src/features/threads/` | Thread list, thread lifecycle, items store, approvals |
| composer | `src/features/composer/` | Message composer UI, model/effort/accessMode selection |
| git | `src/features/git/` | Status/diffs/log/branches, staging/commit/push/pull |
| github | `src/features/github/` | PR/issue panels via daemon/backend gh integration |
| prompts | `src/features/prompts/` | Prompt library UI, CRUD + move between scopes |
| terminal | `src/features/terminal/` | Terminal tabs, PTY output rendering |
| dictation | `src/features/dictation/` | Desktop-only audio dictation + transcription |
| settings | `src/features/settings/` | Settings UI (backend mode, paths, UI scale) |
| history | `src/features/history/` | Local history and last-activity caches |
| timeline | `src/features/timeline/` | Timeline visualization of turns/items |
| toasts | `src/features/toasts/` | Toast notification UI |
| debug | `src/features/debug/` | Debug panels/logging |
| navigation | `src/features/navigation/` | Routing + navigation helpers |
| **life** | `src/features/life/` | **Life workspace: domain dashboards, combined prompts** |

---

## Life Workspace Feature (`src/features/life/`)

The Life workspace is a unified hub for all life domains with domain-specific dashboard views.

### Component Structure

```
src/features/life/
├── components/
│   ├── LifeWorkspaceView.tsx       # Main container
│   ├── DomainSelector.tsx          # Right panel domain tabs
│   ├── DomainViewContainer.tsx     # Switches between domain views
│   │
│   ├── domains/
│   │   ├── DeliveryDashboard.tsx   # Earnings, orders, hourly rates
│   │   ├── NutritionDashboard.tsx  # Macros, meals, weekly trends
│   │   ├── ExerciseDashboard.tsx   # Workouts, streaks, progress
│   │   ├── MediaDashboard.tsx      # Bookshelf covers, ratings
│   │   ├── YouTubeDashboard.tsx    # Pipeline by tier, ideas
│   │   └── FinanceDashboard.tsx    # Bills, due dates, calendar
│   │
│   └── shared/
│       ├── StatCard.tsx            # Reusable stat display
│       ├── TimeRangeSelector.tsx   # Today/Week/Month/Lifetime
│       ├── DataTable.tsx           # Sortable table
│       ├── CoverImage.tsx          # Media covers with fallback
│       └── ProgressBar.tsx         # Visual progress
│
├── hooks/
│   ├── useLifeWorkspace.ts         # State management
│   ├── useDomainDashboard.ts       # Generic data fetching
│   ├── useDeliveryData.ts
│   ├── useNutritionData.ts
│   ├── useMediaData.ts
│   ├── useYouTubeData.ts
│   └── useFinanceData.ts
│
└── styles/
    └── life-dashboard.css
```

### Domain View State

```typescript
type LifeDomain = 'delivery' | 'nutrition' | 'exercise' | 'media' | 'youtube' | 'finance';

interface DomainViewState {
  activeDomain: LifeDomain | null;  // null = show chat
  timeRange: 'today' | 'week' | 'month' | 'lifetime';
  filters: Record<string, string>;
  sortBy: string;
  sortDirection: 'asc' | 'desc';
}
```

### Center Panel Behavior

The center panel switches between two modes:

1. **Chat Mode** (default): Standard Codex conversation with full life context injected
2. **Dashboard Mode**: When a domain is selected, renders the domain-specific dashboard component

```tsx
// LifeWorkspaceView.tsx
function LifeWorkspaceView() {
  const { activeDomain, setActiveDomain } = useLifeWorkspace();

  return (
    <div className="life-workspace">
      <div className="center-panel">
        {activeDomain ? (
          <DomainViewContainer
            domain={activeDomain}
            onBack={() => setActiveDomain(null)}
          />
        ) : (
          <ChatPanel />
        )}
      </div>
      <div className="right-panel">
        <DomainSelector
          activeDomain={activeDomain}
          onSelect={setActiveDomain}
        />
      </div>
    </div>
  );
}
```

### Combined Prompt Injection

When starting a thread in the Life workspace, all four domain prompts are combined:

```typescript
// useLifeWorkspace.ts
async function getLifeWorkspacePrompt(): Promise<string> {
  const prompts = await Promise.all([
    readPromptFile('workspace-delivery-finance.md'),
    readPromptFile('workspace-food-exercise.md'),
    readPromptFile('workspace-media.md'),
    readPromptFile('workspace-youtube.md'),
  ]);

  return prompts.join('\n---\n') + '\n\nYou are the life assistant...';
}
```

### Dashboard Data Hooks

Each domain has a dedicated hook following this pattern:

```typescript
// useDeliveryData.ts
function useDeliveryData(timeRange: TimeRange = 'today') {
  const [data, setData] = useState<DeliveryDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke('get_delivery_dashboard', { workspaceId, range: timeRange })
      .then(setData)
      .catch(setError)
      .finally(() => setLoading(false));
  }, [timeRange]);

  return { data, loading, error, refetch };
}
```

### Visual Design

**Dark grid aesthetic** matching user's iPad logging style:

```css
/* life-dashboard.css */
.life-workspace {
  background: linear-gradient(to bottom, #1a1a2e, #16213e);
  background-image:
    linear-gradient(rgba(255,255,255,0.03) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255,255,255,0.03) 1px, transparent 1px);
  background-size: 20px 20px;
}

.stat-card {
  background: rgba(30, 30, 50, 0.8);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
}

/* Domain accent colors */
.domain-delivery { --accent: #3b82f6; }  /* blue-500 */
.domain-nutrition { --accent: #22c55e; } /* green-500 */
.domain-finance { --accent: #eab308; }   /* yellow-500 */
.domain-youtube { --accent: #ef4444; }   /* red-500 */
.domain-media { --accent: #a855f7; }     /* purple-500 */
```

### Progressive Disclosure

To avoid overwhelming users (Schwartz's paradox of choice):

| Domain | Initial View | Expanded View |
|--------|--------------|---------------|
| Delivery | Today's stats + last 5 orders | Full history with filters |
| Nutrition | Today's macros + meals | Weekly trends, food frequency |
| Media | Last 10 watched + backlog | Full library with type filters |
| YouTube | S-tier + in-progress | Full pipeline by tier |
| Finance | Next 7 days bills | Full month calendar |

---

## State management approach

The app does not use a single monolithic global store; it composes state from:
- feature-level hooks (`useWorkspaces`, `useThreads`, etc.)
- reducers for complex state (threads/items)
- local `useState` for UI state
- `localStorage` for “sticky” UX settings (pinned threads, panel sizes)

Common persistence keys:
- `codexmonitor.threadLastUserActivity`
- `codexmonitor.pinnedThreads`

See:
- `src/features/history/utils/localStorage.ts`

---

## Key hooks (agent-focused)

### `useWorkspaces`

Path: `src/features/workspaces/hooks/useWorkspaces.ts`

Responsibilities:
- Fetch list of workspaces from backend (`list_workspaces`)
- Add/remove workspaces (`add_workspace`, `remove_workspace`)
- Connect workspace sessions (`connect_workspace`)
- Worktree operations (`add_worktree`, `remove_worktree`, `rename_worktree`)
- Update workspace settings (`update_workspace_settings`)
- Track “active workspace” selection for the UI

### `useThreads`

Path: `src/features/threads/hooks/useThreads.ts`

Responsibilities:
- Maintain the threads list for the active workspace
- Start/resume/archive threads
- Maintain a local cache of conversation items per thread
- Integrate with app-server streaming events:
  - append deltas (`item/*Delta`)
  - mark completion (`item/completed`, `turn/completed`)
  - update plans (`turn/plan/updated`)
  - handle approvals (`*requestApproval*` methods)
- Persist per-thread “last activity” timestamps

### `useAppServerEvents`

Path: `src/features/app/hooks/useAppServerEvents.ts`

Responsibilities:
- Listen to backend-emitted `app-server-event` payloads
- Normalize the raw message into:
  - `AppServerMessage`: `{ method, params }` from Codex
- Dispatch to registered callbacks by method string

This hook is the main bridge between:
- raw Codex app-server protocol
- the rest of the React state machine

### Git hooks

- `src/features/git/hooks/useGitStatus.ts` — status polling, staged/unstaged lists
- `src/features/git/hooks/useGitDiffs.ts` — file diff list + selection
- `src/features/git/hooks/useGitLog.ts` — commit history view
- `src/features/git/hooks/useGitBranches.ts` — branch picker/create/checkout

Each wraps a thin Tauri command in a React state machine with loading/error state.

### Terminal hooks

- `src/features/terminal/hooks/useTerminalSessions.ts` — opens/closes sessions and binds output events
- `src/features/terminal/hooks/useTerminalOutput.ts` — accumulation & trimming of output buffers

---

## Tauri IPC

### How React calls Rust

React uses wrapper functions in:

- `src/services/tauri.ts`

Example pattern:

```ts
import { invoke } from "@tauri-apps/api/core";

export async function listWorkspaces() {
  return invoke("list_workspaces");
}
```

### How Rust exposes commands

Rust commands are registered in:

- `src-tauri/src/lib.rs` (`tauri::generate_handler![ ... ]`)

Each command is implemented in a feature module (settings/workspaces/codex/git/etc.).

---

## All Tauri commands (desktop)

These are the commands registered in `src-tauri/src/lib.rs`.

> Not all of these are available when running against a remote daemon; see “Remote backend mode” below.

| Tauri command (JS invoke) | Rust handler | Notes |
|---|---|---|
| `get_app_settings` | `settings::get_app_settings` | |
| `update_app_settings` | `settings::update_app_settings` | |
| `menu_set_accelerators` | `menu::menu_set_accelerators` | |
| `codex_doctor` | `codex::codex_doctor` | |
| `list_workspaces` | `workspaces::list_workspaces` | |
| `is_workspace_path_dir` | `workspaces::is_workspace_path_dir` | |
| `add_workspace` | `workspaces::add_workspace` | |
| `add_clone` | `workspaces::add_clone` | |
| `add_worktree` | `workspaces::add_worktree` | |
| `remove_workspace` | `workspaces::remove_workspace` | |
| `remove_worktree` | `workspaces::remove_worktree` | |
| `rename_worktree` | `workspaces::rename_worktree` | |
| `rename_worktree_upstream` | `workspaces::rename_worktree_upstream` | |
| `apply_worktree_changes` | `workspaces::apply_worktree_changes` | |
| `update_workspace_settings` | `workspaces::update_workspace_settings` | |
| `update_workspace_codex_bin` | `workspaces::update_workspace_codex_bin` | |
| `start_thread` | `codex::start_thread` | |
| `send_user_message` | `codex::send_user_message` | |
| `turn_interrupt` | `codex::turn_interrupt` | |
| `start_review` | `codex::start_review` | |
| `respond_to_server_request` | `codex::respond_to_server_request` | |
| `remember_approval_rule` | `codex::remember_approval_rule` | |
| `get_commit_message_prompt` | `codex::get_commit_message_prompt` | |
| `generate_commit_message` | `codex::generate_commit_message` | |
| `resume_thread` | `codex::resume_thread` | |
| `list_threads` | `codex::list_threads` | |
| `archive_thread` | `codex::archive_thread` | |
| `collaboration_mode_list` | `codex::collaboration_mode_list` | |
| `connect_workspace` | `workspaces::connect_workspace` | |
| `get_git_status` | `git::get_git_status` | |
| `list_git_roots` | `git::list_git_roots` | |
| `get_git_diffs` | `git::get_git_diffs` | |
| `get_git_log` | `git::get_git_log` | |
| `get_git_commit_diff` | `git::get_git_commit_diff` | |
| `get_git_remote` | `git::get_git_remote` | |
| `stage_git_file` | `git::stage_git_file` | |
| `stage_git_all` | `git::stage_git_all` | |
| `unstage_git_file` | `git::unstage_git_file` | |
| `revert_git_file` | `git::revert_git_file` | |
| `revert_git_all` | `git::revert_git_all` | |
| `commit_git` | `git::commit_git` | |
| `push_git` | `git::push_git` | |
| `pull_git` | `git::pull_git` | |
| `sync_git` | `git::sync_git` | |
| `get_github_issues` | `git::get_github_issues` | |
| `get_github_pull_requests` | `git::get_github_pull_requests` | |
| `get_github_pull_request_diff` | `git::get_github_pull_request_diff` | |
| `get_github_pull_request_comments` | `git::get_github_pull_request_comments` | |
| `list_workspace_files` | `workspaces::list_workspace_files` | |
| `read_workspace_file` | `workspaces::read_workspace_file` | |
| `open_workspace_in` | `workspaces::open_workspace_in` | |
| `list_git_branches` | `git::list_git_branches` | |
| `checkout_git_branch` | `git::checkout_git_branch` | |
| `create_git_branch` | `git::create_git_branch` | |
| `model_list` | `codex::model_list` | |
| `account_rate_limits` | `codex::account_rate_limits` | |
| `skills_list` | `codex::skills_list` | |
| `prompts_list` | `prompts::prompts_list` | |
| `prompts_create` | `prompts::prompts_create` | |
| `prompts_update` | `prompts::prompts_update` | |
| `prompts_delete` | `prompts::prompts_delete` | |
| `prompts_move` | `prompts::prompts_move` | |
| `prompts_workspace_dir` | `prompts::prompts_workspace_dir` | |
| `prompts_global_dir` | `prompts::prompts_global_dir` | |
| `terminal_open` | `terminal::terminal_open` | |
| `terminal_write` | `terminal::terminal_write` | |
| `terminal_resize` | `terminal::terminal_resize` | |
| `terminal_close` | `terminal::terminal_close` | |
| `dictation_model_status` | `dictation::dictation_model_status` | |
| `dictation_download_model` | `dictation::dictation_download_model` | |
| `dictation_cancel_download` | `dictation::dictation_cancel_download` | |
| `dictation_remove_model` | `dictation::dictation_remove_model` | |
| `dictation_start` | `dictation::dictation_start` | |
| `dictation_stop` | `dictation::dictation_stop` | |
| `dictation_cancel` | `dictation::dictation_cancel` | |
| `local_usage_snapshot` | `local_usage::local_usage_snapshot` | |

---

## Backend-emitted events (desktop)

The Rust backend emits events to the React frontend:

| Event name | Payload | Source |
|---|---|---|
| `app-server-event` | `AppServerEvent` | forwarded from Codex app-server (`src-tauri/src/backend/events.rs`) |
| `terminal-output` | `TerminalOutput` | PTY output (local or remote) |
| `dictation-download` | progress/status | dictation model download (`src-tauri/src/dictation/`) |
| `dictation-event` | transcription chunks | dictation runtime |

React subscribes via:
- `src/services/events.ts`

---

## Remote backend mode (Desktop → Daemon)

When `AppSettings.backendMode == "remote"` (see `src/types.ts` and backend settings UI), the desktop backend routes almost all stateful operations through:

- `src-tauri/src/remote_backend.rs`

Mechanics:
1. Tauri backend opens a TCP connection to `remoteBackendHost`
2. If a token is set, it calls daemon `auth`
3. For each command, it sends `{id, method, params}`
4. It forwards daemon notifications to the UI as normal Tauri events:
   - daemon `app-server-event` → Tauri `app-server-event`
   - daemon `terminal-output` → Tauri `terminal-output`

### Desktop-only commands

Some commands are inherently local-only (they manipulate local OS UI, audio, menus):

- `open_workspace_in`
- `menu_set_accelerators`
- dictation commands (`dictation_*`)

In remote mode, these are either:
- still executed locally (if meaningful), or
- disabled / return errors (if they require local OS integration on the remote host)

---

## Common gotchas & landmines (desktop)

- **Codex-backed commands return nested envelopes**  
  Many `codex` commands return the *raw app-server response* (with its own `result`), so JS often needs to read `response.result` (see `useThreads`).

- **Remote vs local type drift**  
  The daemon API uses a subset/superset of desktop commands. If you add a new command, decide whether it must be supported remotely too.

- **Event ordering assumptions**  
  UI logic often assumes `item/started` happens before deltas and `item/completed` eventually arrives. Network retries can cause missed updates; ensure reducers are defensive.

- **Terminal output can be large**  
  Both desktop and daemon stream terminal output as raw strings. Without trimming, memory can grow.


---

## Browser Panel (2026-01-26)

- **Path**: `src/features/browser/components/BrowserPanel.tsx`
- Located in right panel tabs
- Supports auto-refresh (3s/5s/10s) which pauses when the app is backgrounded

### Features

| Feature | Description |
|---------|-------------|
| Session management | Create, list, close browser sessions |
| Navigation | Enter URL, navigate, view current page |
| Screenshot | Capture and display page screenshot |
| Auto-refresh | Configurable interval (3s/5s/10s) with visibility-aware pausing |
| Interactive elements | Click, type, press keyboard keys |

### Auto-refresh Pause Behavior

Uses `document.visibilitychange` event to detect when the app is backgrounded:

```tsx
useEffect(() => {
  const handleVisibility = () => {
    setIsVisible(document.visibilityState === "visible");
  };
  document.addEventListener("visibilitychange", handleVisibility);
  return () => document.removeEventListener("visibilitychange", handleVisibility);
}, []);
```

When `isVisible` is false, the auto-refresh timer is paused to conserve resources and prevent unnecessary browser operations.

---

## Skills Panel (2026-01-26)

- **Path**: `src/features/skills/components/SkillsPanel.tsx`
- Located in settings/config area of the app

### Features

| Feature | Description |
|---------|-------------|
| Skill listing | Shows all installed skills with name and description |
| Enable/disable toggle | Checkbox to enable or disable each skill |
| Validation status | Shows issues (missing binaries, env vars, OS incompatibility) |
| Git installation | Install skills from git repository URL |
| Persistence | Config saved to `{CODEX_HOME}/skills/config.json` |

### State Management

```tsx
const [skills, setSkills] = useState<{ name: string; path: string; description?: string }[]>([]);
const [enabledSkills, setEnabledSkills] = useState<Set<string>>(new Set());
```

### Configuration Persistence

On toggle change, writes to config via `skills_config_write`:

```tsx
const persistConfig = async (nextEnabled: Set<string>) => {
  const enabled = skills.filter(s => nextEnabled.has(`${s.name}|${s.path}`));
  const disabled = skills.filter(s => !nextEnabled.has(`${s.name}|${s.path}`));
  await skillsConfigWrite(workspaceId, { enabled, disabled });
};
```

### Skill Resolution Logic

Skills enabled/disabled state is resolved from config using:
1. If `config.enabled` is non-empty, use that list
2. Else if `config.disabled` is non-empty, enable all except those
3. Else enable all skills by default

---

## Prompt History (2026-01-28)

- **Path**: `src/features/composer/hooks/usePromptHistory.ts`
- Integrated into the Composer component

### Features

| Feature | Description |
|---------|-------------|
| Arrow up/down navigation | Recall previous prompts when composer is empty |
| Per-workspace storage | Each workspace has isolated history |
| 200 prompt limit | Oldest prompts are dropped when limit reached |
| Draft preservation | Current draft is saved when navigating history |
| Auto-migration | Default history migrates to workspace-specific key |

### Hook API

```typescript
const {
  handleHistoryKeyDown,    // Attach to textarea onKeyDown
  handleHistoryTextChange, // Call when text changes (resets navigation)
  recordHistory,           // Call when message is sent
  resetHistoryNavigation,  // Reset navigation state
} = usePromptHistory({
  historyKey: workspace?.id,  // null uses "default" key
  text,
  hasAttachments,
  disabled,
  isAutocompleteOpen,
  textareaRef,
  setText,
  setSelectionStart,
});
```

### Storage Keys

- **Pattern**: `codexmonitor.promptHistory.<key>`
- **Default**: `codexmonitor.promptHistory.default`
- **Per-workspace**: `codexmonitor.promptHistory.<workspaceId>`

---

## Image Diff Card (2026-01-28)

- **Path**: `src/features/git/components/ImageDiffCard.tsx`
- Renders in the git diff viewer for image files

### Props

```typescript
type ImageDiffCardProps = {
  path: string;              // File path
  status: string;            // Git status (A/M/D)
  oldImageData?: string;     // Base64 encoded old image
  newImageData?: string;     // Base64 encoded new image
  oldImageMime?: string;     // MIME type override
  newImageMime?: string;     // MIME type override
  isSelected: boolean;       // Highlight state
};
```

### Display Modes

| Status | Layout |
|--------|--------|
| Modified | Side-by-side comparison |
| Added | Single pane (new image) |
| Deleted | Single pane (old image) |

### Supported Formats

PNG, JPG, JPEG, GIF, WebP, SVG, BMP, ICO

---

## Settings View - Global Config Tabs (2026-01-28)

### Global AGENTS.md Editor

- **Hook**: `src/features/settings/hooks/useGlobalAgentsMd.ts`
- **File**: `~/.codex/AGENTS.md`
- Edit global agent instructions that apply to all Codex sessions

### Global config.toml Editor

- **Hook**: `src/features/settings/hooks/useGlobalCodexConfigToml.ts`
- **File**: `~/.codex/config.toml`
- Edit Codex configuration (feature flags, MCP servers, etc.)

### Shared File Editor Pattern

Both use `useFileEditor` from `src/features/shared/hooks/useFileEditor.ts`:

```typescript
const {
  content,      // Current editor content
  exists,       // Whether file exists on disk
  truncated,    // Whether content was truncated
  isLoading,    // Loading state
  isSaving,     // Saving state
  isDirty,      // Whether content differs from saved
  error,        // Error message if any
  setContent,   // Update content
  refresh,      // Reload from disk
  save,         // Save to disk
} = useFileEditor({ key, read, write, readErrorTitle, writeErrorTitle });
```

---

## Sidebar Search (2026-01-28)

- **Location**: Workspace sidebar
- Filter workspaces by name using a search input

### Features

| Feature | Description |
|---------|-------------|
| Real-time filtering | Workspaces filter as you type |
| Case-insensitive | Search matches regardless of case |
| Preserves groups | Filtered view maintains group structure |

---

## WorkspaceHome Component (2026-01-28)

- **Path**: `src/features/workspaces/components/WorkspaceHome.tsx`
- Displays recent threads when no thread is selected

### Features

| Feature | Description |
|---------|-------------|
| Recent threads | Shows up to 8 most recent threads |
| Quick access | Click to select and resume thread |
| Relative time | Shows time since last update |

### Props

```typescript
type WorkspaceHomeProps = {
  workspace: WorkspaceInfo;
  threads: ThreadSummary[];
  onSelectThread: (threadId: string) => void;
};
```

---

## Domain Accent Colors (2026-01-28)

Life workspace domains now have consistent accent colors via CSS variables:

```css
/* Domain accent colors */
.domain-delivery { --domain-accent: var(--blue-500); }
.domain-nutrition { --domain-accent: var(--green-500); }
.domain-exercise { --domain-accent: var(--orange-500); }
.domain-finance { --domain-accent: var(--yellow-500); }
.domain-youtube { --domain-accent: var(--red-500); }
.domain-media { --domain-accent: var(--purple-500); }
```

### Usage

Components within a domain context can use `var(--domain-accent)` for consistent theming.

---

## Manual Cover Overrides (2026-01-28)

Media and YouTube dashboards support manual cover image overrides.

### Override File

- **Path**: `Obsidian/Indexes/media.covers.overrides.json`

### Format

```json
{
  "media-id-123": "https://example.com/custom-cover.jpg",
  "youtube-idea-456": "/local/path/to/cover.png"
}
```

### Resolution Order

1. Manual override from `media.covers.overrides.json`
2. API-fetched cover (TMDB, IGDB)
3. Fallback placeholder
