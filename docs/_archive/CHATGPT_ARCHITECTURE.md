# Life Workspace Architecture (ChatGPT 5.2 Pro)

*Generated: 2026-01-28*

## 1. Executive Summary

You're keeping **two workspaces** in the left sidebar:

* **CodexMonitor** (coding): no behavioral changes
* **Life** (new): one unified life workspace that contains:

  * **Center panel**: normal Codex chat threads by default
  * **Right panel**: **domain selector tabs** (Delivery / Nutrition / Exercise / Media / YouTube / Finance)
  * When a domain is selected, the **center panel swaps** from chat → a **real React dashboard view** (not markdown), and you can "Back to Chat" at any time.

Key system behaviors:

1. **Life thread prompt injection (thread-level)**
   When creating a thread in the **Life** workspace, the Rust backend injects a **single combined system prompt** (delivery+finance + food+exercise + media + youtube, in that order, separated by `---`) into `thread/start` via `systemPrompt` (the app-server supports this, and the repo docs explicitly call this out).

2. **Dashboards are data-driven, not LLM-driven**
   Each domain dashboard calls new **typed Tauri commands** like `get_delivery_dashboard(range)` which assemble "today" from **local Obsidian** (fast) and "week/month/lifetime" from **Supabase aggregations** (fast).

3. **One rendering pattern reused everywhere**
   The "swap center panel" behavior should follow the same mental model as Git diff:

   * state picks a "center mode"
   * center panel shows the appropriate "layer" (chat vs dashboard; diff remains unchanged)

4. **Mobile/iOS parity**
   iOS gets **native SwiftUI dashboards** (glanceable while driving), powered by the same daemon RPC methods as desktop remote mode.

---

## 2. System Architecture

### 2.1 High-level component diagram

```
┌──────────────────────────────┐
│          React UI            │
│  (Desktop / Tauri WebView)   │
├───────────────┬──────────────┤
│ Left Sidebar  │ Right Panel  │
│ Workspaces    │ Life Domains │
│ Threads       │ Tabs/Controls│
├───────────────┴──────────────┤
│          Center Panel         │
│  Chat Threads  OR  Dashboard  │
└───────────────┬──────────────┘
                │ invoke()
                ▼
┌────────────────────────────────────────┐
│            Rust Backend (Tauri)         │
│  - start_thread() injects Life prompt   │
│  - get_*_dashboard(range)               │
│  - Obsidian parsers + Supabase client   │
└───────────────┬────────────────────────┘
                │ (remote mode)
                ▼
┌────────────────────────────────────────┐
│        Rust Daemon (codex_monitor_daemon)│
│   Same commands + same Life logic        │
└───────────────┬────────────────────────┘
                │
        ┌───────┴────────┐
        ▼                ▼
┌──────────────┐   ┌──────────────┐
│ Obsidian Vault│   │   Supabase    │
│ Stream/Entities│  │ Aggregations  │
└──────────────┘   └──────────────┘
```

### 2.2 Data flow diagrams

#### A) Starting a thread in Life (combined system prompt)

```
User clicks "New Thread" in Life workspace
  → React calls tauri.start_thread(workspaceId)
    → Rust start_thread()
      → detect workspace is Life (WorkspacePurpose == "life")
      → build combined prompt:
           delivery-finance + "---" +
           food-exercise + "---" +
           media + "---" +
           youtube + "---" +
           final instruction block
      → session.send_request("thread/start", { cwd, approvalPolicy, systemPrompt })
    → returns threadId → UI attaches to thread stream
```

#### B) Selecting a domain dashboard

```
User clicks "Delivery" in right panel
  → React sets LifeViewState.activeDomain = "delivery"
  → Center panel renders <DeliveryDashboard />
    → useDeliveryData(range) fires:
      invoke("get_delivery_dashboard", { workspaceId, range })
        → Rust:
          - today: parse Stream + Entities/Delivery/Sessions
          - week/month/lifetime: query Supabase aggregations
      → returns typed DeliveryDashboard JSON
  → Dashboard renders stats + orders table + merchant analysis
```

### 2.3 State management (frontend)

**Existing global state** already includes:

