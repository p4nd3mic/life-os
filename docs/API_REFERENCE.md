# Life OS API Reference

**Snapshot analyzed:** `life-os-codebase-20260202`  
**This doc covers:**
- Tauri command surface (all registered `invoke(...)` targets)
- Codex app-server RPC methods used by the desktop app
- life-mcp tool surface (tools, high-frequency subset, meta tools)
- Key React hooks in the Life OS UI

> Naming note: Rust uses `snake_case` identifiers; the JS side often passes `camelCase` keys to `invoke`. Tauri’s argument deserialization handles this mapping in practice, but it’s easy to get tripped up when adding new commands or calling them from scripts.

---

## Tauri Commands (invoke targets)

Source of truth: `CodexMonitor-lifeos/src-tauri/src/lib.rs` → `tauri::generate_handler![...]`.

### Event channels (important)
- `life_stream_event` — emitted by Life Stream backend and consumed by `useLifeStream`
- `app_server_event` — emitted for Codex app-server streaming events
- `open_file` — emitted for file-open actions

### Command tables by module

### `life_stream` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `life_stream_load_day` | workspace_id: String, date_iso: String | `Result<Vec<StreamCard>, String>` | `life_stream/mod.rs` |
| `life_stream_submit` | workspace_id: String, card_id: String, input: String, occurred_at_iso: Option<String>, model: Option<String>, effort: Option<String>, access_mode: Option<String>, collaboration_mode: Option<Value> | `Result<(), String>` | `life_stream/mod.rs` |
| `life_stream_cancel` | workspace_id: String, card_id: String | `Result<(), String>` | `life_stream/mod.rs` |
| `life_stream_retry` | workspace_id: String, card_id: String | `Result<(), String>` | `life_stream/mod.rs` |
| `life_stream_clarify` | workspace_id: String, card_id: String, option_id: String | `Result<(), String>` | `life_stream/mod.rs` |
| `life_stream_task_dock_load` | workspace_id: String | `Result<TaskDockPayload, String>` | `life_stream/mod.rs` |
| `life_stream_task_dock_save` | workspace_id: String, payload: TaskDockPayload | `Result<(), String>` | `life_stream/mod.rs` |
| `life_stream_restructure` | workspace_id: String, card_id: String, action: CausalRestructureAction, source_node_ids: Option<Vec<String>>, target_mode: Option<String> | `Result<CausalRestructureResult, String>` | `life_stream/mod.rs` |
| `life_stream_regenerate_semantics` | workspace_id: String, card_ids: Vec<String>, force_llm: Option<bool>, persist: Option<bool> | `Result<SemanticRegenerationResult, String>` | `life_stream/mod.rs` |
| `life_stream_image_candidates` | workspace_id: String, card_id: String, node_id: Option<String> | `Result<ImageCandidateResponse, String>` | `life_stream/mod.rs` |
| `life_stream_image_attach` | workspace_id: String, card_id: String, node_id: Option<String>, source_path: String, set_primary: Option<bool>, set_context_override: Option<bool>, context_hint: Option<String>, update_entity_file: Option<bool>, update_entity_embed: Option<bool> | `Result<ImageAttachResult, String>` | `life_stream/mod.rs` |
| `life_stream_image_backfill` | workspace_id: String, update_embed_block: Option<bool> | `Result<ImageBackfillSummary, String>` | `life_stream/mod.rs` |
| `life_stream_image_autofetch` | workspace_id: String, card_ids: Option<Vec<String>>, mode: Option<ImageAutoFetchMode>, update_entity_file: Option<bool>, update_entity_embed: Option<bool> | `Result<ImageAutoFetchSummary, String>` | `life_stream/mod.rs` |
| `life_stream_read_log` | workspace_id: String, limit: Option<u32> | `Result<Vec<String>, String>` | `life_stream/mod.rs` |

### `life` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `get_life_workspace_prompt` | — | `Result<String, String>` | `life.rs` |
| `get_delivery_dashboard` | workspace_id: String, range: String | `Result<DeliveryDashboard, String>` | `life.rs` |
| `get_nutrition_dashboard` | workspace_id: String, range: String | `Result<NutritionDashboard, String>` | `life.rs` |
| `get_exercise_dashboard` | workspace_id: String, range: String | `Result<ExerciseDashboard, String>` | `life.rs` |
| `get_media_dashboard` | workspace_id: String | `Result<MediaLibrary, String>` | `life.rs` |
| `get_youtube_dashboard` | workspace_id: String | `Result<YouTubeLibrary, String>` | `life.rs` |
| `enrich_media_covers` | workspace_id: String, force: Option<bool> | `Result<MediaCoverSummary, String>` | `life.rs` |
| `get_finance_dashboard` | workspace_id: String, range: String | `Result<FinanceDashboard, String>` | `life.rs` |

### `codex` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `codex_doctor` | codex_bin: Option<String> | `Result<Value, String>` | `codex.rs` |
| `start_thread` | workspace_id: String | `Result<Value, String>` | `codex.rs` |
| `send_user_message` | workspace_id: String, thread_id: String, text: String, model: Option<String>, effort: Option<String>, access_mode: Option<String>, images: Option<Vec<String>>, collaboration_mode: Option<Value> | `Result<Value, String>` | `codex.rs` |
| `turn_interrupt` | workspace_id: String, thread_id: String, turn_id: String | `Result<Value, String>` | `codex.rs` |
| `start_review` | workspace_id: String, thread_id: String, target: Value, delivery: Option<String> | `Result<Value, String>` | `codex.rs` |
| `respond_to_server_request` | workspace_id: String, request_id: Value, result: Value | `Result<(), String>` | `codex.rs` |
| `remember_approval_rule` | workspace_id: String, command: Vec<String> | `Result<Value, String>` | `codex.rs` |
| `get_commit_message_prompt` | workspace_id: String | `Result<String, String>` | `codex.rs` |
| `generate_commit_message` | workspace_id: String | `Result<String, String>` | `codex.rs` |
| `resume_thread` | workspace_id: String, thread_id: String | `Result<Value, String>` | `codex.rs` |
| `list_threads` | workspace_id: String, cursor: Option<String>, limit: Option<u32> | `Result<Value, String>` | `codex.rs` |
| `list_session_threads` | workspace_path: String, limit: Option<usize> | `Result<Value, String>` | `codex.rs` |
| `archive_thread` | workspace_id: String, thread_id: String | `Result<Value, String>` | `codex.rs` |
| `collaboration_mode_list` | workspace_id: String | `Result<Value, String>` | `codex.rs` |
| `model_list` | workspace_id: String | `Result<Value, String>` | `codex.rs` |
| `account_rate_limits` | workspace_id: String | `Result<Value, String>` | `codex.rs` |
| `skills_list` | workspace_id: String | `Result<Value, String>` | `codex.rs` |

