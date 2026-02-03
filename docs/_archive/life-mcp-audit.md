# Life-MCP Tool Audit (Archive Snapshot)

**Source:** `/Volumes/YouTube 4TB/code/_archive/life-mcp`  
**Date:** 2026-02-02  
**Mode:** stdio + HTTP (MCP SDK)

## Summary
- **Total tools:** **143**
- **Categories:** 17
- **High-frequency tools (always-on candidates):**
  - advise_order
  - log_meal_quick
  - add_delivery
  - delivery_bulk_add
  - start_session_manual
  - get_session_context
  - set_current_ar
  - agent_status
  - log_activity
  - log_reward
  - reward_status
  - reward_rules
  - note_add
  - note_list
  - note_search
  - note_explore
  - note_delete
  - note_links
  - knowledge_browse
  - knowledge_search
  - knowledge_status

## Category Counts
| Category | Count |
| --- | ---: |
| delivery | 11 |
| advisor | 11 |
| youtube | 13 |
| media | 12 |
| creators | 5 |
| finance | 11 |
| nutrition | 11 |
| tasks | 5 |
| analysis | 17 |
| agents | 4 |
| goals | 8 |
| relationships | 11 |
| inbox | 9 |
| rewards | 4 |
| notes | 6 |
| knowledge | 3 |
| digest | 2 |

## Tool Lists (by category)

### delivery (11)
start_session, start_session_manual, end_session, add_delivery, delivery_bulk_add, get_session, get_deliveries, get_session_stats, get_day_analysis, get_recommendations, generate_session_report

### advisor (11)
advise_order, set_current_ar, set_whale_mode, set_end_of_shift, set_promo_active, get_session_context, set_intersection_distance, get_intersection_distances, get_merchant, get_all_merchants, get_ruleset

### youtube (13)
yt_add_idea, yt_get_idea, yt_update_idea, yt_bulk_update, yt_search_ideas, yt_list_by_status, yt_get_pipeline, yt_connect_ideas, yt_get_connections, yt_get_script, yt_save_script, yt_bulk_import, yt_generate_outline

### media (12)
media_add, media_search, media_log_watch, media_get_stats, media_get, media_update, media_bulk_update, media_delete, media_recent, media_top_rated, media_timeline, media_by_creator

### creators (5)
creator_add, creator_get, creator_search, creator_update, creator_rankings

### finance (11)
log_expense, log_income, pay_bill, get_bills_due, get_monthly_summary, get_delivery_income, get_spending_by_category, update_bill, get_bills, add_bill, delete_bill

### nutrition (11)
log_meal, food_lookup, log_meal_quick, get_meals, log_workout, log_supplement, get_daily_summary, get_weekly_trends, nutrient_check, gap_analysis, analyze_nutrition_delivery

### tasks (5)
add_task, get_tasks, update_task, delete_task, get_recent

### analysis (17)
analyze_tips, analyze_hidden_tips, analyze_pay_correlation, analyze_zones, analyze_merchants, analyze_time_patterns, analyze_stacks, analyze_decisions, analyze_ar_impact, analyze_wait_times, analyze_trends, analyze_session_targets, suggest_merchant_tiers, suggest_dpm_thresholds, suggest_wait_time_updates, suggest_zone_rules, suggest_whale_windows

### agents (4)
agent_status, agent_events, agent_details, agent_refresh

### goals (8)
goal_add, goal_get, goal_update, goal_delete, goal_list, goal_graph, goal_bulk_update, goal_children

### relationships (11)
contact_add, contact_get, contact_update, contact_delete, contact_list, contact_search, contact_followups, interaction_add, interaction_history, interaction_recent, relationship_health

### inbox (9)
inbox_list, inbox_get, inbox_add, inbox_process, inbox_snooze, inbox_archive, inbox_sync, inbox_stats, inbox_update

### rewards (4)
log_activity, log_reward, reward_status, reward_rules

### notes (6)
note_add, note_list, note_search, note_explore, note_delete, note_links

### knowledge (3)
knowledge_status, knowledge_browse, knowledge_search

### digest (2)
get_daily_digest, weekly_report

## Observations (for Phase 2)
1. **143 tools is too heavy to load eagerly.**
2. **High-frequency list already defined** in `tool-registry.js` and should be used as the default-on set.
3. **Category-based lazy-load** is feasible (all tools are grouped by category files).
4. Several tools depend on **Airtable / Sheets / Supabase** flags — need environment gating.

## Phase 2 Strategy (preview)
1. Register only high-frequency tools at startup.
2. Load additional categories based on Codex intent + domain.
3. Add MCP `meta` tools to list available categories on demand.
4. Add telemetry log: which category loaded per message.