* active workspace
* active thread
* right panel mode
* git diff state

Add a **Life-specific state slice**:

* `lifeView.activeDomain: LifeDomain | null`
* `lifeView.domainState[domain]` (time range, filters, sort, expanded sections)
* persisted to `localStorage` (recommended) so switching workspaces doesn't nuke filters.

**Rule of thumb**:

* Life state is only "active" when `activeWorkspace.settings.purpose === 'life'`.
* Otherwise it's inert.

---

## 3. Type Definitions (TypeScript)

Put these in a new file like:

* `src/features/life/types.ts` (best)
* and re-export from `src/types.ts` only if needed globally

```ts
// src/features/life/types.ts

export type LifeDomain =
  | "delivery"
  | "nutrition"
  | "exercise"
  | "media"
  | "youtube"
  | "finance";

export type LifeTimeRange = "today" | "week" | "month" | "lifetime";
export type SortDirection = "asc" | "desc";

export interface LifeDomainConfig {
  id: LifeDomain;
  label: string;
  icon: string; // emoji
  accentColor: string; // hex or css var name
}

export interface LifeDomainState {
  timeRange: LifeTimeRange;
  filters: Record<string, string>;
  sortBy: string;
  sortDirection: SortDirection;
  expandedSections: Record<string, boolean>;
}

export interface LifeWorkspaceViewState {
  activeDomain: LifeDomain | null;
  domainState: Record<LifeDomain, LifeDomainState>;
}

export interface DashboardMeta {
  domain: LifeDomain;
  range: LifeTimeRange;
  periodStart: string; // YYYY-MM-DD
  periodEnd: string;   // YYYY-MM-DD
  generatedAt: string; // ISO timestamp
  sources: Array<"obsidian" | "supabase">;
  cacheHit?: boolean;
}

// -----------------------------
// Delivery
// -----------------------------

export interface DeliveryStats {
  totalEarnings: number;
  orderCount: number;
  activeHours: number;
  totalMiles?: number;
  hourlyRate: number;
  perMileRate: number;
  avgOrderValue?: number;
}

export interface DeliveryOrder {
  id: string;
  startedAt: string; // ISO timestamp
  merchantName: string;
  payout: number;
  miles?: number;
  durationMinutes?: number;
  platform?: "doordash" | "uber" | "grubhub" | "other";
  notes?: string;
  // Optional "cause→effect" fields:
  rewardTag?: string; // e.g. "Hot zone"
  warningTag?: string; // e.g. "Long wait"
}

export interface MerchantStats {
  merchantName: string;
  orderCount: number;
  totalEarnings: number;
  avgPayout: number;
  avgMiles?: number;
  avgPerMile?: number;
  tier?: "S" | "A" | "B" | "C" | "D";
}

export interface DeliveryDashboard {
  meta: DashboardMeta;
  stats: DeliveryStats;
  orders: DeliveryOrder[];
  topMerchants: MerchantStats[];
}

// -----------------------------
// Nutrition
// -----------------------------

export interface NutritionTargets {
  calories: number;
  protein: number;
  carbs: number;
  fat: number;
  fiber?: number;
}

export interface MealItem {
  name: string;
  calories?: number;
  protein?: number;
  carbs?: number;
  fat?: number;
  fiber?: number;
  imageUrl?: string;
  entityId?: string; // Obsidian entity slug
}

export interface MealEntry {
  id: string;
  eatenAt: string; // ISO timestamp
  label?: "breakfast" | "lunch" | "dinner" | "snack";
  items: MealItem[];
  notes?: string;
  photoUrl?: string;
}

export interface DailyNutrition {
  date: string; // YYYY-MM-DD
  calories: number;
  protein: number;
  carbs: number;
  fat: number;
  fiber?: number;
}

export interface NutritionDashboard {
  meta: DashboardMeta;
  targets: NutritionTargets;
  today: DailyNutrition & { meals: MealEntry[] };
  trend: DailyNutrition[]; // daily points for charting (week/month)
}

// -----------------------------
// Exercise
// -----------------------------

export type ExerciseType = "walk" | "strength" | "run" | "mobility" | "other";

export interface ExerciseEntry {
  id: string;
  startedAt: string; // ISO timestamp
  type: ExerciseType;
  durationMinutes?: number;
  distanceMiles?: number;
  intensity?: "easy" | "moderate" | "hard";
  notes?: string;
}

export interface ExerciseStreak {
  currentDays: number;
  bestDays: number;
  lastWorkoutDate?: string; // YYYY-MM-DD
}

export interface ExerciseDayCell {
  date: string; // YYYY-MM-DD
  didWorkout: boolean;
  types?: ExerciseType[];
}

export interface ExerciseDashboard {
  meta: DashboardMeta;
  streak: ExerciseStreak;
  recentWorkouts: ExerciseEntry[];
  grid: ExerciseDayCell[]; // typically 7/28/35 cells depending on range
}

// -----------------------------
// Media
// -----------------------------

export type MediaType = "film" | "tv" | "game" | "book" | "anime" | "other";
export type MediaStatus = "backlog" | "in_progress" | "completed" | "dropped";

export interface MediaItem {
  id: string;
  title: string;
  type: MediaType;
  status: MediaStatus;
  rating?: number; // 1-10
  coverUrl?: string;
  lastActivityAt?: string; // ISO timestamp
  completedAt?: string; // ISO timestamp
  tags?: string[];
}

export interface MediaStats {
  backlogCount: number;
  inProgressCount: number;
  completedCount: number;
  avgRating?: number;
}

export interface MediaDashboard {
  meta: DashboardMeta;
  stats: MediaStats;
  recentlyActive: MediaItem[];
  byType: Record<MediaType, number>;
}

// -----------------------------
// YouTube
// -----------------------------

export type PipelineStage =
  | "brain_dump"
  | "development"
  | "outline"
  | "evaluation"
  | "script"
  | "edit"
  | "published";

export type IdeaTier = "S" | "A" | "B" | "C";

export interface VideoIdea {
  id: string;
  title: string;
  tier: IdeaTier;
  stage: PipelineStage;
  thesis?: string;
  updatedAt: string; // ISO timestamp
  nextAction?: string;
}

export interface PipelineEvent {
  id: string;
  at: string; // ISO timestamp
  ideaId: string;
  fromStage?: PipelineStage;
  toStage: PipelineStage;
  note?: string;
}

export interface YouTubeDashboard {
  meta: DashboardMeta;
  pipelineStats: Record<PipelineStage, number>;
  sTier: VideoIdea[];
  inProgress: VideoIdea[];
  recentActivity: PipelineEvent[];
}

// -----------------------------
// Finance
// -----------------------------

export type BillStatus = "upcoming" | "paid" | "overdue";

export interface Bill {
  id: string;
  name: string;
  amount: number;
  dueDate: string; // YYYY-MM-DD
  paidDate?: string; // YYYY-MM-DD
  status: BillStatus;
  category?: string;
  autopay?: boolean;
  notes?: string;
}

export interface BillCalendarDay {
  date: string; // YYYY-MM-DD
  bills: Bill[];
  totalDue: number;
  totalPaid: number;
}

export interface FinanceStats {
  monthlyTotal: number;
  paidThisPeriod: number;
  remainingThisPeriod: number;
  overdueTotal?: number;
}

export interface FinanceDashboard {
  meta: DashboardMeta;
  stats: FinanceStats;
  upcomingBills: Bill[]; // next N
  calendar: BillCalendarDay[]; // month grid (or range grid)
}
```