### `workspaces` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `list_workspaces` | — | `Result<Vec<WorkspaceInfo>, String>` | `workspaces.rs` |
| `is_workspace_path_dir` | path: String | `Result<bool, String>` | `workspaces.rs` |
| `add_workspace` | path: String, codex_bin: Option<String> | `Result<WorkspaceInfo, String>` | `workspaces.rs` |
| `add_clone` | source_workspace_id: String, copy_name: String, copies_folder: String | `Result<WorkspaceInfo, String>` | `workspaces.rs` |
| `add_worktree` | parent_id: String, branch: String | `Result<WorkspaceInfo, String>` | `workspaces.rs` |
| `remove_workspace` | id: String | `Result<(), String>` | `workspaces.rs` |
| `remove_worktree` | id: String | `Result<(), String>` | `workspaces.rs` |
| `rename_worktree` | id: String, branch: String | `Result<WorkspaceInfo, String>` | `workspaces.rs` |
| `rename_worktree_upstream` | id: String, old_branch: String, new_branch: String | `Result<(), String>` | `workspaces.rs` |
| `apply_worktree_changes` | workspace_id: String | `Result<(), String>` | `workspaces.rs` |
| `update_workspace_settings` | id: String, settings: WorkspaceSettings | `Result<WorkspaceInfo, String>` | `workspaces.rs` |
| `update_workspace_codex_bin` | id: String, codex_bin: Option<String> | `Result<WorkspaceInfo, String>` | `workspaces.rs` |
| `connect_workspace` | id: String | `Result<(), String>` | `workspaces.rs` |
| `list_workspace_files` | workspace_id: String | `Result<Vec<String>, String>` | `workspaces.rs` |
| `read_workspace_file` | workspace_id: String, path: String | `Result<WorkspaceFileResponse, String>` | `workspaces.rs` |
| `open_workspace_in` | path: String, app: String | `Result<(), String>` | `workspaces.rs` |

### `terminal` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `terminal_open` | workspace_id: String, terminal_id: String, cols: u16, rows: u16 | `Result<TerminalSessionInfo, String>` | `terminal.rs` |
| `terminal_write` | workspace_id: String, terminal_id: String, data: String | `Result<(), String>` | `terminal.rs` |
| `terminal_resize` | workspace_id: String, terminal_id: String, cols: u16, rows: u16 | `Result<(), String>` | `terminal.rs` |
| `terminal_close` | workspace_id: String, terminal_id: String | `Result<(), String>` | `terminal.rs` |

### `settings` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `get_app_settings` | — | `Result<AppSettings, String>` | `settings.rs` |
| `update_app_settings` | settings: AppSettings | `Result<AppSettings, String>` | `settings.rs` |

### `domains` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `domains_list` | — | `Result<Vec<Domain>, String>` | `domains.rs` |
| `domains_create` | mut domain: Domain | `Result<Domain, String>` | `domains.rs` |
| `domains_update` | domain: Domain | `Result<Domain, String>` | `domains.rs` |
| `domains_delete` | domain_id: String | `Result<(), String>` | `domains.rs` |
| `domain_trends` | workspace_id: String, domain_id: String, range: String | `Result<DomainTrendSnapshot, String>` | `domains.rs` |
| `read_text_file` | path: String | `Result<String, String>` | `domains.rs` |

### `git` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `get_git_status` | workspace_id: String | `Result<serde_json::Value, String>` | `git.rs` |
| `list_git_roots` | workspace_id: String, depth: Option<usize> | `Result<Vec<String>, String>` | `git.rs` |
| `get_git_diffs` | workspace_id: String | `Result<Vec<GitFileDiff>, String>` | `git.rs` |
| `get_git_log` | workspace_id: String, limit: Option<usize> | `Result<GitLogResponse, String>` | `git.rs` |
| `get_git_commit_diff` | workspace_id: String, sha: String | `Result<Vec<GitCommitDiff>, String>` | `git.rs` |
| `get_git_remote` | workspace_id: String | `Result<Option<String>, String>` | `git.rs` |
| `stage_git_file` | workspace_id: String, path: String | `Result<(), String>` | `git.rs` |
| `stage_git_all` | workspace_id: String | `Result<(), String>` | `git.rs` |
| `unstage_git_file` | workspace_id: String, path: String | `Result<(), String>` | `git.rs` |
| `revert_git_file` | workspace_id: String, path: String | `Result<(), String>` | `git.rs` |
| `revert_git_all` | workspace_id: String | `Result<(), String>` | `git.rs` |
| `commit_git` | workspace_id: String, message: String | `Result<(), String>` | `git.rs` |
| `push_git` | workspace_id: String | `Result<(), String>` | `git.rs` |
| `pull_git` | workspace_id: String | `Result<(), String>` | `git.rs` |
| `sync_git` | workspace_id: String | `Result<(), String>` | `git.rs` |
| `get_github_issues` | workspace_id: String | `Result<GitHubIssuesResponse, String>` | `git.rs` |
| `get_github_pull_requests` | workspace_id: String | `Result<GitHubPullRequestsResponse, String>` | `git.rs` |
| `get_github_pull_request_diff` | workspace_id: String, pr_number: u64 | `Result<Vec<GitHubPullRequestDiff>, String>` | `git.rs` |
| `get_github_pull_request_comments` | workspace_id: String, pr_number: u64 | `Result<Vec<GitHubPullRequestComment>, String>` | `git.rs` |
| `list_git_branches` | workspace_id: String | `Result<serde_json::Value, String>` | `git.rs` |
| `checkout_git_branch` | workspace_id: String, name: String | `Result<(), String>` | `git.rs` |
| `create_git_branch` | workspace_id: String, name: String | `Result<(), String>` | `git.rs` |