---

## 4. Component Specifications (React)

### 4.1 Component tree (Life workspace)

Add:

```
src/features/life/
├── components/
│   ├── LifeWorkspaceView.tsx
│   ├── DomainSelector.tsx
│   ├── DomainViewContainer.tsx
│   ├── domains/
│   │   ├── DeliveryDashboard.tsx
│   │   ├── NutritionDashboard.tsx
│   │   ├── ExerciseDashboard.tsx
│   │   ├── MediaDashboard.tsx
│   │   ├── YouTubeDashboard.tsx
│   │   └── FinanceDashboard.tsx
│   └── shared/
│       ├── StatCard.tsx
│       ├── TimeRangeSelector.tsx
│       ├── DataTable.tsx
│       ├── CoverImage.tsx
│       ├── ProgressBar.tsx
│       ├── Section.tsx
│       ├── Skeleton.tsx
│       └── EmptyState.tsx
├── hooks/
│   ├── useLifeWorkspace.ts
│   ├── useDomainDashboard.ts
│   ├── useDeliveryData.ts
│   ├── useNutritionData.ts
│   ├── useExerciseData.ts
│   ├── useMediaData.ts
│   ├── useYouTubeData.ts
│   └── useFinanceData.ts
└── styles/
    └── life-dashboard.css
```