### `menu` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `menu_set_accelerators` | updates: Vec<MenuAcceleratorUpdate> | `Result<(), String>` | `menu.rs` |

### `dictation` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `dictation_model_status` | — | `Result<DictationModelStatus, String>` | `dictation.rs` |
| `dictation_download_model` | — | `Result<DictationModelStatus, String>` | `dictation.rs` |
| `dictation_cancel_download` | — | `Result<DictationModelStatus, String>` | `dictation.rs` |
| `dictation_remove_model` | — | `Result<DictationModelStatus, String>` | `dictation.rs` |
| `dictation_start` | preferred_language: Option<String> | `Result<DictationSessionState, String>` | `dictation.rs` |
| `dictation_stop` | — | `Result<DictationSessionState, String>` | `dictation.rs` |
| `dictation_cancel` | — | `Result<DictationSessionState, String>` | `dictation.rs` |

### `local_usage` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `local_usage_snapshot` | days: Option<u32>, workspace_path: Option<String> | `Result<LocalUsageSnapshot, String>` | `local_usage.rs` |

### `files` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `read_global_agents_md` | — | `Result<TextFileResponse, String>` | `files.rs` |
| `write_global_agents_md` | content: String | `Result<(), String>` | `files.rs` |
| `read_global_config_toml` | — | `Result<TextFileResponse, String>` | `files.rs` |
| `write_global_config_toml` | content: String | `Result<(), String>` | `files.rs` |

### `memory_commands` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `memory_status` | — | `Result<MemoryStatus, String>` | `memory_commands.rs` |
| `memory_search` | query: String, limit: Option<usize> | `Result<Vec<MemorySearchResult>, String>` | `memory_commands.rs` |
| `memory_append` | memory_type: String, content: String, tags: Vec<String>, workspace_id: Option<String> | `Result<MemoryEntry, String>` | `memory_commands.rs` |
| `memory_bootstrap` | — | `Result<Vec<MemorySearchResult>, String>` | `memory_commands.rs` |
| `memory_flush_now` | workspace_id: String, thread_id: String, force: Option<bool> | `Result<serde_json::Value, String>` | `memory_commands.rs` |

### `prompts` module

| Command (invoke target) | JS args (excluding injected State/AppHandle) | Rust return type | Source |
| --- | --- | --- | --- |
| `prompts_list` | — | `Result<Vec<CustomPromptEntry>, String>` | `prompts.rs` |
| `prompts_create` | — | `Result<CustomPromptEntry, String>` | `prompts.rs` |
| `prompts_update` | — | `Result<CustomPromptEntry, String>` | `prompts.rs` |
| `prompts_delete` | — | `Result<(), String>` | `prompts.rs` |
| `prompts_move` | — | `Result<CustomPromptEntry, String>` | `prompts.rs` |
| `prompts_workspace_dir` | — | `Result<String, String>` | `prompts.rs` |
| `prompts_global_dir` | — | `Result<String, String>` | `prompts.rs` |


---

## Codex app-server RPC methods

These are the JSONL (JSON‑RPC‑lite) methods the Rust backend calls via `WorkspaceSession.send_request(...)` and `send_notification(...)`.

**Handshake rule:** `initialize` **must** be sent once before any other request. The app enforces this and sends `initialized` after a successful response.

### Requests (client → app-server)

- `account/rateLimits/read`
- `collaborationMode/list`
- `initialize`
- `model/list`
- `review/start`
- `skills/config/write`
- `skills/list`
- `thread/archive`
- `thread/list`
- `thread/resume`
- `thread/start`
- `turn/interrupt`
- `turn/start`

### Notifications (client → app-server)

- `initialized`

### App-server → client events you must handle

The app-server streams updates as event-like messages (not requests). These are the core lifecycle events:

**Thread lifecycle**
- `thread/started`
- `thread/completed`

**Turn lifecycle**
- `turn/started`
- `turn/completed`
- `turn/error`
- `turn/plan/updated`
- `turn/diff/updated`

**Item lifecycle**
- `item/started`
- `item/*/delta` (streaming deltas; e.g. `item/agentMessage/delta`)
- `item/completed`

**User input / approvals**
- `item/tool/requestUserInput`
- `workspace/requestApproval` (or similar `*/requestApproval`)

**Client‑side debug events emitted by CodexMonitor**
- `codex/version`
- `codex/capabilities`

---

## life-mcp MCP tool surface

### What actually gets registered in MCP stdio mode

In `life-mcp/src/server/mcp.js`, the MCP server registers:
1) **Meta tools** (always): `list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`  
2) **High-frequency tools** (subset): tools returned by `getHighFrequencyTools()`

So a plain MCP client calling `tools/list` will only see **meta + high-frequency** tools.

> If you need to call *non-registered* tools over MCP, you can route through the meta tool `execute_tool` (it can execute any tool in the registry by name).  
> The Rust Life Stream bridge does **not** use `execute_tool` — it calls tools directly by name and therefore must whitelist tools that are *actually registered*.

### High-frequency tools (Node registry)

From `life-mcp/src/tool-registry.js` (`HIGH_FREQUENCY_TOOL_NAMES`):

- `advise_order`
- `log_meal_quick`
- `add_delivery`
- `delivery_bulk_add`
- `start_session_manual`
- `get_session_context`
- `set_current_ar`
- `agent_status`
- `log_activity`
- `log_reward`
- `reward_status`
- `reward_rules`
- `note_add`
- `note_list`
- `note_search`
- `note_explore`
- `note_delete`
- `note_links`
- `knowledge_browse`
- `knowledge_search`
- `knowledge_status`

---

## life-mcp Tools Reference (by module)

**Parsed from:** `life-mcp/src/tools/*.js`  
**Total tools found in this snapshot:** **147**

HF legend: ⭐ = in `HIGH_FREQUENCY_TOOL_NAMES`

### Meta / Registry Tools (4)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `describe_tool` | `describe_tool(name)` |  | name | — | Get parameter details for a specific tool. |
| `execute_tool` | `execute_tool(name, params?)` |  | name | params | Execute a tool by name with parameters (use after describe_tool). |
| `list_categories` | `list_categories()` |  | — | — | List all available tool categories and their purposes. |
| `search_tools` | `search_tools(query?, category?)` |  | — | query, category | Search for tools by keyword or category. Use this first to discover tools. |

### Delivery Session Tools (11)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `add_delivery` | `add_delivery(app, merchant, status, session_id?, timestamp_offered?, merchant_tier?, zone_pickup?, zone_dropoff?, quoted_pay?, final_pay?, listed_miles?, real_miles?, deadhead_miles?, order_type?, stack_size?, stack_id?, decision_type?, decision_reasoning?, ar_at_decision?, whale_window_active?, promo_active?, wait_time_mins?, total_time_mins?, position_after?, chained_well?, what_came_next?, notes?)` | ⭐ | app, merchant, status | session_id, timestamp_offered, merchant_tier, zone_pickup, zone_dropoff, quoted_pay, final_pay, listed_miles, real_miles, deadhead_miles, order_type, stack_size, stack_id, decision_type, decision_reasoning, ar_at_decision, whale_window_active, promo_active, wait_time_mins, total_time_mins, position_after, chained_well, what_came_next, notes | Log a delivery to the current session |
| `delivery_bulk_add` | `delivery_bulk_add(session_id?, deliveries?)` | ⭐ | — | session_id, deliveries | Bulk log deliveries to a session (preserves input order) |
| `end_session` | `end_session(session_id?, actual?, ending_ar?, whale_catches?, strategic_notes?)` |  | — | session_id, actual, ending_ar, whale_catches, strategic_notes | Close the current session with final stats |
| `generate_session_report` | `generate_session_report(session_id?)` |  | — | session_id | Generate comprehensive end-of-session report with order breakdown, merchant analysis, zone analysis, and performance metrics |
| `get_day_analysis` | `get_day_analysis(day_of_week, shift?, app?, zone?, date_from?, date_to?, limit?)` |  | day_of_week | shift, app, zone, date_from, date_to, limit | Get aggregated historical patterns for a day/shift combination. Uses fallback logic if not enough matching sessions. |
| `get_deliveries` | `get_deliveries(session_id?, date?, app?, status?, merchant?)` |  | — | session_id, date, app, status, merchant | Query deliveries with optional filters (returns raw data - prefer get_day_analysis for summaries) |
| `get_recommendations` | `get_recommendations(day_of_week, shift, starting_ar?)` |  | day_of_week, shift | starting_ar | Get predictions and strategy tips for an upcoming session based on historical data |
| `get_session` | `get_session(session_id?)` |  | — | session_id | Get a session with all its deliveries |
| `get_session_stats` | `get_session_stats(session_id?)` |  | — | session_id | Get aggregated statistics for a single session (compact summary instead of raw deliveries) |
| `start_session` | `start_session(day_type?, promo_windows?, starting_ar?, target?, strategic_notes?)` |  | — | day_type, promo_windows, starting_ar, target, strategic_notes | Begin a new delivery shift, creates session record and returns session_id |
| `start_session_manual` | `start_session_manual(date, shift_start, force?, session_id?, shift_end?, day_type?, promo_windows?, starting_ar?, ending_ar?, target?, actual?, whale_catches?, strategic_notes?)` | ⭐ | date, shift_start | force, session_id, shift_end, day_type, promo_windows, starting_ar, ending_ar, target, actual, whale_catches, strategic_notes | Create a delivery session with explicit date/time (for backfilling). |

### Delivery Advisor Tools (11)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `advise_order` | `advise_order(stt_text?, app?, merchant?, pay?, miles?, destination?, ar?, whale_window_active?, catering?, promo_active?, end_of_shift?, session_id?, format?)` | ⭐ | — | stt_text, app, merchant, pay, miles, destination, ar, whale_window_active, catering, promo_active, end_of_shift, session_id, format | Evaluate a delivery order in real-time. Uses thresholds from life-os/systems/delivery.yaml (AR zones, $/mile, whale thresholds). Supports messy STT input. Returns verdict with reasoning breakdown. |
| `get_all_merchants` | `get_all_merchants()` |  | — | — | List all known merchants with their tiers (for debugging) |
| `get_intersection_distances` | `get_intersection_distances(hub?, include_custom?)` |  | — | hub, include_custom | Get all known intersection distances (built-in + custom) |
| `get_merchant` | `get_merchant(merchant_name)` |  | merchant_name | — | Get merchant tier, wait time estimate, and watchlist status |
| `get_ruleset` | `get_ruleset()` |  | — | — | Get the current decision engine rules and thresholds. Shows config from life-os/systems/delivery.yaml (AR zones, $/mile thresholds, whale windows, merchant tiers). Useful for debugging or explaining decisions. |
| `get_session_context` | `get_session_context(session_id?)` | ⭐ | — | session_id | Get current AR, whale mode, promo status, and running totals for active session |
| `set_current_ar` | `set_current_ar(ar, session_id?)` | ⭐ | ar | session_id | Update your current acceptance rate during a session |
| `set_end_of_shift` | `set_end_of_shift(enabled, session_id?)` |  | enabled | session_id | Toggle end of shift mode (applies return-ticket logic) |
| `set_intersection_distance` | `set_intersection_distance(intersection, miles, minutes, zone?, pch_miles?, pch_minutes?)` |  | intersection, miles, minutes | zone, pch_miles, pch_minutes | Add or update an intersection distance for deadhead calculations (runtime addition) |
| `set_promo_active` | `set_promo_active(active, session_id?)` |  | active | session_id | Set whether +$2 DoorDash promo is currently active |
| `set_whale_mode` | `set_whale_mode(enabled, session_id?)` |  | enabled | session_id | Toggle whale hunting mode (prioritize staying in RV for big orders) |