### 4.2 Integration point in existing layout

You already build `messagesNode` inside `useLayoutNodes.tsx`.

**Change**: when active workspace is Life and `lifeViewState.activeDomain != null`,
render `<DomainViewContainer />` instead of `<Messages />`.

Pseudo:

```tsx
const isLife = options.activeWorkspace?.settings.purpose === "life";

const messagesNode = isLife ? (
  <LifeWorkspaceView
    activeDomain={options.lifeViewState.activeDomain}
    onBackToChat={options.onLifeBackToChat}
    domainViewNode={
      <DomainViewContainer ... />
    }
    chatNode={
      <Messages ... />
    }
  />
) : (
  <Messages ... />
);
```

And make `composerNode` conditional:

* show composer only when **Life + chat mode** OR non-life.

### 4.3 LifeWorkspaceView.tsx

**Responsibility**

* Owns the "center panel swap"
* Displays either:

  * `chatNode` (existing Messages)
  * `domainViewNode` (DomainViewContainer)

**Props**

* `activeDomain: LifeDomain | null`
* `chatNode: ReactNode`
* `domainViewNode: ReactNode`

**Behavior**

* If `activeDomain !== null`, show `domainViewNode`
* Else show `chatNode`
* Also provides a consistent header area if you want "Back to Chat" anchored

### 4.4 DomainSelector.tsx (right panel)

**Responsibility**

* Renders the domain tabs in the right panel
* Sets active domain
* Shows domain accent + icon
* Optional: mini "today" stats chips below each tab (future)

**Props**

* `domains: LifeDomainConfig[]`
* `activeDomain: LifeDomain | null`
* `onSelectDomain(domain: LifeDomain): void`
* `onBackToChat(): void` (optional shortcut)

**UI**

* Horizontal tabs (or wrap)
* Active tab gets accent underline and subtle glow
* Follows your color semantics:

  * Yellow = section headers
  * Cyan/Teal = labels
  * Green = "good outcome"
  * Red = warnings/costs
  * Grey = timestamps

### 4.5 DomainViewContainer.tsx (center panel dashboard router)

**Responsibility**

* Provides shared dashboard chrome:

  * Back to Chat
  * TimeRangeSelector
  * Optional filter row / search
* Switches to the right dashboard component

**Props**

* `workspaceId: string`
* `domain: LifeDomain`
* `domainState: LifeDomainState`
* setters: `setTimeRange`, `setFilters`, `setSort`, `toggleSection`

**Implementation**

* `switch(domain)` or mapping object to component
* Each dashboard component gets:

  * `workspaceId`
  * `state`
  * `setState(...)`

### 4.6 Dashboard components (each domain)

All dashboards should follow the same "not overwhelming" layout:

1. **Hero stat** (big)
2. **4 supporting stat cards**
3. **Recent activity list** (last 5–10)
4. **Collapsed deep dive** section(s)

#### DeliveryDashboard.tsx

* Hero: "Today's Earnings"
* Cards: orders, $/hr, $/mi, active hours
* Table: last 5 orders (merchant, payout, miles, time)
* Merchant analysis: top 5 (grouped)
* Expand: "All orders", "Tier performance"

#### NutritionDashboard.tsx

* Hero: calories remaining (or protein remaining)
* Cards: protein/carbs/fat/fiber
* Meal log list w/ timestamps and optional photos
* Trend: week bars/lines (use existing chart approach if any; otherwise simple stacked list first)
* Expand: frequent foods, macro breakdown

#### ExerciseDashboard.tsx