### Nutrition Tools (11)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `analyze_nutrition_delivery` | `analyze_nutrition_delivery(date_from?, date_to?, include_workouts?, nutrition_filter?, workout_filter?)` |  | — | date_from, date_to, include_workouts, nutrition_filter, workout_filter | Correlate nutrition/workout data with delivery earnings. Answer questions like "do I earn more on high protein days?" |
| `food_lookup` | `food_lookup(query, quantity?)` |  | query | quantity | Look up nutrition info for a food from the embedded database |
| `gap_analysis` | `gap_analysis(days?)` |  | — | days | Find nutritional gaps - nutrients below target levels |
| `get_daily_summary` | `get_daily_summary(date?)` |  | — | date | Get nutrition totals for a specific day. Use for "what were my macros on [day]?" questions. |
| `get_meals` | `get_meals(date?, date_from?, date_to?, meal_type?)` |  | — | date, date_from, date_to, meal_type | Get raw meal data for a specific date or range. Use this when user asks "what did I eat on [day]?" |
| `get_weekly_trends` | `get_weekly_trends(start_date?)` |  | — | start_date | Get 7-day nutrition overview. Use for general "how is my nutrition?" questions to avoid context overflow. |
| `log_meal` | `log_meal(meal_type, description, date?, time?, meal_key?, source?, restaurant?, calories?, protein?, carbs?, fat?, fiber?, sodium?, vitamin_a?, vitamin_c?, vitamin_d?, vitamin_e?, vitamin_k?, b12?, calcium?, iron?, magnesium?, zinc?, potassium?, omega3?, notes?)` |  | meal_type, description | date, time, meal_key, source, restaurant, calories, protein, carbs, fat, fiber, sodium, vitamin_a, vitamin_c, vitamin_d, vitamin_e, vitamin_k, b12, calcium, iron, magnesium, zinc, potassium, omega3, notes | Log a meal with nutrition data |
| `log_meal_quick` | `log_meal_quick(input, meal_type?)` | ⭐ | input | meal_type | Log a meal using natural language with auto-lookup. Perfect for voice input: "had my smoothie and 3 eggs for breakfast" |
| `log_supplement` | `log_supplement(supplement, amount, date?, notes?)` |  | supplement, amount | date, notes | Log supplement intake |
| `log_workout` | `log_workout(type, description, duration_mins, date?, time?, notes?)` |  | type, description, duration_mins | date, time, notes | Log a workout |
| `nutrient_check` | `nutrient_check(nutrient, days?)` |  | nutrient | days | Check a specific nutrient over time against targets |

### Finance Tools (11)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `add_bill` | `add_bill(payee, amount, due_day?, type?, autopay?, notes?)` |  | payee, amount | due_day, type, autopay, notes | Add a new bill (recurring or one-time) |
| `delete_bill` | `delete_bill(payee)` |  | payee | — | Delete a bill (use for one-time bills after paying) |
| `get_bills` | `get_bills()` |  | — | — | Get list of all recurring bills |
| `get_bills_due` | `get_bills_due(days?)` |  | — | days | See upcoming bills due in the next N days |
| `get_delivery_income` | `get_delivery_income(month?)` |  | — | month | Get delivery income for a month (pulled from delivery sessions) |
| `get_monthly_summary` | `get_monthly_summary(month?)` |  | — | month | Get financial summary for a month - income (including delivery), expenses, bills, net cash flow |
| `get_spending_by_category` | `get_spending_by_category(month?)` |  | — | month | See spending breakdown by category for a month |
| `log_expense` | `log_expense(amount, category, payee, date?, payment_method?, notes?)` |  | amount, category, payee | date, payment_method, notes | Log a purchase or expense |
| `log_income` | `log_income(amount, source, date?, notes?)` |  | amount, source | date, notes | Log income (non-delivery income - delivery income is auto-pulled from sessions) |
| `pay_bill` | `pay_bill(payee, amount?, date?)` |  | payee | amount, date | Record a bill payment. Uses fuzzy matching - "discover" matches "Discover Credit Card" |
| `update_bill` | `update_bill(payee, min_payment?, autopay?, notes?)` |  | payee | min_payment, autopay, notes | Update a bill's minimum payment, autopay status, or notes |