* Hero: streak
* Cards: workouts this week, minutes, distance, last workout
* Weekly grid (7×N) "habit heatmap"
* Recent workouts list
* Expand: PRs / notes

#### MediaDashboard.tsx

* Hero: "Recently watched/played"
* Shelf row of covers (CoverImage)
* Type filter pills
* Recent list with rating color semantics
* Expand: backlog view

#### YouTubeDashboard.tsx

* Hero: S-tier count in pipeline
* Stage stats cards
* Lists: S-tier ideas, in-progress
* Recent activity timeline (stage changes)
* Expand: full pipeline board

#### FinanceDashboard.tsx

* Hero: "Remaining this month"
* Cards: total, paid, remaining, overdue
* Upcoming bills list (next 7 days)
* Calendar grid (month)
* Expand: payment history

### 4.7 Styling (life-dashboard.css)

Add a subtle grid background only for Life dashboards:

* `background-image: linear-gradient(...)` with very low alpha
* card backgrounds use existing dark palette and border tokens
* use CSS variables:

  * `--life-accent` set per active domain
  * reuse `--domain-accent` if you want consistency

---

## 5. Backend Specifications (Rust + Supabase)

### 5.1 Rust module layout

Add:

* `src-tauri/src/life.rs` (Tauri commands)
* `src-tauri/src/life/service.rs` (pure logic: prompt building + dashboard builders)
* `src-tauri/src/life/types.rs` (dashboard structs)

Reuse existing:

* `src-tauri/src/obsidian/mod.rs` loaders (sessions, bills, food, media, youtube)
* `src-tauri/src/memory/supabase.rs` patterns for Supabase REST calls (or extract a shared Supabase client)

### 5.2 New Tauri commands

Add to `src-tauri/src/lib.rs` invoke handler:

```rust
get_life_workspace_prompt,
get_delivery_dashboard,
get_nutrition_dashboard,
get_exercise_dashboard,
get_media_dashboard,
get_youtube_dashboard,
get_finance_dashboard,
```

Command signatures (example):

```rust
#[tauri::command]
pub async fn get_life_workspace_prompt(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String>;

#[tauri::command]
pub async fn get_delivery_dashboard(
    workspace_id: String,
    range: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<DeliveryDashboard, String>;
```

**Remote mode pattern**
Follow `domains.rs` and `workspaces.rs` style:

* if remote: `remote_backend::call_remote(... "get_delivery_dashboard", json!({...}))`
* else compute locally.

### 5.3 Daemon changes

In `src-tauri/src/bin/codex_monitor_daemon.rs`:

1. Implement handler methods mirroring Tauri commands:

   * `get_life_workspace_prompt`
   * `get_*_dashboard`

2. Add method strings to the big dispatcher `match method.as_str()`.

This keeps desktop(remote) + iOS fully aligned.

### 5.4 Thread-level prompt injection (core requirement)

Modify both:

* `src-tauri/src/codex.rs::start_thread`
* daemon `start_thread(...)`

Current start thread sends:

```json
{ "cwd": "...", "approvalPolicy": "..." }
```

**Change**: if workspace is Life, include:

```json
{ "cwd": "...", "approvalPolicy": "...", "systemPrompt": "<combined>" }
```

How to detect Life workspace:

* recommended: add a workspace settings field (see below), or a dedicated "Life workspace id" stored in settings.

Also modify `send_user_message` logic:

* For Life workspace: **do not** add `domain_instructions` per-turn (avoid re-sending an 800+ line prompt every message).

### 5.5 Workspace typing (recommended minimal change)

Add to `WorkspaceSettings`:

* `purpose: Option<WorkspacePurpose>` where `WorkspacePurpose = Life | Coding`