### YouTube Tools (13)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `yt_add_idea` | `yt_add_idea(title, tier?, thesis?, pillars?, hook_ideas?, evidence?, frame?, themes?, frameworks?, estimated_length?, notes?)` |  | title | tier, thesis, pillars, hook_ideas, evidence, frame, themes, frameworks, estimated_length, notes | Create a new YouTube video idea from a brain dump |
| `yt_bulk_import` | `yt_bulk_import(ideas?)` |  | — | ideas | Import multiple YouTube video ideas at once |
| `yt_bulk_update` | `yt_bulk_update(updates?)` |  | — | updates | Update multiple YouTube ideas at once (max 20) |
| `yt_connect_ideas` | `yt_connect_ideas(id1, id2)` |  | id1, id2 | — | Link two related YouTube video ideas together |
| `yt_generate_outline` | `yt_generate_outline(id, advance_status?)` |  | id | advance_status | Generate a structured 3-pillar outline from an existing YouTube idea, using thesis/pillars/hooks/evidence |
| `yt_get_connections` | `yt_get_connections(id)` |  | id | — | Get all ideas connected to a specific YouTube video idea |
| `yt_get_idea` | `yt_get_idea(id)` |  | id | — | Get full details of a YouTube video idea |
| `yt_get_pipeline` | `yt_get_pipeline()` |  | — | — | Get overview of YouTube video pipeline - counts by status and in-progress items |
| `yt_get_script` | `yt_get_script(id)` |  | id | — | Get the script and outline for a YouTube video idea |
| `yt_list_by_status` | `yt_list_by_status(status, limit?)` |  | status | limit | List YouTube video ideas by status |
| `yt_save_script` | `yt_save_script(id, script)` |  | id, script | — | Save or update the script for a YouTube video idea |
| `yt_search_ideas` | `yt_search_ideas(keyword?, tier?, status?, theme?, framework?, limit?)` |  | — | keyword, tier, status, theme, framework, limit | Search YouTube video ideas by keyword, tier, status, theme, or framework |
| `yt_update_idea` | `yt_update_idea(id, title?, tier?, status?, thesis?, pillars?, hook_ideas?, evidence?, frame?, themes?, frameworks?, estimated_length?, notes?, outline?, research_notes?)` |  | id | title, tier, status, thesis, pillars, hook_ideas, evidence, frame, themes, frameworks, estimated_length, notes, outline, research_notes | Update any field on a YouTube video idea |

### Media Tools (12)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `media_add` | `media_add(title, type, status?, rating?, notes?, creator?, creator_link?, url?, tags?, year?, date_consumed?)` |  | title, type | status, rating, notes, creator, creator_link, url, tags, year, date_consumed | Add a movie, game, TV show, etc. to your library |
| `media_bulk_update` | `media_bulk_update(updates?)` |  | — | updates | Update multiple media items at once (max 50) |
| `media_by_creator` | `media_by_creator(creator)` |  | creator | — | Get all media by a specific creator with stats |
| `media_delete` | `media_delete(id)` |  | id | — | Delete a media item from your library |
| `media_get` | `media_get(id)` |  | id | — | Get details of a specific media item by ID |
| `media_get_stats` | `media_get_stats()` |  | — | — | Get statistics about your media library (counts by type, average rating, rating distribution, top tags) |
| `media_log_watch` | `media_log_watch(id, rating, notes?, date_consumed?)` |  | id, rating | notes, date_consumed | Mark an existing item as Completed and add a rating/review |
| `media_recent` | `media_recent(n?, type?)` |  | — | n, type | Get recently completed media items |
| `media_search` | `media_search(keyword?, type?, status?, rating_min?, rating_max?, year?, creator?, tags?, limit?)` |  | — | keyword, type, status, rating_min, rating_max, year, creator, tags, limit | Search your media library by keyword, type, rating, year, or status |
| `media_timeline` | `media_timeline(year?, month?, type?)` |  | — | year, month, type | Get media completed in a specific time period |
| `media_top_rated` | `media_top_rated(type?, limit?, min_rating?)` |  | — | type, limit, min_rating | Get your top rated media items |
| `media_update` | `media_update(id, title?, type?, status?, rating?, notes?, creator?, creator_link?, url?, tags?, year?, date_consumed?)` |  | id | title, type, status, rating, notes, creator, creator_link, url, tags, year, date_consumed | Update an existing media item |

### Creators Tools (5)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `creator_add` | `creator_add(name, type, focus?, notes?, url?)` |  | name, type | focus, notes, url | Add a new creator (director, author, YouTuber, etc.) |
| `creator_get` | `creator_get(id)` |  | id | — | Get creator details with aggregated media stats |
| `creator_rankings` | `creator_rankings(type?, min_media_count?, limit?)` |  | — | type, min_media_count, limit | Get top-rated creators by average rating |
| `creator_search` | `creator_search(keyword?, type?, limit?)` |  | — | keyword, type, limit | Search creators by name or type |
| `creator_update` | `creator_update(id, name?, type?, focus?, notes?, url?)` |  | id | name, type, focus, notes, url | Update a creator record |

### Tasks Tools (5)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `add_task` | `add_task(title, date?, time?, description?, priority?)` |  | title | date, time, description, priority | Create a new task/todo item for today or a specific date |
| `delete_task` | `delete_task(id)` |  | id | — | Delete a task permanently |
| `get_recent` | `get_recent(type?)` |  | — | type | Get recently accessed records (media, ideas, creators) for quick re-use |
| `get_tasks` | `get_tasks(id?, date?, range_days?, status?, priority?, include_completed?, limit?)` |  | — | id, date, range_days, status, priority, include_completed, limit | Get tasks/todos with optional filters |
| `update_task` | `update_task(id, status?, title?, time?, description?, priority?)` |  | id | status, title, time, description, priority | Update a task - change status, priority, time, or other fields |