Rust:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspacePurpose {
    Coding,
    Life,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
    // existing...
    pub purpose: Option<WorkspacePurpose>,
    // optional: pub display_name: Option<String>,
}
```

TS + Swift models must match.

### 5.6 get_life_workspace_prompt implementation detail

Best practical source of truth in *this codebase*:

* your existing **Domain** system prompts are seeded from the prompt files and persisted in `domains.json`.

So `get_life_workspace_prompt()` should:

1. load domains from storage
2. locate the 4 "prompt domains" by id:

   * `delivery_finance`
   * `food_exercise`
   * `media`
   * `youtube`
3. concatenate in a stable order with `\n---\n`
4. append the "life assistant" tail:

```
You are JMWillis's life assistant with full context across all domains.
Auto-detect the domain from user messages and respond appropriately.
```

Fallback if they don't exist:

* read the four files from known paths (or return a clear error telling the user which domains/paths are missing)

### 5.7 Dashboard command behavior

**Common contract**:

* inputs: `workspace_id`, `range`
* outputs: typed dashboard struct with:

  * `meta` (period start/end, computedAt, sources)
  * stats + lists

**Today**

* Obsidian parsing only (Stream + Entities) for freshness

**Week/Month/Lifetime**

* Supabase aggregation row(s) for performance
* optionally supplement with Obsidian for "recent list" items

### 5.8 Supabase schema (aggregations)

You already have memory tables/functions; add life-domain aggregation tables.

I'd keep your "period + start/end" format, and add indexes:

```sql
-- Delivery
create table if not exists delivery_aggregations (
  id uuid primary key default gen_random_uuid(),
  period text not null check (period in ('day','week','month','lifetime')),
  period_start date not null,
  period_end date not null,

  total_earnings numeric,
  order_count int,
  total_hours numeric,
  total_miles numeric,
  hourly_rate numeric,
  per_mile_rate numeric,

  computed_at timestamptz not null default now()
);

create index if not exists delivery_agg_period_idx
  on delivery_aggregations (period, period_start desc);

-- Nutrition
create table if not exists nutrition_aggregations (
  id uuid primary key default gen_random_uuid(),
  period text not null check (period in ('day','week','month','lifetime')),
  period_start date not null,
  period_end date not null,

  avg_calories numeric,
  avg_protein numeric,
  avg_carbs numeric,
  avg_fat numeric,
  avg_fiber numeric,
  meals_logged int,

  computed_at timestamptz not null default now()
);

create index if not exists nutrition_agg_period_idx
  on nutrition_aggregations (period, period_start desc);

-- Exercise
create table if not exists exercise_aggregations (
  id uuid primary key default gen_random_uuid(),
  period text not null check (period in ('day','week','month','lifetime')),
  period_start date not null,
  period_end date not null,

  workout_days int,
  workout_count int,
  total_minutes numeric,
  total_distance numeric,

  computed_at timestamptz not null default now()
);

create index if not exists exercise_agg_period_idx
  on exercise_aggregations (period, period_start desc);

-- Media
create table if not exists media_aggregations (
  id uuid primary key default gen_random_uuid(),
  period text not null check (period in ('day','week','month','lifetime')),
  period_start date not null,
  period_end date not null,

  completed_count int,
  in_progress_count int,
  backlog_count int,
  avg_rating numeric,

  by_type jsonb, -- {"film": 12, "tv": 4, ...}

  computed_at timestamptz not null default now()
);

create index if not exists media_agg_period_idx
  on media_aggregations (period, period_start desc);

-- YouTube
create table if not exists youtube_aggregations (
  id uuid primary key default gen_random_uuid(),
  period text not null check (period in ('day','week','month','lifetime')),
  period_start date not null,
  period_end date not null,

  pipeline_counts jsonb, -- {"brain_dump": 10, "development": 3, ...}
  s_tier_count int,
  in_progress_count int,

  computed_at timestamptz not null default now()
);

create index if not exists youtube_agg_period_idx
  on youtube_aggregations (period, period_start desc);

-- Finance
create table if not exists finance_aggregations (
  id uuid primary key default gen_random_uuid(),
  period text not null check (period in ('day','week','month','lifetime')),
  period_start date not null,
  period_end date not null,

  total_due numeric,
  total_paid numeric,
  remaining numeric,
  overdue numeric,

  computed_at timestamptz not null default now()
);

create index if not exists finance_agg_period_idx
  on finance_aggregations (period, period_start desc);