### Analysis Tools (17)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `analyze_ar_impact` | `analyze_ar_impact(days?, focus?)` |  | — | days, focus | Analyze the cost of AR protection - what each AR point costs in potential earnings. |
| `analyze_decisions` | `analyze_decisions(days?, focus?)` |  | — | days, focus | Analyze decision patterns - whale catches, AR protection plays, and decision quality. |
| `analyze_hidden_tips` | `analyze_hidden_tips(days?, min_orders?, order_type?)` |  | — | days, min_orders, order_type | Analyze hidden tip patterns - which merchants/apps hide tips most and at what quoted pay levels. |
| `analyze_merchants` | `analyze_merchants(days?, min_orders?, sort_by?)` |  | — | days, min_orders, sort_by | Analyze merchant performance and compare to tier assignments. Suggests tier updates based on actual data. |
| `analyze_pay_correlation` | `analyze_pay_correlation(days?, app?, bucket_size?, order_type?)` |  | — | days, app, bucket_size, order_type | Analyze the relationship between quoted pay and final pay to identify hidden tip patterns. |
| `analyze_session_targets` | `analyze_session_targets(days?, target_type?)` |  | — | days, target_type | Analyze goal/target performance - hit rate, time to target, and earnings velocity. |
| `analyze_stacks` | `analyze_stacks(days?, min_orders?)` |  | — | days, min_orders | Compare stacked orders vs singles - performance, hidden tips, and which merchants stack well. |
| `analyze_time_patterns` | `analyze_time_patterns(days?, granularity?, metric?)` |  | — | days, granularity, metric | Analyze performance by hour, day of week, or shift to identify optimal working times. |
| `analyze_tips` | `analyze_tips(days?, group_by?, app?, order_type?)` |  | — | days, group_by, app, order_type | Analyze tip patterns by zone, merchant, tier, day of week, or hour. Returns aggregated insights instead of raw records. |
| `analyze_trends` | `analyze_trends(period?, metric?, lookback?)` |  | — | period, metric, lookback | Analyze week-over-week or month-over-month trends in key metrics. |
| `analyze_wait_times` | `analyze_wait_times(days?, min_orders?, sort_by?)` |  | — | days, min_orders, sort_by | Analyze restaurant wait times - which merchants waste your time and impact on hourly rate. |
| `analyze_zones` | `analyze_zones(days?, shift?, metric?)` |  | — | days, shift, metric | Analyze zone profitability including deadhead impact, hourly rate, and $/mile by zone. |
| `suggest_dpm_thresholds` | `suggest_dpm_thresholds(days?, target_hourly?, bucket_size?)` |  | — | days, target_hourly, bucket_size | Find optimal $/mi thresholds that would maximize hourly rate. Analyzes actual outcomes to suggest threshold adjustments. |
| `suggest_merchant_tiers` | `suggest_merchant_tiers(days?, min_orders?, hourly_threshold?)` |  | — | days, min_orders, hourly_threshold | Compare actual merchant performance to current tier assignments. Suggests upgrades/downgrades based on hourly rate and wait time data. |
| `suggest_wait_time_updates` | `suggest_wait_time_updates(days?, min_orders?)` |  | — | days, min_orders | Update merchant wait time estimates based on actual recorded wait times. |
| `suggest_whale_windows` | `suggest_whale_windows(days?)` |  | — | days | Validate and adjust whale window times based on actual whale frequency by hour. |
| `suggest_zone_rules` | `suggest_zone_rules(days?, min_orders?, hourly_threshold?)` |  | — | days, min_orders, hourly_threshold | Identify zones that should be auto-decline or have adjusted deadhead values based on actual performance. |

### Agents Tools (4)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `agent_details` | `agent_details(run_id)` |  | run_id | — | Get detailed information about a specific agent run, including tool usage stats. |
| `agent_events` | `agent_events(since?, limit?, run_id?, event_type?)` |  | — | since, limit, run_id, event_type | Get recent agent events (started, progress, completed, failed, stalled). |
| `agent_refresh` | `agent_refresh()` |  | — | — | Force refresh of agent state from disk. Use after manual file changes. |
| `agent_status` | `agent_status(status?, limit?)` | ⭐ | — | status, limit | Get current status of agents. Returns running, completed, failed, and stalled agents. |

### Goals Tools (8)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `goal_add` | `goal_add(title, type, status?, priority?, progress?, parent_id?, blocked_by?, domain?, start_date?, end_date?, notes?, icon?)` |  | title, type | status, priority, progress, parent_id, blocked_by, domain, start_date, end_date, notes, icon | Add a new goal, objective, key result, project, or task |
| `goal_bulk_update` | `goal_bulk_update(updates?)` |  | — | updates | Update multiple goals at once |
| `goal_children` | `goal_children(parent_id)` |  | parent_id | — | Get all child goals of a parent goal |
| `goal_delete` | `goal_delete(id)` |  | id | — | Delete a goal |
| `goal_get` | `goal_get(id)` |  | id | — | Get a goal by ID |
| `goal_graph` | `goal_graph()` |  | — | — | Get all goals with their relationships for visualization |
| `goal_list` | `goal_list(type?, status?, priority?, domain?, parent_id?, limit?)` |  | — | type, status, priority, domain, parent_id, limit | List goals with optional filters |
| `goal_update` | `goal_update(id, title?, type?, status?, priority?, progress?, parent_id?, blocked_by?, domain?, start_date?, end_date?, notes?, icon?)` |  | id | title, type, status, priority, progress, parent_id, blocked_by, domain, start_date, end_date, notes, icon | Update an existing goal |

### Relationships Tools (11)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `contact_add` | `contact_add(name, nickname?, relationship_type?, email?, phone?, birthday?, location?, company?, role?, how_we_met?, notes?, topics?, contact_frequency?, favorite?)` |  | name | nickname, relationship_type, email, phone, birthday, location, company, role, how_we_met, notes, topics, contact_frequency, favorite | Add a new contact to the relationship system. Use for family, friends, professional contacts, etc. |
| `contact_delete` | `contact_delete(id)` |  | id | — | Delete a contact (prefer archiving instead) |
| `contact_followups` | `contact_followups()` |  | — | — | Get contacts with overdue follow-ups |
| `contact_get` | `contact_get(id)` |  | id | — | Get details for a specific contact by ID |
| `contact_list` | `contact_list(relationship_type?, favorite?, needs_followup?, search?, include_archived?, limit?)` |  | — | relationship_type, favorite, needs_followup, search, include_archived, limit | List contacts with optional filters |
| `contact_search` | `contact_search(query)` |  | query | — | Search contacts by name |
| `contact_update` | `contact_update(id, name?, nickname?, relationship_type?, email?, phone?, birthday?, location?, company?, role?, how_we_met?, notes?, topics?, contact_frequency?, next_followup?, favorite?, archived?)` |  | id | name, nickname, relationship_type, email, phone, birthday, location, company, role, how_we_met, notes, topics, contact_frequency, next_followup, favorite, archived | Update an existing contact |
| `interaction_add` | `interaction_add(contact_id, summary, type?, date?, details?, sentiment?, topics_discussed?, location?, duration_mins?, followup_needed?, followup_date?)` |  | contact_id, summary | type, date, details, sentiment, topics_discussed, location, duration_mins, followup_needed, followup_date | Log an interaction with a contact (call, text, meeting, etc.) |
| `interaction_history` | `interaction_history(contact_id, limit?)` |  | contact_id | limit | Get interaction history for a contact |
| `interaction_recent` | `interaction_recent(days?, limit?)` |  | — | days, limit | Get recent interactions across all contacts |
| `relationship_health` | `relationship_health()` |  | — | — | Get relationship health metrics and identify contacts needing attention |