```

**Cron / scheduler**

* run daily (or hourly) to recompute "day/week/month" for the current period and rollups
* store a single "lifetime" row that updates daily

**RLS**

* if this is single-user, simplest is to disable RLS for these tables or use a service role from the daemon (preferred)
* if you keep anon key, you need permissive read policies for your client

---

## 6. iOS Specifications (SwiftUI + CodexStore)

### 6.1 SwiftUI view hierarchy

Add:

```
ios/CodexMonitorMobile/CodexMonitorMobile/Views/
├── LifeWorkspaceView.swift
├── DomainTabBar.swift
├── domains/
│   ├── DeliveryDashboardView.swift
│   ├── NutritionDashboardView.swift
│   ├── ExerciseDashboardView.swift
│   ├── MediaDashboardView.swift
│   ├── YouTubeDashboardView.swift
│   └── FinanceDashboardView.swift
└── shared/
    ├── StatCardView.swift
    ├── TimeRangePicker.swift
    ├── CoverImageView.swift
    ├── DashboardSection.swift
    └── LoadingSkeletonView.swift
```

### 6.2 Recommended navigation on iOS

You already have:

* Phone: TabView (Projects / Domain / Codex / Memory / Git / …)
* Tablet: NavigationSplitView + segmented detail picker

**Best alignment with your target UX**:

* Keep "Projects" as workspace picker
* When selected workspace is Life:

  * In the **Codex** detail/tab, show **LifeWorkspaceView** instead of plain Conversation UI
  * LifeWorkspaceView contains:

    * DomainTabBar
    * "Chat" vs "Dashboard" toggle behavior identical to desktop (Back to Chat)

This avoids inventing a whole new top-level tab and makes Life behave like "a workspace with special center content".

### 6.3 CodexStore additions

Add state:

```swift
@Published var lifeActiveDomain: LifeDomain? = nil
@Published var lifeTimeRange: LifeTimeRange = .today

@Published var deliveryDashboard: DeliveryDashboard?
@Published var nutritionDashboard: NutritionDashboard?
@Published var exerciseDashboard: ExerciseDashboard?
@Published var mediaDashboard: MediaDashboard?
@Published var youtubeDashboard: YouTubeDashboard?
@Published var financeDashboard: FinanceDashboard?