### Inbox Tools (9)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `inbox_add` | `inbox_add(title, preview?, priority?, domain?, type?)` |  | title | preview, priority, domain, type | Add a manual item to the inbox. Use for capturing quick thoughts, reminders, or items from other sources. |
| `inbox_archive` | `inbox_archive(id)` |  | id | — | Archive an inbox item without processing. Use for items that are no longer relevant. |
| `inbox_get` | `inbox_get(id)` |  | id | — | Get a single inbox item by ID with full details. |
| `inbox_list` | `inbox_list(type?, priority?, domain?, limit?)` |  | — | type, priority, domain, limit | Get active inbox items for triage. Returns pending items ordered by priority. |
| `inbox_process` | `inbox_process(id, action)` |  | id, action | — | Mark an inbox item as processed with an action. Use after handling the item. |
| `inbox_snooze` | `inbox_snooze(id, until)` |  | id, until | — | Snooze an inbox item until later. Supports relative times like "1h", "tomorrow", "next week". |
| `inbox_stats` | `inbox_stats()` |  | — | — | Get inbox statistics - counts by priority, type, and processing stats. |
| `inbox_sync` | `inbox_sync()` |  | — | — | Sync internal sources (tasks, follow-ups, at-risk goals) into the inbox. Run periodically or on demand. |
| `inbox_update` | `inbox_update(id, priority?, domain?, title?, preview?)` |  | id | priority, domain, title, preview | Update an inbox item (priority, domain, title, etc.). |

### Notes Tools (6)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `note_add` | `note_add(content, tags?, source?)` | ⭐ | content | tags, source | Add a quick note. Auto-extracts tags from content based on keywords. |
| `note_delete` | `note_delete(id)` | ⭐ | id | — | Delete a note by ID. |
| `note_explore` | `note_explore(query, limit?, max_distance?)` | ⭐ | query | limit, max_distance | Semantic search notes by meaning (wraps knowledge_search). |
| `note_links` | `note_links(id)` | ⭐ | id | — | Show outbound wiki-links and backlinks for a note (requires note_links table). |
| `note_list` | `note_list(days?, tag?, source?, limit?)` | ⭐ | — | days, tag, source, limit | List recent notes with optional filtering by days, tag, or source. |
| `note_search` | `note_search(query, tag?, limit?)` | ⭐ | query | tag, limit | Search notes by keyword using full-text search. |

### Knowledge Tools (3)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `knowledge_browse` | `knowledge_browse(topic?)` | ⭐ | — | topic | Browse configured knowledge topics and suggested search settings. |
| `knowledge_search` | `knowledge_search(query, limit?, max_distance?)` | ⭐ | query | limit, max_distance | Semantic search over your notes (MiniMax embeddings + pgvector). |
| `knowledge_status` | `knowledge_status()` | ⭐ | — | — | Show embedding pipeline status (counts of ready/pending/error). |

### Rewards Tools (4)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `log_activity` | `log_activity(activity_type, minutes, date?, notes?)` | ⭐ | activity_type, minutes | date, notes | Log a beneficial activity (walk, workout, deep_work) to earn reward credits |
| `log_reward` | `log_reward(reward_type, minutes?, date?, notes?)` | ⭐ | reward_type | minutes, date, notes | Log reward consumption (music, gaming, media, treat) to spend credits |
| `reward_rules` | `reward_rules(action?, activity_type?, reward_type?, ratio?, active?)` | ⭐ | — | action, activity_type, reward_type, ratio, active | View or modify reward rules (ratios between activities and rewards) |
| `reward_status` | `reward_status(date?)` | ⭐ | — | date | Check current reward balance, activities, and streak |

### Digest Tools (2)

| Tool | Signature | HF | Required params | Optional params | Description |
| --- | --- | --- | --- | --- | --- |
| `get_daily_digest` | `get_daily_digest(date?)` |  | — | date | Get today's summary across all domains - tasks, nutrition, delivery, bills, rewards |
| `weekly_report` | `weekly_report(weeks_back?)` |  | — | weeks_back | Get weekly performance summary across all domains |


---

## React API surface (Life OS)

### `useLifeStream(...)`

**File:** `src/features/life-stream/hooks/useLifeStream.ts`

```ts
useLifeStream({
  workspaceId: string | null,
  obsidianRoot: string | null,
  userId: string,
}) => {
  cards: StreamCard[]
  isLoading: boolean
  loadError: string | null
  currentDate: string              // ISO date (YYYY-MM-DD)
  setCurrentDate(dateIso: string): void
  refresh(): Promise<void>

  submit(input: CardSubmitInput): Promise<void>
  cancel(cardId: string): Promise<void>
  retry(cardId: string): Promise<void>
  clarify(cardId: string, option: string): Promise<void>

  expandedCardId: string | null
  setExpandedCardId(id: string | null): void
  emojiFilters: string[]
  toggleEmojiFilter(emoji: string): void
}
```

### `LifeStreamContext`

**File:** `src/features/life-stream/context/LifeStreamContext.tsx`

Provides the `useLifeStream(...)` return object to the Life Stream UI subtree. It throws if used outside the provider.

---

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [DATA_MODELS.md](DATA_MODELS.md)
- [MCP_INTEGRATION.md](MCP_INTEGRATION.md)
- [LIFE_STREAM.md](LIFE_STREAM.md)
- [STATE_MANAGEMENT.md](STATE_MANAGEMENT.md)
- [GOTCHAS.md](GOTCHAS.md)