@Published var dashboardLoading: Bool = false
@Published var dashboardError: String? = nil
```

Add methods:

```swift
func fetchDeliveryDashboard(range: LifeTimeRange) async
func fetchNutritionDashboard(range: LifeTimeRange) async
func fetchExerciseDashboard(range: LifeTimeRange) async
func fetchMediaDashboard(range: LifeTimeRange) async
func fetchYouTubeDashboard(range: LifeTimeRange) async
func fetchFinanceDashboard(range: LifeTimeRange) async
```

All methods call daemon RPC like you already do for `domain_trends`.

### 6.4 Swift models

In:

* `ios/Packages/CodexMonitorRPC/Sources/CodexMonitorModels/Models.swift`

Add `Codable` structs mirroring the TS/Rust dashboard structs.

Keep them flat and pragmatic; SwiftUI likes value types.

---

## 7. Implementation Plan (Phased)

### Phase 0 — Foundation (no major UI changes)

1. Add `WorkspacePurpose` to WorkspaceSettings across Rust + TS + Swift.
2. Implement `get_life_workspace_prompt` in:

   * Tauri (local)
   * Daemon
3. Modify `start_thread` (Tauri + Daemon):

   * if workspace purpose == Life → inject `systemPrompt`
4. Modify `send_user_message` (Tauri + Daemon):

   * skip per-turn domain injection for Life
5. Add stub dashboard commands returning "not implemented yet" or empty structs:

   * get_delivery_dashboard, etc.
6. Add Supabase tables + indexes.

**Done = you can start Life threads and confirm system prompt injection works.**

### Phase 1 — Delivery Dashboard proof-of-concept

1. React:

   * Add `src/features/life/` folder
   * Add Life state hook and DomainSelector in right panel (Life only)
   * Add center swap: chat ↔ delivery dashboard
2. Rust:

   * Implement `get_delivery_dashboard(range)`:

     * today: parse delivery sessions + derive order list (initially from sessions or stream)
     * week/month/lifetime: supabase row
3. iOS:

   * Create DeliveryDashboardView + TimeRangePicker
   * Wire store method

### Phase 2 — Second domain (Media or YouTube)

Pick one with strong visuals (I'd do **Media** first because covers are straightforward).

* Build the dashboard
* Extract shared components (StatCard, CoverImage)
* Lock down the dashboard response "meta" pattern

### Phase 3 — Remaining domains

* Nutrition
* Exercise
* Finance
* whichever of Media/YouTube wasn't Phase 2

### Phase 4 — Polish

* caching:

  * Rust TTL cache per (domain, range)
  * file watcher invalidation for Stream/Entities (desktop/daemon)
* skeleton loading states everywhere
* error states and "retry"
* filter persistence (localStorage + iOS UserDefaults)
* optional: domain auto-detect suggestions ("Looks like Delivery — open dashboard?")

---

## 8. Test Plan

### 8.1 Unit tests (TS)

* `detectLifeDomainFromMessage(text)` (keyword-based)
* reducers/state transitions:

  * selecting a domain sets activeDomain
  * back clears it
  * timeRange persists per domain

### 8.2 Hook tests (TS)

* `useDeliveryData()` default range today
* changing range triggers refetch
* cache behavior (if you add caching at hook layer)

### 8.3 Integration tests (React)

* Life workspace:

  * clicking Delivery swaps center panel
  * back returns to chat panel
  * composer hidden while dashboard visible
* prompt injection:

  * starting a thread in Life causes backend to include combined prompt

    * (you can test by adding a debug endpoint or storing last thread/start params in dev builds)

### 8.4 Rust tests

* Prompt builder:

  * stable ordering
  * separators
  * handles missing domains gracefully
* Obsidian parsing for dashboard builders (feed fixture markdown files)
* Supabase query wrapper tests (mock HTTP)

### 8.5 E2E (Playwright or equivalent)

* Start Life thread → send "had eggs" → verify response
* Switch to Nutrition dashboard → verify today macros reflect newest entries (once implemented)

---

## 9. Risk Assessment + Mitigations

### Risk: Prompt source fragility (file paths / missing domains)

* **Mitigation**: build Life prompt from `domains.json` first; only fall back to reading external files if needed; surface a clear "missing prompt domains" error.

### Risk: Token bloat if prompts injected per turn

* **Mitigation**: for Life, inject at thread start only, and explicitly skip per-turn domain injection.

### Risk: Obsidian data format drift

* **Mitigation**: treat dashboard parsing as "best effort":

  * strict YAML decoding where possible
  * tolerant parsing for stream body lines
  * add fixtures and tests

### Risk: Supabase stale data

* **Mitigation**: hybrid strategy:

  * "today" always local
  * show `computedAt` in meta
  * allow "Refresh" button that triggers on-demand recompute later (Phase 4)

### Risk: Cross-platform type drift (Rust/TS/Swift)

* **Mitigation**: keep dashboard structs in one canonical Rust module and:

  * generate JSON fixtures for TS/Swift tests
  * enforce stable naming (camelCase) with serde attributes

---

## 10. Open Questions (with recommended defaults)

1. **Domain navigation: tabs vs routes?**
   **Recommendation:** tabs/state (no routing).
   Reason: this is a desktop app; you already use internal UI state (like Git diff). Add keyboard shortcuts:

   * `Esc` → Back to Chat
   * `Cmd+1..6` → switch domains

2. **Hybrid data strategy (Obsidian vs Supabase):**
   **Recommendation:**

   * Today: Obsidian always
   * Week/Month/Lifetime: Supabase primary, with Obsidian fallback if Supabase unavailable
     Add `meta.sources` so UI can reflect what was used.

3. **Image handling (covers / food photos):**
   **Recommendation:** store URLs in entity files when known, cache downloaded images locally by URL hash for fast rendering.
   External API lookups can happen in the daemon and write back to entity metadata later.

4. **Filter persistence location:**
   **Recommendation:** local persistence (localStorage on desktop, UserDefaults on iOS), keyed by domain + workspace id.
   Keeps UX stable without needing backend schema.

5. **iOS parity:**
   **Recommendation:** native SwiftUI with "80% parity" first.
   WebView adds complexity and usually feels off compared to your existing native UI.
