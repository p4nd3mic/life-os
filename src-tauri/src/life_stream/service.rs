use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::Emitter;
use tokio::fs;
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::time::timeout;

use super::handlers::code_task::CodeTaskHandler;
use super::handlers::delivery::DeliveryHandler;
use super::handlers::media::MediaHandler;
use super::handlers::nutrition::NutritionHandler;
use super::handlers::query::QueryHandler;
use super::handlers::thought::ThoughtHandler;
use super::image_catalog::{
    load_catalog, primary_asset_path, primary_asset_path_for_context, save_catalog,
    upsert_context_override, upsert_entity_asset,
};
use super::image_manual::{
    absolute_from_relative, find_image_candidates, import_image_asset,
    promote_entity_from_source_path, resolve_entity_for_card, sync_entity_file_image,
    EntityFileSyncOptions, ResolvedEntity,
};
use super::mcp_bridge::LifeMcpBridge;
use super::obsidian::{
    DayThreadRuntimeState, ObsidianIO, SemanticRewriteLogAttempt, SemanticRewriteLogEntry,
};
use super::types::*;
use crate::backend::app_server::{next_background_callback_id, WorkspaceSession};
use crate::codex_params::build_turn_start_params;

pub struct LifeStreamService {
    cards: Arc<Mutex<HashMap<String, StreamCard>>>,
    worker_semaphore: Arc<Semaphore>,
    write_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    cancelled_cards: Arc<Mutex<HashSet<String>>>,
    obsidian: ObsidianIO,
    tmdb_api_key: Option<String>,
    mcp_bridge: LifeMcpBridge,
    emitter: Option<tauri::AppHandle>,
    event_sink: Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
}

impl LifeStreamService {
    pub fn new(obsidian_root: Option<String>, tmdb_api_key: Option<String>) -> Self {
        Self {
            cards: Arc::new(Mutex::new(HashMap::new())),
            worker_semaphore: Arc::new(Semaphore::new(5)),
            write_locks: Arc::new(Mutex::new(HashMap::new())),
            cancelled_cards: Arc::new(Mutex::new(HashSet::new())),
            obsidian: ObsidianIO::new(obsidian_root),
            tmdb_api_key,
            mcp_bridge: LifeMcpBridge::from_env(),
            emitter: None,
            event_sink: None,
        }
    }

    pub fn set_emitter(&mut self, app: tauri::AppHandle) {
        self.emitter = Some(app);
    }

    pub fn set_event_sink<F>(&mut self, sink: F)
    where
        F: Fn(LifeStreamEvent) + Send + Sync + 'static,
    {
        self.event_sink = Some(Arc::new(sink));
    }

    pub async fn load_day(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
    ) -> Result<Vec<StreamCard>, String> {
        let mut cards = self
            .obsidian
            .load_cards_for_date(workspace_path, obsidian_root, date_iso)
            .await
            .map_err(|err| err.to_string())?;

        let root = self
            .obsidian
            .resolve_root_path(workspace_path, obsidian_root)
            .map_err(|err| err.to_string())?;
        if let Ok(catalog) = load_catalog(&root).await {
            for card in &mut cards {
                hydrate_card_images_from_catalog(card, &root, &catalog);
            }
        }

        {
            let mut cards_guard = self.cards.lock().await;
            for card in &cards {
                cards_guard.insert(card.id.clone(), card.clone());
            }
        }

        Ok(cards)
    }

    pub async fn submit(
        &self,
        workspace_id: &str,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
        input: &str,
        occurred_at: Option<&str>,
        request: Option<CardRequestMeta>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let occurred = occurred_at.unwrap_or(&now).to_string();

        {
            let mut cancelled = self.cancelled_cards.lock().await;
            cancelled.remove(card_id);
        }

        let card = StreamCard {
            id: card_id.to_string(),
            occurred_at: occurred.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
            version: 1,
            card_type: CardType::Generic,
            domain: DomainId::General,
            emoji: "📝".to_string(),
            layout_mode: LayoutMode::CauseEffect,
            causal: None,
            state: CardState::Pending,
            processing_step: Some("Queued...".to_string()),
            processing_steps: Some(vec!["Queued...".to_string()]),
            title: truncate(input, 50),
            subtitle: None,
            summary: None,
            duration_ms: None,
            image: None,
            stats: None,
            entities: None,
            original_input: Some(input.to_string()),
            assistant_preview: None,
            request,
            source: None,
            expanded: None,
            clarification_options: None,
            error_message: None,
        };

        {
            let mut cards = self.cards.lock().await;
            cards.insert(card_id.to_string(), card.clone());
        }

        self.emit_event(LifeStreamEvent::CardCreated { card: card.clone() });

        self.spawn_processing(
            workspace_id.to_string(),
            workspace_path.to_string(),
            obsidian_root.map(|value| value.to_string()),
            card_id.to_string(),
            input.to_string(),
            occurred,
            None,
            self.tmdb_api_key.clone(),
        );

        Ok(())
    }

    pub async fn cancel(&self, card_id: &str) -> Result<(), String> {
        emit_patch(
            card_id,
            StreamCardPatch {
                state: Some(CardState::Cancelled),
                processing_step: Some("Cancelled".to_string()),
                error_message: Some(String::new()),
                ..Default::default()
            },
            &self.cards,
            &self.emitter,
            &self.event_sink,
        )
        .await
        .ok_or("card not found")?;

        let mut cancelled = self.cancelled_cards.lock().await;
        cancelled.insert(card_id.to_string());
        drop(cancelled);

        Ok(())
    }

    pub async fn retry(
        &self,
        workspace_id: &str,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
    ) -> Result<(), String> {
        let (input, occurred_at) = {
            let cards_guard = self.cards.lock().await;
            let card = cards_guard.get(card_id).ok_or("card not found")?;
            let input = card
                .original_input
                .clone()
                .ok_or("card has no original input")?;
            (input, card.occurred_at.clone())
        };

        {
            let mut cancelled = self.cancelled_cards.lock().await;
            cancelled.remove(card_id);
        }

        emit_patch(
            card_id,
            StreamCardPatch {
                state: Some(CardState::Processing),
                processing_step: Some("Retrying...".to_string()),
                error_message: Some(String::new()),
                ..Default::default()
            },
            &self.cards,
            &self.emitter,
            &self.event_sink,
        )
        .await
        .ok_or("card not found")?;

        self.spawn_processing(
            workspace_id.to_string(),
            workspace_path.to_string(),
            obsidian_root.map(|value| value.to_string()),
            card_id.to_string(),
            input,
            occurred_at,
            None,
            self.tmdb_api_key.clone(),
        );

        Ok(())
    }

    pub async fn resume_with_clarification(
        &self,
        workspace_id: &str,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
        option_id: &str,
    ) -> Result<(), String> {
        let (input, occurred_at) = {
            let cards_guard = self.cards.lock().await;
            let card = cards_guard.get(card_id).ok_or("card not found")?;
            let input = card
                .original_input
                .clone()
                .ok_or("card has no original input")?;
            (input, card.occurred_at.clone())
        };

        emit_patch(
            card_id,
            StreamCardPatch {
                state: Some(CardState::Processing),
                processing_step: Some("Resuming...".to_string()),
                clarification_options: Some(Vec::new()),
                error_message: Some(String::new()),
                ..Default::default()
            },
            &self.cards,
            &self.emitter,
            &self.event_sink,
        )
        .await
        .ok_or("card not found")?;

        self.spawn_processing(
            workspace_id.to_string(),
            workspace_path.to_string(),
            obsidian_root.map(|value| value.to_string()),
            card_id.to_string(),
            input,
            occurred_at,
            Some(option_id.to_string()),
            self.tmdb_api_key.clone(),
        );

        Ok(())
    }

    pub async fn restructure(
        &self,
        card_id: &str,
        action: CausalRestructureAction,
        source_node_ids: Vec<String>,
        target_mode: Option<String>,
    ) -> Result<CausalRestructureResult, String> {
        let existing = {
            let cards_guard = self.cards.lock().await;
            cards_guard.get(card_id).cloned().ok_or("card not found")?
        };

        let causal = existing
            .causal
            .clone()
            .ok_or("card has no causal structure")?;

        let updated_causal = apply_restructure_action(
            card_id,
            &causal,
            action,
            &source_node_ids,
            target_mode.as_deref(),
        );

        let patch = StreamCardPatch {
            causal: Some(updated_causal),
            ..Default::default()
        };

        let version = emit_patch(
            card_id,
            patch.clone(),
            &self.cards,
            &self.emitter,
            &self.event_sink,
        )
        .await
        .ok_or("card not found")?;

        Ok(CausalRestructureResult { patch, version })
    }

    pub async fn regenerate_semantics_for_cards(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_ids: Vec<String>,
        force_llm: bool,
        persist: bool,
        workspace_session: Option<Arc<WorkspaceSession>>,
    ) -> Result<SemanticRegenerationResult, String> {
        if force_llm && workspace_session.is_none() {
            return Err(
                "LLM semantic rewrite requires an active Codex app-server session.".to_string(),
            );
        }

        let mut updated = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;
        let mut errors = Vec::new();
        let mut day_threads: HashMap<String, SemanticDayThread> = HashMap::new();
        let mut llm_log_failures = 0usize;
        let mut llm_log_writes = 0usize;

        let cards_snapshot = {
            let cards_guard = self.cards.lock().await;
            cards_guard.values().cloned().collect::<Vec<_>>()
        };
        let mut cards_by_day: HashMap<String, Vec<StreamCard>> = HashMap::new();
        for snapshot_card in cards_snapshot {
            if let Some(date_iso) = card_date_iso(snapshot_card.occurred_at.as_str()) {
                cards_by_day
                    .entry(date_iso)
                    .or_default()
                    .push(snapshot_card);
            }
        }

        for card_id in card_ids {
            let existing = {
                let cards_guard = self.cards.lock().await;
                cards_guard.get(&card_id).cloned()
            };

            let Some(card) = existing else {
                failed += 1;
                errors.push(format!("{card_id}: card not found"));
                continue;
            };

            if card.state != CardState::Complete {
                skipped += 1;
                continue;
            }

            let input_text = card
                .original_input
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    card.expanded
                        .as_ref()
                        .and_then(|expanded| expanded.original_input.as_deref())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| card.title.clone());

            if input_text.trim().is_empty() {
                skipped += 1;
                continue;
            }

            let enriched = EnrichedData {
                title: card.title.clone(),
                subtitle: card.subtitle.clone(),
                summary: card.summary.clone(),
                stats: card.stats.clone(),
                entities: card.entities.clone(),
                image: card.image.clone(),
                expanded: card.expanded.clone(),
                image_lookup: None,
            };

            let mut rebuilt = build_causal_content(
                &card.id,
                &card.card_type,
                input_text.as_str(),
                &card.occurred_at,
                &enriched,
            );

            let output_text = card_output_text(&card);
            if force_llm {
                let Some(session) = workspace_session.clone() else {
                    failed += 1;
                    errors.push(format!("{card_id}: workspace session unavailable"));
                    continue;
                };

                let card_date = card_date_iso(card.occurred_at.as_str())
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
                if !day_threads.contains_key(card_date.as_str()) {
                    let day_cards = cards_by_day
                        .get(card_date.as_str())
                        .cloned()
                        .unwrap_or_else(|| vec![card.clone()]);
                    let day_thread = match ensure_semantic_day_thread(
                        session.clone(),
                        workspace_path,
                        obsidian_root,
                        &self.obsidian,
                        card_date.as_str(),
                        &day_cards,
                        false,
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(error) => {
                            failed += 1;
                            errors.push(format!("{card_id}: {error}"));
                            continue;
                        }
                    };
                    day_threads.insert(card_date.clone(), day_thread);
                }

                let Some(day_thread) = day_threads.get(card_date.as_str()).cloned() else {
                    failed += 1;
                    errors.push(format!(
                        "{card_id}: unable to resolve day thread for {}",
                        card_date
                    ));
                    continue;
                };

                let mut rewrite_result = rewrite_semantics_with_codex(
                    session.clone(),
                    workspace_path,
                    day_thread.thread_id.as_str(),
                    &card,
                    input_text.as_str(),
                    output_text.as_deref(),
                )
                .await;

                if let Err(failure) = &rewrite_result {
                    let normalized_error = failure.error.to_lowercase();
                    let should_refresh_thread = normalized_error.contains("echoed prompt input")
                        || normalized_error.contains("returned empty output")
                        || normalized_error.contains("timed out waiting");
                    if should_refresh_thread {
                        let day_cards = cards_by_day
                            .get(card_date.as_str())
                            .cloned()
                            .unwrap_or_else(|| vec![card.clone()]);
                        if let Ok(fresh_day_thread) = ensure_semantic_day_thread(
                            session.clone(),
                            workspace_path,
                            obsidian_root,
                            &self.obsidian,
                            card_date.as_str(),
                            &day_cards,
                            true,
                        )
                        .await
                        {
                            day_threads.insert(card_date.clone(), fresh_day_thread.clone());
                            rewrite_result = rewrite_semantics_with_codex(
                                session.clone(),
                                workspace_path,
                                fresh_day_thread.thread_id.as_str(),
                                &card,
                                input_text.as_str(),
                                output_text.as_deref(),
                            )
                            .await;
                        }
                    }
                }

                match rewrite_result {
                    Ok(rewritten) => {
                        rebuilt = rewritten.causal;
                        if let Err(log_error) = self
                            .obsidian
                            .write_semantic_rewrite_log(
                                workspace_path,
                                obsidian_root,
                                rewritten.log.date.as_str(),
                                card.id.as_str(),
                                &SemanticRewriteLogEntry {
                                    card_id: rewritten.log.card_id.clone(),
                                    date: rewritten.log.date.clone(),
                                    thread_id: rewritten.log.thread_id.clone(),
                                    prompt: rewritten.log.prompt.clone(),
                                    raw_response: rewritten.log.raw_response.clone(),
                                    parsed_json: rewritten.log.parsed_json.clone(),
                                    failure_reason: rewritten.log.failure_reason.clone(),
                                    started_at: rewritten.log.started_at.clone(),
                                    completed_at: rewritten.log.completed_at.clone(),
                                    attempts: to_semantic_log_attempts(&rewritten.log.attempts),
                                    trace_stats: rewritten.log.trace_stats.clone(),
                                },
                            )
                            .await
                        {
                            llm_log_failures += 1;
                            errors.push(format!(
                                "{card_id}: failed to write semantic rewrite log: {}",
                                log_error
                            ));
                        } else {
                            llm_log_writes += 1;
                        }
                    }
                    Err(error) => {
                        let failure_log = error.log;
                        let write_result = self
                            .obsidian
                            .write_semantic_rewrite_log(
                                workspace_path,
                                obsidian_root,
                                failure_log.date.as_str(),
                                card.id.as_str(),
                                &SemanticRewriteLogEntry {
                                    card_id: failure_log.card_id.clone(),
                                    date: failure_log.date.clone(),
                                    thread_id: failure_log.thread_id.clone(),
                                    prompt: failure_log.prompt.clone(),
                                    raw_response: failure_log.raw_response.clone(),
                                    parsed_json: failure_log.parsed_json.clone(),
                                    failure_reason: failure_log
                                        .failure_reason
                                        .clone()
                                        .or(Some(error.error.clone())),
                                    started_at: failure_log.started_at.clone(),
                                    completed_at: failure_log.completed_at.clone(),
                                    attempts: to_semantic_log_attempts(&failure_log.attempts),
                                    trace_stats: failure_log.trace_stats.clone(),
                                },
                            )
                            .await;
                        if let Err(log_error) = write_result {
                            llm_log_failures += 1;
                            errors.push(format!(
                                "{card_id}: failed to write semantic rewrite log: {}",
                                log_error
                            ));
                        } else {
                            llm_log_writes += 1;
                        }
                        failed += 1;
                        errors.push(format!("{card_id}: {}", error.error));
                        continue;
                    }
                }
            }

            preserve_semantic_images(card.causal.as_ref(), &mut rebuilt);

            let patch = StreamCardPatch {
                layout_mode: Some(LayoutMode::CauseEffect),
                causal: Some(rebuilt.clone()),
                ..Default::default()
            };

            let version = emit_patch(
                &card.id,
                patch,
                &self.cards,
                &self.emitter,
                &self.event_sink,
            )
            .await;

            if version.is_some() {
                if persist {
                    if let Err(error) = self
                        .obsidian
                        .write_note_semantic_payload(
                            workspace_path,
                            obsidian_root,
                            &card.id,
                            &card.occurred_at,
                            &rebuilt,
                        )
                        .await
                    {
                        failed += 1;
                        errors.push(format!("{card_id}: {}", error));
                        continue;
                    }
                }
                updated += 1;
            } else {
                failed += 1;
                errors.push(format!("{card_id}: unable to apply patch"));
            }
        }

        let llm_log_status = if !force_llm {
            None
        } else if llm_log_failures > 0 {
            Some("failed".to_string())
        } else if llm_log_writes > 0 {
            Some("success".to_string())
        } else {
            Some("failed".to_string())
        };

        Ok(SemanticRegenerationResult {
            updated,
            skipped,
            failed,
            errors,
            llm_log_status,
        })
    }

    pub async fn day_thread_debug_summary(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
    ) -> Result<DayThreadDebugSummary, String> {
        NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
            .map_err(|error| format!("Invalid date {date_iso}: {error}"))?;
        let runtime_state = self
            .obsidian
            .read_day_thread_runtime_state(workspace_path, obsidian_root, date_iso)
            .await
            .map_err(|error| error.to_string())?;
        let root = self
            .obsidian
            .resolve_root_path(workspace_path, obsidian_root)
            .map_err(|error| error.to_string())?;
        let log_directory = root.join("Runtime").join("semantic-rewrite").join(date_iso);
        let log_directory_label = log_directory.to_string_lossy().to_string();
        let mut log_items = Vec::<DayThreadDebugLogItem>::new();

        if log_directory.exists() {
            let mut entries = fs::read_dir(&log_directory).await.map_err(|error| {
                format!(
                    "Failed reading log directory {}: {}",
                    log_directory.display(),
                    error
                )
            })?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|error| format!("Failed reading log directory entry: {error}"))?
            {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    continue;
                }
                let content = match fs::read_to_string(&path).await {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                let Ok(payload) = serde_json::from_str::<SemanticRewriteLogEntry>(&content) else {
                    continue;
                };
                let latest_attempt = payload.attempts.last();
                let attempt_prompt = latest_attempt
                    .map(|attempt| attempt.prompt.as_str())
                    .unwrap_or(payload.prompt.as_str());
                let attempt_raw = latest_attempt
                    .map(|attempt| attempt.raw_response.as_str())
                    .unwrap_or(payload.raw_response.as_str());
                let prompt_echo_detected = looks_like_prompt_echo(attempt_raw, attempt_prompt);
                let preview = sanitize_semantic_text(attempt_raw, 160);
                let trace_stats = latest_attempt
                    .and_then(|attempt| attempt.trace_stats.clone())
                    .or_else(|| payload.trace_stats.clone());
                let event_trace_preview = latest_attempt
                    .map(|attempt| {
                        attempt
                            .event_trace
                            .iter()
                            .rev()
                            .take(8)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let failure_reason = payload
                    .failure_reason
                    .clone()
                    .or_else(|| latest_attempt.and_then(|attempt| attempt.failure_reason.clone()));

                log_items.push(DayThreadDebugLogItem {
                    card_id: payload.card_id,
                    thread_id: Some(payload.thread_id),
                    started_at: Some(payload.started_at),
                    completed_at: Some(payload.completed_at),
                    failure_reason,
                    prompt_echo_detected,
                    raw_response_preview: preview,
                    trace_stats,
                    event_trace_preview,
                });
            }
        }

        log_items.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.card_id.cmp(&a.card_id))
        });

        let total_logs = log_items.len();
        let failed_logs = log_items
            .iter()
            .filter(|item| item.failure_reason.is_some())
            .count();
        let successful_logs = total_logs.saturating_sub(failed_logs);
        let last_failure_reason = log_items
            .iter()
            .find_map(|item| item.failure_reason.clone());
        let last_failure_card_id = log_items
            .iter()
            .find(|item| item.failure_reason.is_some())
            .map(|item| item.card_id.clone());

        Ok(DayThreadDebugSummary {
            date: date_iso.to_string(),
            thread_id: runtime_state.as_ref().map(|value| value.thread_id.clone()),
            last_seed_hash: runtime_state
                .as_ref()
                .and_then(|value| value.last_seed_hash.clone()),
            card_count: runtime_state.as_ref().map(|value| value.card_count),
            updated_at: runtime_state.as_ref().map(|value| value.updated_at.clone()),
            log_directory: log_directory_label,
            total_logs,
            failed_logs,
            successful_logs,
            last_failure_reason,
            last_failure_card_id,
            recent_logs: log_items.into_iter().take(20).collect::<Vec<_>>(),
        })
    }

    pub async fn reset_day_thread_state(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
    ) -> Result<(), String> {
        NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
            .map_err(|error| format!("Invalid date {date_iso}: {error}"))?;
        self.obsidian
            .delete_day_thread_runtime_state(workspace_path, obsidian_root, date_iso)
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn auth_health_check(
        &self,
        workspace_path: &str,
        workspace_session: Option<Arc<WorkspaceSession>>,
    ) -> LifeStreamAuthHealth {
        let checked_at = chrono::Utc::now().to_rfc3339();
        let Some(session) = workspace_session else {
            return LifeStreamAuthHealth {
                state: "unknown".to_string(),
                message: "Codex workspace session unavailable".to_string(),
                checked_at,
            };
        };

        let thread_id =
            match start_semantic_rewrite_day_thread(session.clone(), workspace_path).await {
                Ok(thread_id) => thread_id,
                Err(error) => {
                    let state = if semantic_auth_error_marker(error.as_str()) {
                        "unauthorized"
                    } else {
                        "error"
                    };
                    return LifeStreamAuthHealth {
                        state: state.to_string(),
                        message: error,
                        checked_at,
                    };
                }
            };

        let prompt = "Reply with exactly AUTH_OK";
        let turn_result =
            run_codex_semantic_turn(session.clone(), workspace_path, thread_id.as_str(), prompt)
                .await;
        let _ = session
            .send_request("thread/archive", json!({ "threadId": thread_id }))
            .await;

        match turn_result {
            Ok(output) => {
                let state = if output.output.contains("AUTH_OK") {
                    "healthy"
                } else {
                    "unknown"
                };
                let message = if state == "healthy" {
                    "Codex auth healthy".to_string()
                } else {
                    "Auth probe completed but returned unexpected output".to_string()
                };
                LifeStreamAuthHealth {
                    state: state.to_string(),
                    message,
                    checked_at,
                }
            }
            Err(error) => {
                let detail = error
                    .trace_stats
                    .first_error_message
                    .clone()
                    .unwrap_or(error.error.clone());
                let state = if semantic_auth_error_marker(detail.as_str()) {
                    "unauthorized"
                } else {
                    "error"
                };
                LifeStreamAuthHealth {
                    state: state.to_string(),
                    message: detail,
                    checked_at,
                }
            }
        }
    }

    pub async fn image_candidates(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
        node_id: Option<&str>,
    ) -> Result<ImageCandidateResponse, String> {
        let card = {
            let cards_guard = self.cards.lock().await;
            cards_guard.get(card_id).cloned().ok_or("card not found")?
        };
        let root = self
            .obsidian
            .resolve_root_path(workspace_path, obsidian_root)
            .map_err(|err| err.to_string())?;
        let entity = resolve_entity_for_card(&card, node_id);
        let candidates =
            find_image_candidates(&root, &entity, 12, self.tmdb_api_key.as_deref()).await?;

        Ok(ImageCandidateResponse {
            entity_key: entity.entity_key,
            entity_name: entity.entity_name,
            entity_type: entity.entity_type,
            candidates,
        })
    }

    pub async fn attach_image(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
        node_id: Option<&str>,
        source_path: &str,
        set_primary: bool,
        set_context_override: bool,
        context_hint: Option<&str>,
        update_entity_file: bool,
        update_entity_embed: bool,
    ) -> Result<ImageAttachResult, String> {
        let existing_card = {
            let cards_guard = self.cards.lock().await;
            cards_guard.get(card_id).cloned().ok_or("card not found")?
        };

        let root = self
            .obsidian
            .resolve_root_path(workspace_path, obsidian_root)
            .map_err(|err| err.to_string())?;
        let source_path_buf = std::path::Path::new(source_path);
        let resolved_entity = resolve_entity_for_card(&existing_card, node_id);
        let entity = promote_entity_from_source_path(&resolved_entity, source_path_buf);

        let imported_asset = import_image_asset(&root, &entity, source_path_buf).await?;
        let mut catalog = load_catalog(&root).await?;
        let stored_asset = upsert_entity_asset(
            &mut catalog,
            &entity.entity_key,
            &entity.entity_type,
            &entity.entity_name,
            &entity.entity_slug,
            imported_asset,
            set_primary,
        );
        if set_context_override {
            let hint = context_hint
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .or_else(|| entity.context_text.clone());
            if let Some(hint) = hint.as_deref() {
                let _ = upsert_context_override(
                    &mut catalog,
                    &entity.entity_key,
                    &stored_asset.id,
                    hint,
                    Some("manual_selection".to_string()),
                );
            }
        }
        save_catalog(&root, &catalog).await?;

        let primary_relative_path = primary_asset_path(&catalog, &entity.entity_key)
            .ok_or("no primary asset after catalog update")?;
        if update_entity_file || update_entity_embed {
            if let Err(error) = sync_entity_file_image(
                &root,
                &entity,
                &primary_relative_path,
                EntityFileSyncOptions {
                    update_frontmatter: update_entity_file,
                    update_embed_block: update_entity_embed,
                },
            )
            .await
            {
                eprintln!(
                    "life_stream: failed to sync entity image metadata for {}: {}",
                    entity.entity_key, error
                );
            }
        }
        let image = CardImage {
            url: Some(
                absolute_from_relative(&root, &primary_relative_path)
                    .to_string_lossy()
                    .to_string(),
            ),
            status: ImageStatus::Ready,
            source: Some("manual_local".to_string()),
        };

        let patch = if let Some(target_node_id) = node_id {
            let mut causal = existing_card
                .causal
                .clone()
                .ok_or("card has no causal nodes")?;
            let mut matched = false;
            for node in causal
                .left_nodes
                .iter_mut()
                .chain(causal.right_nodes.iter_mut())
            {
                if node.id != target_node_id {
                    continue;
                }
                node.image = Some(image.clone());
                if node.entity.is_none() {
                    node.entity = Some(EntityRef {
                        entity_type: entity.entity_type.clone(),
                        id: None,
                        name: entity.entity_name.clone(),
                        link: Some(entity_link_for(&entity.entity_type, &entity.entity_name)),
                    });
                }
                matched = true;
                break;
            }
            if !matched {
                return Err("target node not found on card".to_string());
            }
            StreamCardPatch {
                causal: Some(causal),
                image: Some(image.clone()),
                ..Default::default()
            }
        } else {
            let mut patch = StreamCardPatch {
                image: Some(image.clone()),
                ..Default::default()
            };

            if let Some(mut causal) = existing_card.causal.clone() {
                if let Some(first_node) = causal.right_nodes.first_mut() {
                    first_node.image = Some(image.clone());
                    if first_node.entity.is_none() {
                        first_node.entity = Some(EntityRef {
                            entity_type: entity.entity_type.clone(),
                            id: None,
                            name: entity.entity_name.clone(),
                            link: Some(entity_link_for(&entity.entity_type, &entity.entity_name)),
                        });
                    }
                }
                patch.causal = Some(causal);
            }

            patch
        };

        let version = emit_patch(
            card_id,
            patch.clone(),
            &self.cards,
            &self.emitter,
            &self.event_sink,
        )
        .await
        .ok_or("card not found")?;

        Ok(ImageAttachResult {
            patch,
            version,
            entity_key: entity.entity_key,
            primary_relative_path,
            asset: stored_asset,
        })
    }

    pub async fn backfill_entity_images(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        update_embed_block: bool,
    ) -> Result<ImageBackfillSummary, String> {
        let root = self
            .obsidian
            .resolve_root_path(workspace_path, obsidian_root)
            .map_err(|err| err.to_string())?;
        let catalog = load_catalog(&root).await?;

        let mut summary = ImageBackfillSummary {
            updated: 0,
            skipped: 0,
            failed: 0,
            errors: Vec::new(),
        };

        for (entity_key, entity_entry) in &catalog.entities {
            let Some(relative_path) = primary_asset_path(&catalog, entity_key) else {
                summary.skipped += 1;
                continue;
            };

            let entity = ResolvedEntity {
                entity_key: entity_key.clone(),
                entity_type: entity_entry.entity_type.clone(),
                entity_name: entity_entry.entity_name.clone(),
                entity_slug: entity_entry.entity_slug.clone(),
                entity_link: Some(entity_link_for(
                    entity_entry.entity_type.as_str(),
                    entity_entry.entity_name.as_str(),
                )),
                context_text: None,
            };
            match sync_entity_file_image(
                &root,
                &entity,
                &relative_path,
                EntityFileSyncOptions {
                    update_frontmatter: true,
                    update_embed_block,
                },
            )
            .await
            {
                Ok(true) => summary.updated += 1,
                Ok(false) => summary.skipped += 1,
                Err(error) => {
                    summary.failed += 1;
                    summary.errors.push(error);
                }
            }
        }

        Ok(summary)
    }

    pub async fn auto_fetch_images_for_cards(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_ids: Vec<String>,
        mode: ImageAutoFetchMode,
        update_entity_file: bool,
        update_entity_embed: bool,
    ) -> Result<ImageAutoFetchSummary, String> {
        let root = self
            .obsidian
            .resolve_root_path(workspace_path, obsidian_root)
            .map_err(|err| err.to_string())?;

        let cards_to_process = {
            let cards_guard = self.cards.lock().await;
            let target_ids: Vec<String> = if card_ids.is_empty() {
                cards_guard.keys().cloned().collect()
            } else {
                card_ids
            };

            target_ids
                .into_iter()
                .filter_map(|id| cards_guard.get(&id).cloned())
                .collect::<Vec<_>>()
        };

        let mut summary = ImageAutoFetchSummary {
            reviewed: 0,
            applied: 0,
            skipped: 0,
            failed: 0,
            errors: Vec::new(),
        };
        let mut catalog = load_catalog(&root).await?;
        let mut catalog_changed = false;

        for card in cards_to_process {
            let target_node_id = auto_image_target_node_id(&card);
            if target_node_id.is_none() && is_image_ready(card.image.as_ref()) {
                summary.skipped += 1;
                continue;
            }
            let resolved_entity = resolve_entity_for_card(&card, target_node_id.as_deref());
            let candidates =
                find_image_candidates(&root, &resolved_entity, 12, self.tmdb_api_key.as_deref())
                    .await
                    .map_err(|error| format!("{}: {}", card.id, error))?;
            let Some(top_candidate) = candidates.first() else {
                summary.skipped += 1;
                continue;
            };

            let promoted_entity = promote_entity_from_source_path(
                &resolved_entity,
                std::path::Path::new(top_candidate.source_path.as_str()),
            );

            match mode {
                ImageAutoFetchMode::ReviewFirst => {
                    let task = TaskDockItem {
                        id: format!("task_{}", uuid::Uuid::new_v4().simple()),
                        key: format!(
                            "review-image-candidates:{}:{}",
                            card.id,
                            target_node_id.as_deref().unwrap_or("card")
                        ),
                        text: format!(
                            "Review image candidates for {}",
                            promoted_entity.entity_name
                        ),
                        kind: TaskDockItemKind::Reminder,
                        completed: false,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        target_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
                        source_card_id: Some(card.id.clone()),
                        source_node_id: target_node_id.clone(),
                    };
                    if let Err(error) =
                        super::task_dock::upsert_task_dock_item_at_root(&root, task).await
                    {
                        summary.failed += 1;
                        summary.errors.push(format!(
                            "{}: failed to queue review task: {}",
                            card.id, error
                        ));
                    } else {
                        summary.reviewed += 1;
                    }
                }
                ImageAutoFetchMode::AutoApply => {
                    if promoted_entity.entity_type == "general" {
                        summary.skipped += 1;
                        continue;
                    }
                    let import_result = import_image_asset(
                        &root,
                        &promoted_entity,
                        std::path::Path::new(top_candidate.source_path.as_str()),
                    )
                    .await;
                    let imported_asset = match import_result {
                        Ok(asset) => asset,
                        Err(error) => {
                            summary.failed += 1;
                            summary
                                .errors
                                .push(format!("{}: failed to import image: {}", card.id, error));
                            continue;
                        }
                    };

                    let stored_asset = upsert_entity_asset(
                        &mut catalog,
                        &promoted_entity.entity_key,
                        &promoted_entity.entity_type,
                        &promoted_entity.entity_name,
                        &promoted_entity.entity_slug,
                        imported_asset,
                        true,
                    );
                    catalog_changed = true;

                    let relative_path = stored_asset.relative_path.clone();
                    if update_entity_file || update_entity_embed {
                        if let Err(error) = sync_entity_file_image(
                            &root,
                            &promoted_entity,
                            &relative_path,
                            EntityFileSyncOptions {
                                update_frontmatter: update_entity_file,
                                update_embed_block: update_entity_embed,
                            },
                        )
                        .await
                        {
                            summary.errors.push(format!(
                                "{}: entity metadata sync failed: {}",
                                card.id, error
                            ));
                        }
                    }

                    let image = CardImage {
                        url: Some(
                            absolute_from_relative(&root, &relative_path)
                                .to_string_lossy()
                                .to_string(),
                        ),
                        status: ImageStatus::Ready,
                        source: Some("auto_fetch".to_string()),
                    };

                    let mut patch = StreamCardPatch {
                        image: Some(image.clone()),
                        ..Default::default()
                    };
                    if let Some(mut causal) = card.causal.clone() {
                        if let Some(node_id) = target_node_id.as_deref() {
                            if let Some(node) = causal
                                .left_nodes
                                .iter_mut()
                                .chain(causal.right_nodes.iter_mut())
                                .find(|node| node.id == node_id)
                            {
                                node.image = Some(image.clone());
                                node.entity = Some(EntityRef {
                                    entity_type: promoted_entity.entity_type.clone(),
                                    id: None,
                                    name: promoted_entity.entity_name.clone(),
                                    link: Some(entity_link_for(
                                        promoted_entity.entity_type.as_str(),
                                        promoted_entity.entity_name.as_str(),
                                    )),
                                });
                            }
                        }
                        patch.causal = Some(causal);
                    }

                    if emit_patch(
                        &card.id,
                        patch,
                        &self.cards,
                        &self.emitter,
                        &self.event_sink,
                    )
                    .await
                    .is_some()
                    {
                        summary.applied += 1;
                    } else {
                        summary.failed += 1;
                        summary
                            .errors
                            .push(format!("{}: failed to apply image patch", card.id));
                    }
                }
            }
        }

        if catalog_changed {
            save_catalog(&root, &catalog).await?;
        }

        Ok(summary)
    }

    fn emit_event(&self, event: LifeStreamEvent) {
        if let Some(app) = &self.emitter {
            let _ = app.emit("life_stream_event", event.clone());
        }
        if let Some(sink) = &self.event_sink {
            sink(event);
        }
    }

    fn spawn_processing(
        &self,
        workspace_id: String,
        workspace_path: String,
        obsidian_root: Option<String>,
        card_id: String,
        input: String,
        occurred: String,
        clarification: Option<String>,
        tmdb_api_key: Option<String>,
    ) {
        let cards = Arc::clone(&self.cards);
        let semaphore = Arc::clone(&self.worker_semaphore);
        let write_locks = Arc::clone(&self.write_locks);
        let obsidian = self.obsidian.clone();
        let emitter = self.emitter.clone();
        let event_sink = self.event_sink.clone();
        let cancelled_cards = Arc::clone(&self.cancelled_cards);
        let clarification = clarification.clone();
        let mcp_bridge = self.mcp_bridge.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let lock = {
                let mut locks = write_locks.lock().await;
                locks
                    .entry(workspace_id.clone())
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone()
            };

            let _ = emit_patch(
                &card_id,
                StreamCardPatch {
                    state: Some(CardState::Processing),
                    processing_step: Some("Starting...".to_string()),
                    ..Default::default()
                },
                &cards,
                &emitter,
                &event_sink,
            )
            .await;

            let result = process_card(
                &card_id,
                workspace_path.as_str(),
                obsidian_root.as_deref(),
                &input,
                &occurred,
                clarification.as_deref(),
                &cards,
                &lock,
                &obsidian,
                &emitter,
                &event_sink,
                &cancelled_cards,
                &mcp_bridge,
                tmdb_api_key.as_deref(),
            )
            .await;

            if let Err(e) = result {
                let mut cards_guard = cards.lock().await;
                if let Some(card) = cards_guard.get_mut(&card_id) {
                    if card.state == CardState::Cancelled {
                        return;
                    }
                    card.state = CardState::Error;
                    card.error_message = Some(e.clone());
                    card.version += 1;

                    let event = LifeStreamEvent::CardError {
                        card_id: card_id.clone(),
                        message: e,
                        version: card.version,
                    };
                    if let Some(app) = &emitter {
                        let _ = app.emit("life_stream_event", event.clone());
                    }
                    if let Some(sink) = &event_sink {
                        sink(event);
                    }
                }
            }
        });
    }
}

async fn emit_patch(
    card_id: &str,
    patch: StreamCardPatch,
    cards: &Arc<Mutex<HashMap<String, StreamCard>>>,
    emitter: &Option<tauri::AppHandle>,
    event_sink: &Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
) -> Option<u32> {
    let version = {
        let mut cards_guard = cards.lock().await;
        let card = cards_guard.get_mut(card_id)?;
        apply_patch_to_card(card, &patch);
        card.version += 1;
        card.updated_at = chrono::Utc::now().to_rfc3339();
        card.version
    };

    let event = LifeStreamEvent::CardUpdated {
        card_id: card_id.to_string(),
        patch,
        version,
    };

    if let Some(app) = emitter {
        let _ = app.emit("life_stream_event", event.clone());
    }
    if let Some(sink) = event_sink {
        sink(event);
    }

    Some(version)
}

fn apply_patch_to_card(card: &mut StreamCard, patch: &StreamCardPatch) {
    if let Some(state) = &patch.state {
        card.state = state.clone();
    }
    if let Some(title) = &patch.title {
        card.title = title.clone();
    }
    if let Some(subtitle) = &patch.subtitle {
        card.subtitle = Some(subtitle.clone());
    }
    if let Some(step) = &patch.processing_step {
        card.processing_step = Some(step.clone());
        let steps = card.processing_steps.get_or_insert_with(Vec::new);
        steps.push(step.clone());
    }
    if let Some(steps) = &patch.processing_steps {
        card.processing_steps = Some(steps.clone());
    }
    if let Some(duration_ms) = patch.duration_ms {
        card.duration_ms = Some(duration_ms);
    }
    if let Some(error) = &patch.error_message {
        if error.is_empty() {
            card.error_message = None;
        } else {
            card.error_message = Some(error.clone());
        }
    }
    if let Some(preview) = &patch.assistant_preview {
        if preview.is_empty() {
            card.assistant_preview = None;
        } else {
            card.assistant_preview = Some(preview.clone());
        }
    }
    if let Some(stats) = &patch.stats {
        card.stats = Some(stats.clone());
    }
    if let Some(image) = &patch.image {
        card.image = Some(image.clone());
    }
    if let Some(expanded) = &patch.expanded {
        card.expanded = Some(expanded.clone());
    }
    if let Some(layout_mode) = &patch.layout_mode {
        card.layout_mode = layout_mode.clone();
    }
    if let Some(causal) = &patch.causal {
        card.causal = Some(causal.clone());
    }
    if let Some(options) = &patch.clarification_options {
        if options.is_empty() {
            card.clarification_options = None;
        } else {
            card.clarification_options = Some(options.clone());
        }
    }
}

async fn process_card(
    card_id: &str,
    workspace_path: &str,
    obsidian_root: Option<&str>,
    input: &str,
    occurred_at: &str,
    clarification: Option<&str>,
    cards: &Arc<Mutex<HashMap<String, StreamCard>>>,
    write_lock: &Arc<Mutex<()>>,
    obsidian: &ObsidianIO,
    emitter: &Option<tauri::AppHandle>,
    event_sink: &Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
    cancelled_cards: &Arc<Mutex<HashSet<String>>>,
    mcp_bridge: &LifeMcpBridge,
    tmdb_api_key: Option<&str>,
) -> Result<(), String> {
    if is_cancelled(card_id, cancelled_cards).await {
        return Ok(());
    }
    emit_step(card_id, "Detecting intent...", cards, emitter, event_sink).await;
    let (card_type, domain, emoji) = detect_intent(input);

    if is_cancelled(card_id, cancelled_cards).await {
        return Ok(());
    }
    emit_step(
        card_id,
        &format!("Processing as {:?}...", domain),
        cards,
        emitter,
        event_sink,
    )
    .await;

    if is_cancelled(card_id, cancelled_cards).await {
        return Ok(());
    }

    let mut enriched = match card_type {
        CardType::Meal => {
            emit_step(
                card_id,
                "Looking up nutrition...",
                cards,
                emitter,
                event_sink,
            )
            .await;
            match handle_nutrition(
                card_id,
                input,
                occurred_at,
                workspace_path,
                obsidian_root,
                clarification,
                obsidian,
                cards,
                emitter,
                event_sink,
            )
            .await?
            {
                Some(value) => value,
                None => return Ok(()),
            }
        }
        CardType::DeliveryOrder => {
            handle_delivery(input, occurred_at, workspace_path, obsidian_root, obsidian).await?
        }
        CardType::MediaAdd => {
            handle_media(input, occurred_at, workspace_path, obsidian_root, obsidian).await?
        }
        CardType::Thought => handle_thought(input).await?,
        CardType::Query => handle_query(input).await?,
        CardType::CodeTask => handle_code_task(input).await?,
        _ => handle_generic(input).await?,
    };

    if is_cancelled(card_id, cancelled_cards).await {
        return Ok(());
    }

    if let Some(mcp_output) = maybe_call_mcp_tool(
        mcp_bridge, card_id, &card_type, input, cards, emitter, event_sink,
    )
    .await
    {
        apply_mcp_output(&mut enriched, mcp_output, input);
    }

    if is_cancelled(card_id, cancelled_cards).await {
        return Ok(());
    }
    emit_step(card_id, "Saving...", cards, emitter, event_sink).await;
    {
        let _guard = write_lock.lock().await;
        obsidian
            .write_card(
                workspace_path,
                obsidian_root,
                card_id,
                occurred_at,
                &enriched,
            )
            .await
            .map_err(|err| err.to_string())?;
    }

    let causal = build_causal_content(card_id, &card_type, input, occurred_at, &enriched);

    let mut cards_guard = cards.lock().await;
    if let Some(card) = cards_guard.get_mut(card_id) {
        if card.state == CardState::Cancelled {
            return Ok(());
        }
        card.state = CardState::Complete;
        card.card_type = card_type;
        card.domain = domain;
        card.emoji = emoji;
        card.layout_mode = LayoutMode::CauseEffect;
        card.causal = Some(causal);
        card.title = enriched.title;
        card.subtitle = enriched.subtitle;
        card.summary = enriched.summary;
        card.stats = enriched.stats;
        card.entities = enriched.entities;
        card.image = enriched.image;
        card.expanded = enriched.expanded;
        card.clarification_options = None;
        card.version += 1;

        let event = LifeStreamEvent::CardCompleted { card: card.clone() };
        if let Some(app) = emitter {
            let _ = app.emit("life_stream_event", event.clone());
        }
        if let Some(sink) = event_sink {
            sink(event);
        }
    }

    // Image lookup/fetching intentionally disabled while rebuilding image pipeline.
    let _ = (obsidian_root, tmdb_api_key, enriched.image_lookup);

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmSemanticRewritePayload {
    #[serde(default)]
    semantic_mode: Option<String>,
    #[serde(default)]
    left: Option<LlmSemanticNodePayload>,
    #[serde(default)]
    right: Vec<LlmSemanticNodePayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmSemanticNodePayload {
    headline: String,
    #[serde(default)]
    summary_line: Option<String>,
    #[serde(default)]
    bullets: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticRewriteAttemptTrace {
    stage: String,
    prompt: String,
    raw_response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parsed_json: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_reason: Option<String>,
    #[serde(default, rename = "eventTrace")]
    event_trace: Vec<SemanticEventTraceEntry>,
    #[serde(default, rename = "traceStats")]
    trace_stats: Option<SemanticAttemptTraceStats>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticRewriteRunLog {
    card_id: String,
    date: String,
    thread_id: String,
    prompt: String,
    raw_response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parsed_json: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_reason: Option<String>,
    started_at: String,
    completed_at: String,
    attempts: Vec<SemanticRewriteAttemptTrace>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "traceStats")]
    trace_stats: Option<SemanticAttemptTraceStats>,
}

#[derive(Debug)]
struct SemanticRewriteRunSuccess {
    causal: CausalCardContent,
    log: SemanticRewriteRunLog,
}

#[derive(Debug)]
struct SemanticRewriteRunFailure {
    error: String,
    log: SemanticRewriteRunLog,
}

#[derive(Debug, Clone)]
struct SemanticDayThread {
    thread_id: String,
}

#[derive(Debug)]
struct SemanticTurnOutput {
    output: String,
    event_trace: Vec<SemanticEventTraceEntry>,
    trace_stats: SemanticAttemptTraceStats,
}

#[derive(Debug)]
struct SemanticTurnFailure {
    error: String,
    output: String,
    event_trace: Vec<SemanticEventTraceEntry>,
    trace_stats: SemanticAttemptTraceStats,
}

const SEMANTIC_EVENT_TRACE_LIMIT: usize = 400;
const SEMANTIC_TRACE_PREVIEW_LIMIT: usize = 300;

fn sanitize_semantic_text(value: &str, max_chars: usize) -> Option<String> {
    let cleaned = value
        .trim()
        .trim_matches(|ch| ch == '"' || ch == '\'' || ch == '`')
        .replace('\u{00a0}', " ");
    let collapsed = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if collapsed.is_empty() {
        return None;
    }
    Some(truncate_text_to_chars(collapsed.as_str(), max_chars))
}

fn truncate_text_to_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    format!("{}…", value.chars().take(max_chars).collect::<String>())
}

fn strip_json_fence(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(fenced) = trimmed
        .strip_prefix("```json")
        .and_then(|v| v.strip_suffix("```"))
    {
        return fenced.trim();
    }
    if let Some(fenced) = trimmed
        .strip_prefix("```")
        .and_then(|v| v.strip_suffix("```"))
    {
        return fenced.trim();
    }
    trimmed
}

fn extract_json_object(value: &str) -> Option<String> {
    let trimmed = strip_json_fence(value);
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(trimmed[start..=end].to_string())
}

fn parse_llm_semantic_payload(text: &str) -> Result<LlmSemanticRewritePayload, String> {
    let json_candidate = extract_json_object(text)
        .ok_or_else(|| "LLM semantic rewrite did not return JSON.".to_string())?;
    serde_json::from_str::<LlmSemanticRewritePayload>(&json_candidate)
        .map_err(|error| format!("Invalid semantic JSON payload: {error}"))
}

fn parse_semantic_mode(value: Option<&str>) -> Option<CausalSemanticMode> {
    let normalized = value?.trim().to_lowercase();
    match normalized.as_str() {
        "cause_effect" => Some(CausalSemanticMode::CauseEffect),
        "action_reward" => Some(CausalSemanticMode::ActionReward),
        "statement_why" => Some(CausalSemanticMode::StatementWhy),
        "question_response" => Some(CausalSemanticMode::QuestionResponse),
        _ => None,
    }
}

fn mode_to_roles(mode: CausalSemanticMode) -> (CausalNodeRole, CausalNodeRole) {
    match mode {
        CausalSemanticMode::CauseEffect => (CausalNodeRole::Cause, CausalNodeRole::Effect),
        CausalSemanticMode::ActionReward => (CausalNodeRole::Action, CausalNodeRole::Reward),
        CausalSemanticMode::StatementWhy => (CausalNodeRole::Cause, CausalNodeRole::Response),
        CausalSemanticMode::QuestionResponse => {
            (CausalNodeRole::Question, CausalNodeRole::Response)
        }
    }
}

fn card_output_text(card: &StreamCard) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(expanded) = &card.expanded {
        for section in &expanded.sections {
            let title = section.title.trim();
            let body = section.body.trim();
            if body.is_empty() {
                continue;
            }
            if title.is_empty() {
                parts.push(body.to_string());
            } else {
                parts.push(format!("## {title}\n{body}"));
            }
        }
    }

    if !parts.is_empty() {
        return Some(parts.join("\n\n"));
    }

    card.assistant_preview
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            card.summary
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn card_date_iso(occurred_at: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(occurred_at)
        .map(|value| value.naive_local().date().format("%Y-%m-%d").to_string())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%z")
                .map(|value| value.date().format("%Y-%m-%d").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%.f%z")
                .map(|value| value.date().format("%Y-%m-%d").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S")
                .map(|value| value.date().format("%Y-%m-%d").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|value| value.date().format("%Y-%m-%d").to_string())
        })
        .ok()
}

fn card_time_label(occurred_at: &str) -> String {
    DateTime::parse_from_rfc3339(occurred_at)
        .map(|value| value.naive_local().format("%-I:%M %p").to_string())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%z")
                .map(|value| value.format("%-I:%M %p").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%.f%z")
                .map(|value| value.format("%-I:%M %p").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S")
                .map(|value| value.format("%-I:%M %p").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|value| value.format("%-I:%M %p").to_string())
        })
        .unwrap_or_else(|_| "Unknown time".to_string())
}

fn summarize_card_for_day_seed(card: &StreamCard) -> String {
    let input_text = card
        .original_input
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            card.expanded
                .as_ref()
                .and_then(|expanded| expanded.original_input.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| card.title.clone());
    let output_summary =
        card_output_text(card).unwrap_or_else(|| card.summary.clone().unwrap_or_default());
    let input_compact = truncate_text_to_chars(input_text.trim(), 320);
    let output_compact = truncate_text_to_chars(output_summary.trim(), 420);
    format!(
        "[Card {}]\nTime: {}\nTitle: {}\nInput: {}\nOutput summary: {}",
        card.id,
        card_time_label(card.occurred_at.as_str()),
        truncate_text_to_chars(card.title.trim(), 180),
        input_compact,
        if output_compact.is_empty() {
            "(none)".to_string()
        } else {
            output_compact
        }
    )
}

fn build_day_context_seed_prompt(date_iso: &str, cards: &[StreamCard], seed_hash: &str) -> String {
    let mut sorted_cards = cards.to_vec();
    sorted_cards.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at).then(a.id.cmp(&b.id)));
    let card_lines = sorted_cards
        .iter()
        .map(summarize_card_for_day_seed)
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        r#"You are maintaining shared day context for Life Stream date {}.

Store this context silently and use it for future rewrites on the same thread.
Do not rewrite cards in this step. Do not produce schema examples.

Context hash: {}
Card count: {}

Day cards:
{}

Reply with exactly: CONTEXT_SEEDED
"#,
        date_iso,
        seed_hash,
        cards.len(),
        card_lines
    )
}

fn compute_day_seed_hash(cards: &[StreamCard]) -> String {
    let mut fingerprint_rows = cards
        .iter()
        .map(|card| format!("{}|{}", card.id, card.updated_at))
        .collect::<Vec<_>>();
    fingerprint_rows.sort();

    let mut hasher = Sha256::new();
    for row in fingerprint_rows {
        hasher.update(row.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn parse_json_value_from_response(text: &str) -> Option<Value> {
    let json_candidate = extract_json_object(text)?;
    serde_json::from_str::<Value>(&json_candidate).ok()
}

const REWRITE_PLACEHOLDER_PATTERNS: [&str; 6] = [
    "short title",
    "standalone thought",
    "optional one-liner",
    "2-5 concise bullets",
    "schema",
    "example",
];

fn find_placeholder_pattern(value: &str) -> Option<&'static str> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    REWRITE_PLACEHOLDER_PATTERNS
        .iter()
        .copied()
        .find(|pattern| normalized.contains(pattern))
}

fn payload_placeholder_violation(payload: &LlmSemanticRewritePayload) -> Option<String> {
    if let Some(left) = &payload.left {
        if let Some(pattern) = find_placeholder_pattern(left.headline.as_str()) {
            return Some(format!("left.headline contains '{pattern}'"));
        }
        if let Some(summary_line) = left.summary_line.as_deref() {
            if let Some(pattern) = find_placeholder_pattern(summary_line) {
                return Some(format!("left.summaryLine contains '{pattern}'"));
            }
        }
        for bullet in &left.bullets {
            if let Some(pattern) = find_placeholder_pattern(bullet.as_str()) {
                return Some(format!("left.bullets contains '{pattern}'"));
            }
        }
    }

    for (index, node) in payload.right.iter().enumerate() {
        if let Some(pattern) = find_placeholder_pattern(node.headline.as_str()) {
            return Some(format!("right[{index}].headline contains '{pattern}'"));
        }
        if let Some(summary_line) = node.summary_line.as_deref() {
            if let Some(pattern) = find_placeholder_pattern(summary_line) {
                return Some(format!("right[{index}].summaryLine contains '{pattern}'"));
            }
        }
        for bullet in &node.bullets {
            if let Some(pattern) = find_placeholder_pattern(bullet.as_str()) {
                return Some(format!("right[{index}].bullets contains '{pattern}'"));
            }
        }
    }

    None
}

fn semantic_rewrite_prompt(
    card: &StreamCard,
    input_text: &str,
    output_text: Option<&str>,
) -> String {
    let output = output_text.unwrap_or("");
    format!(
        r#"You already have the day context for this date.
Rewrite ONLY Card {} below into concise semantic graph nodes.

Return JSON only, no markdown fences.
Schema:
{{
  "semanticMode": "cause_effect|action_reward|statement_why|question_response",
  "left": {{
    "headline": "short title",
    "summaryLine": "optional one-liner",
    "bullets": ["short supporting point"]
  }},
  "right": [
    {{
      "headline": "standalone thought, never a fragment",
      "summaryLine": "optional one-liner",
      "bullets": ["2-5 concise bullets with concrete details"]
    }}
  ]
}}

Rules:
- Keep right nodes standalone and complete (no trailing colon fragments).
- Remove duplicate/near-duplicate nodes.
- Strip assistant boilerplate and greetings.
- Rewrite only this card, but use the day context for coherence.
- Prefer quality and coherence over speed.
- Include concrete details in bullets.
- Never echo system status messages, test acknowledgements, or speaker names.
- Headlines must read as complete ideas on their own.
- Do not include image guidance.
- Keep right node count <= 7.

Context:
cardType: {}
title: {}
input:
{}

output:
{}
"#,
        card.id,
        format!("{:?}", card.card_type).to_lowercase(),
        card.title,
        input_text.trim(),
        output.trim(),
    )
}

fn semantic_rewrite_retry_prompt(
    card: &StreamCard,
    input_text: &str,
    output_text: Option<&str>,
    previous_nodes: &[CausalNode],
) -> String {
    let output = output_text.unwrap_or("");
    let previous = if previous_nodes.is_empty() {
        "none".to_string()
    } else {
        previous_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let headline = node
                    .headline
                    .as_deref()
                    .or(node.title.as_deref())
                    .unwrap_or(node.text.as_str());
                format!("{}. {}", index + 1, headline)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You already have the day context for this date.
The previous semantic rewrite for Card {} had low quality. Rewrite ONLY this card again with higher precision.

Return STRICT JSON only (no prose, no markdown fences) using this schema:
{{
  "semanticMode": "cause_effect|action_reward|statement_why|question_response",
  "left": {{
    "headline": "short title",
    "summaryLine": "optional one-liner",
    "bullets": ["short supporting point"]
  }},
  "right": [
    {{
      "headline": "complete standalone thought",
      "summaryLine": "optional one-liner",
      "bullets": ["2-5 concise concrete details"]
    }}
  ]
}}

Critical constraints:
- Do NOT output fragments ending with ":".
- Do NOT include greetings, status checks, usernames, or assistant chatter.
- Do NOT duplicate the same point across nodes.
- Do NOT emit placeholder/template words like "short title", "optional one-liner", or "example".
- Each right headline must be meaningful alone.
- Prefer fewer, stronger nodes over noisy nodes.
- Keep right node count <= 7.

Previous low-quality nodes:
{}

Context:
cardType: {}
title: {}
input:
{}

output:
{}
"#,
        card.id,
        previous,
        format!("{:?}", card.card_type).to_lowercase(),
        card.title,
        input_text.trim(),
        output.trim(),
    )
}

fn llm_bullets_from_payload(node: &LlmSemanticNodePayload, headline: &str) -> Vec<String> {
    let normalized_headline = semantic_headline_key(headline);
    let mut seen = HashSet::new();
    let mut bullets = Vec::new();

    for item in &node.bullets {
        let cleaned = sanitize_semantic_text(item.as_str(), 200);
        let Some(cleaned) = cleaned else {
            continue;
        };
        if is_meta_boilerplate_headline(cleaned.as_str()) {
            continue;
        }
        let key = semantic_headline_key(cleaned.as_str());
        if key.is_empty() || key == normalized_headline || !seen.insert(key) {
            continue;
        }
        bullets.push(cleaned);
    }

    if bullets.is_empty() {
        if let Some(summary) = node
            .summary_line
            .as_deref()
            .and_then(|value| sanitize_semantic_text(value, 180))
        {
            let summary_key = semantic_headline_key(summary.as_str());
            if !summary_key.is_empty() && summary_key != normalized_headline {
                bullets.push(summary);
            }
        }
    }

    bullets
}

fn rewrite_meta_headline(headline: &str, bullets: &[String]) -> Option<String> {
    let normalized = semantic_headline_key(headline);
    if normalized.contains("codex is responding")
        || normalized.contains("status online")
        || normalized.contains("receiving your messages")
    {
        return Some("Codex connectivity check passed".to_string());
    }
    if normalized.contains("next test idea") {
        return None;
    }
    for bullet in bullets {
        let candidate = rewrite_fragmentary_headline(bullet.as_str(), None);
        if !candidate.is_empty()
            && !is_meta_boilerplate_headline(candidate.as_str())
            && !is_weak_semantic_fragment(candidate.as_str())
        {
            return Some(candidate);
        }
    }
    None
}

fn normalize_llm_right_nodes(
    card_id: &str,
    right_nodes: &[LlmSemanticNodePayload],
    role: CausalNodeRole,
) -> Vec<CausalNode> {
    let mut normalized_nodes = Vec::new();
    let mut seen = HashSet::new();

    for node in right_nodes {
        let headline_seed = sanitize_semantic_text(node.headline.as_str(), 150);
        let Some(headline_seed) = headline_seed else {
            continue;
        };
        let mut bullets = llm_bullets_from_payload(node, headline_seed.as_str());
        let mut headline = rewrite_fragmentary_headline(headline_seed.as_str(), Some(&bullets));
        if is_meta_boilerplate_headline(headline.as_str()) {
            if let Some(rewritten) = rewrite_meta_headline(headline.as_str(), &bullets) {
                headline = rewritten;
            } else {
                continue;
            }
        }
        if is_weak_semantic_fragment(headline.as_str()) {
            if let Some(first_bullet) = bullets.first() {
                headline = rewrite_fragmentary_headline(
                    format!("{headline} — {}", first_bullet).as_str(),
                    Some(&bullets),
                );
            }
        }
        if is_weak_semantic_fragment(headline.as_str())
            || is_meta_boilerplate_headline(headline.as_str())
        {
            continue;
        }

        let headline_key = semantic_headline_key(headline.as_str());
        if headline_key.is_empty() || !seen.insert(headline_key) {
            continue;
        }

        if bullets.len() > 6 {
            bullets.truncate(6);
        }
        let details = if bullets.is_empty() {
            node.summary_line
                .as_deref()
                .and_then(|value| sanitize_semantic_text(value, 220))
        } else {
            Some(
                bullets
                    .iter()
                    .map(|item| format!("- {item}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        let summary_line = node
            .summary_line
            .as_deref()
            .and_then(|value| sanitize_semantic_text(value, 180))
            .filter(|summary| {
                semantic_headline_key(summary.as_str()) != semantic_headline_key(headline.as_str())
            });

        let text = if bullets.is_empty() {
            headline.clone()
        } else {
            format!("{}: {}", headline, bullets.join("; "))
        };

        normalized_nodes.push(CausalNode {
            id: format!("{}:right:{}", card_id, normalized_nodes.len()),
            text,
            headline: Some(headline.clone()),
            summary_line,
            title: Some(headline),
            bullets: (!bullets.is_empty()).then_some(bullets),
            details,
            role: Some(role.clone()),
            rank: Some((normalized_nodes.len() + 1) as u32),
            group_type: Some(CausalGroupType::Primary),
            is_image_applicable: false,
            image: None,
            entity: None,
            occurred_at: None,
        });
    }

    normalized_nodes
}

fn llm_nodes_need_retry(nodes: &[CausalNode]) -> bool {
    if nodes.is_empty() {
        return true;
    }

    let mut weak = 0usize;
    let mut meta = 0usize;
    let mut duplicate = 0usize;
    let mut seen = HashSet::new();

    for node in nodes {
        let headline = node
            .headline
            .as_deref()
            .or(node.title.as_deref())
            .unwrap_or(node.text.as_str());

        if is_meta_boilerplate_headline(headline) {
            meta += 1;
            continue;
        }
        if is_weak_semantic_fragment(headline) {
            weak += 1;
        }
        let key = semantic_headline_key(headline);
        if key.is_empty() || !seen.insert(key) {
            duplicate += 1;
        }
    }

    meta > 0 || weak > 0 || duplicate > 0
}

fn extract_thread_id_from_response(response: &Value) -> Option<String> {
    response
        .get("result")
        .and_then(|result| result.get("threadId"))
        .or_else(|| {
            response
                .get("result")
                .and_then(|result| result.get("thread"))
                .and_then(|thread| thread.get("id"))
        })
        .or_else(|| response.get("threadId"))
        .or_else(|| response.get("thread").and_then(|thread| thread.get("id")))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_delta_text(event: &Value) -> Option<String> {
    let params = event.get("params")?;
    let delta = params
        .get("delta")
        .or_else(|| params.get("msg").and_then(|msg| msg.get("delta")))
        .or_else(|| params.get("msg").and_then(|msg| msg.get("content")))
        .or_else(|| params.get("msg").and_then(|msg| msg.get("text")))?;
    if let Some(text) = delta.as_str() {
        return Some(text.to_string());
    }
    let mut chunks = Vec::new();
    collect_message_chunks(delta, &mut chunks);
    if chunks.is_empty() {
        None
    } else {
        Some(chunks.join("\n"))
    }
}

fn truncate_trace_preview(value: &str) -> String {
    truncate_text_to_chars(
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim(),
        SEMANTIC_TRACE_PREVIEW_LIMIT,
    )
}

fn extract_turn_id_from_event(event: &Value) -> Option<String> {
    let params = event.get("params")?;
    params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .or_else(|| params.get("turn").and_then(|turn| turn.get("id")))
        .or_else(|| params.get("msg").and_then(|msg| msg.get("turn_id")))
        .or_else(|| params.get("msg").and_then(|msg| msg.get("turnId")))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_thread_ids_from_event(event: &Value) -> Vec<String> {
    fn collect(value: &Value, output: &mut Vec<String>, depth: usize) {
        if depth > 8 {
            return;
        }
        if let Some(candidate) = value
            .get("threadId")
            .or_else(|| value.get("thread_id"))
            .or_else(|| value.get("conversationId"))
            .or_else(|| value.get("conversation_id"))
            .and_then(|entry| entry.as_str())
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
        {
            output.push(candidate.to_string());
        }
        if let Some(thread_obj) = value.get("thread") {
            if let Some(candidate) = thread_obj
                .get("id")
                .or_else(|| thread_obj.get("threadId"))
                .or_else(|| thread_obj.get("thread_id"))
                .and_then(|entry| entry.as_str())
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty())
            {
                output.push(candidate.to_string());
            }
        }
        match value {
            Value::Object(map) => {
                for child in map.values() {
                    collect(child, output, depth + 1);
                }
            }
            Value::Array(items) => {
                for child in items {
                    collect(child, output, depth + 1);
                }
            }
            _ => {}
        }
    }

    let mut candidates = Vec::<String>::new();
    collect(event, &mut candidates, 0);
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect::<Vec<_>>()
}

fn extract_item_identifiers_from_event(event: &Value) -> (Option<String>, Option<String>) {
    let params = event.get("params");
    let item = params
        .and_then(|params| params.get("item"))
        .or_else(|| params.and_then(|params| params.get("msg").and_then(|msg| msg.get("item"))));
    let item_type = item
        .and_then(|item| item.get("type"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let item_id = item
        .and_then(|item| item.get("id"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    (item_type, item_id)
}

fn extract_error_message_from_event(event: &Value) -> Option<String> {
    let params = event.get("params")?;
    let error = params.get("error")?;
    if let Some(text) = error.as_str() {
        return Some(text.to_string());
    }
    if let Some(obj) = error.as_object() {
        let message = obj
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("Unknown error");
        let detail = obj
            .get("additionalDetails")
            .or_else(|| obj.get("additional_details"))
            .and_then(|value| value.as_str());
        return Some(match detail {
            Some(detail) if !detail.trim().is_empty() => format!("{message} ({detail})"),
            _ => message.to_string(),
        });
    }
    Some(error.to_string())
}

fn normalize_semantic_event_method(method: &str) -> Option<&'static str> {
    match method {
        "item/agentMessage/delta"
        | "codex/event/agent_message_content_delta"
        | "codex/event/agent_message_delta" => Some("AgentDelta"),
        "item/agentMessage"
        | "item/completed"
        | "codex/event/agent_message"
        | "codex/event/item_completed" => Some("AgentCompletedText"),
        "turn/completed" => Some("TurnCompleted"),
        "turn/error" | "error" => Some("TurnError"),
        _ => None,
    }
}

fn collect_message_chunks(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                output.push(trimmed.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_message_chunks(item, output);
            }
        }
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|value| value.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    output.push(trimmed.to_string());
                }
            }
            if let Some(value) = map.get("content") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("output") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("message") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("item") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("result") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("response") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("turn") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("items") {
                collect_message_chunks(value, output);
            }
            if let Some(value) = map.get("messages") {
                collect_message_chunks(value, output);
            }
        }
        _ => {}
    }
}

fn extract_message_text_from_value(value: &Value) -> Option<String> {
    let mut chunks = Vec::new();
    collect_message_chunks(value, &mut chunks);
    if chunks.is_empty() {
        None
    } else {
        Some(chunks.join("\n"))
    }
}

fn is_agent_item_payload(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let type_value = object
        .get("type")
        .and_then(|entry| entry.as_str())
        .unwrap_or("")
        .to_lowercase();
    if type_value.contains("agent") || type_value.contains("assistant") {
        return true;
    }
    let role_value = object
        .get("role")
        .and_then(|entry| entry.as_str())
        .unwrap_or("")
        .to_lowercase();
    if role_value == "assistant" || role_value == "agent" {
        return true;
    }
    let author_value = object
        .get("author")
        .and_then(|entry| entry.as_str())
        .unwrap_or("")
        .to_lowercase();
    if author_value == "assistant" || author_value == "agent" {
        return true;
    }
    object
        .get("source")
        .and_then(|entry| entry.as_str())
        .map(|entry| {
            let normalized = entry.to_lowercase();
            normalized.contains("assistant")
                || normalized.contains("agent")
                || normalized.contains("model")
        })
        .unwrap_or(false)
}

fn is_user_item_payload(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let type_value = object
        .get("type")
        .and_then(|entry| entry.as_str())
        .unwrap_or("")
        .to_lowercase();
    if type_value.contains("user") {
        return true;
    }
    let role_value = object
        .get("role")
        .and_then(|entry| entry.as_str())
        .unwrap_or("")
        .to_lowercase();
    if role_value == "user" {
        return true;
    }
    let author_value = object
        .get("author")
        .and_then(|entry| entry.as_str())
        .unwrap_or("")
        .to_lowercase();
    if author_value == "user" {
        return true;
    }
    object
        .get("source")
        .and_then(|entry| entry.as_str())
        .map(|entry| entry.to_lowercase().contains("user"))
        .unwrap_or(false)
}

fn collect_event_text_candidates(event: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(params) = event.get("params") {
        if let Some(msg) = params.get("msg") {
            if let Some(text) = extract_message_text_from_value(msg) {
                candidates.push(text);
            }
            if let Some(item) = msg.get("item") {
                if !is_user_item_payload(item) {
                    if let Some(text) = extract_message_text_from_value(item) {
                        candidates.push(text);
                    }
                }
            }
        }
        if let Some(item) = params.get("item") {
            if !is_user_item_payload(item) {
                if let Some(text) = extract_message_text_from_value(item) {
                    candidates.push(text);
                }
            }
        }
        if let Some(message) = params.get("message") {
            if !is_user_item_payload(message) {
                if let Some(text) = extract_message_text_from_value(message) {
                    candidates.push(text);
                }
            }
        }
        if let Some(output) = params.get("output") {
            if let Some(text) = extract_message_text_from_value(output) {
                candidates.push(text);
            }
        }
        if let Some(result) = params.get("result") {
            if let Some(text) = extract_message_text_from_value(result) {
                candidates.push(text);
            }
        }
        if let Some(turn) = params.get("turn") {
            if let Some(text) = extract_message_text_from_value(turn) {
                candidates.push(text);
            }
        }
    }

    if let Some(result) = event.get("result") {
        if let Some(text) = extract_message_text_from_value(result) {
            candidates.push(text);
        }
    }
    candidates
}

fn extract_agent_message_text_from_event(event: &Value) -> Option<String> {
    let method = event
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let params = event.get("params")?;
    match method {
        "item/agentMessage" => extract_message_text_from_value(params),
        "item/completed" => {
            let candidate = params
                .get("item")
                .or_else(|| params.get("message"))
                .or_else(|| params.get("output"))?;
            if is_agent_item_payload(candidate) {
                extract_message_text_from_value(candidate)
            } else if is_user_item_payload(candidate) {
                None
            } else {
                extract_message_text_from_value(candidate)
            }
        }
        "turn/completed" => collect_event_text_candidates(event)
            .into_iter()
            .max_by_key(|entry| entry.len()),
        "codex/event/agent_message" => params
            .get("msg")
            .or_else(|| params.get("message"))
            .and_then(extract_message_text_from_value)
            .or_else(|| {
                collect_event_text_candidates(event)
                    .into_iter()
                    .max_by_key(|entry| entry.len())
            }),
        "codex/event/item_completed" => {
            let candidate = params
                .get("msg")
                .and_then(|msg| msg.get("item"))
                .or_else(|| params.get("item"))
                .or_else(|| params.get("message"))?;
            if is_agent_item_payload(candidate) {
                extract_message_text_from_value(candidate)
            } else if is_user_item_payload(candidate) {
                None
            } else {
                extract_message_text_from_value(candidate)
            }
        }
        _ => None,
    }
}

fn looks_like_prompt_echo(raw_response: &str, prompt: &str) -> bool {
    let raw = raw_response.trim();
    let input = prompt.trim();
    if raw.is_empty() || input.is_empty() {
        return false;
    }
    if raw == input {
        return true;
    }
    let raw_lower = raw.to_lowercase();
    let input_lower = input.to_lowercase();
    if raw_lower.contains("rewrite only card")
        && raw_lower.contains("return json only")
        && raw_lower.contains("\"semanticmode\"")
        && raw_lower.contains("standalone thought")
    {
        return true;
    }
    let shared_markers = [
        "rewrite only card",
        "return json only",
        "standalone thought, never a fragment",
        "2-5 concise bullets with concrete details",
        "context:",
    ];
    let overlap_count = shared_markers
        .iter()
        .filter(|marker| raw_lower.contains(*marker) && input_lower.contains(*marker))
        .count();
    overlap_count >= 3
}

fn semantic_auth_error_marker(value: &str) -> bool {
    let normalized = value.to_lowercase();
    normalized.contains("missing bearer")
        || normalized.contains("unauthorized")
        || normalized.contains("authentication in header")
        || (normalized.contains("401") && normalized.contains("api.openai.com"))
}

fn choose_semantic_delta_output(
    item_agent_delta: &str,
    codex_content_delta: &str,
    codex_agent_delta: &str,
    generic_delta: &str,
) -> Option<(String, String)> {
    let candidates = [
        ("agent_delta:item", item_agent_delta),
        (
            "agent_delta:codex_content",
            codex_content_delta,
        ),
        ("agent_delta:codex_agent", codex_agent_delta),
        ("agent_delta:generic", generic_delta),
    ];

    let mut fallback: Option<(String, String)> = None;
    for (source, candidate) in candidates {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some((source.to_string(), trimmed.to_string()));
        }
        if parse_json_value_from_response(trimmed).is_some() {
            return Some((source.to_string(), trimmed.to_string()));
        }
    }

    fallback
}

async fn start_semantic_rewrite_day_thread(
    session: Arc<WorkspaceSession>,
    cwd: &str,
) -> Result<String, String> {
    let thread_result = session
        .send_request(
            "thread/start",
            json!({
                "cwd": cwd,
                "approvalPolicy": "never"
            }),
        )
        .await?;

    if let Some(error) = thread_result.get("error") {
        let message = error
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("Unknown error starting semantic rewrite thread");
        return Err(message.to_string());
    }
    extract_thread_id_from_response(&thread_result)
        .ok_or_else(|| "Failed to resolve thread id for semantic rewrite".to_string())
}

async fn resume_semantic_rewrite_day_thread(
    session: Arc<WorkspaceSession>,
    thread_id: &str,
) -> Result<(), String> {
    let response = session
        .send_request("thread/resume", json!({ "threadId": thread_id }))
        .await?;
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("Failed to resume semantic rewrite thread");
        return Err(message.to_string());
    }
    Ok(())
}

async fn run_codex_semantic_turn(
    session: Arc<WorkspaceSession>,
    cwd: &str,
    thread_id: &str,
    prompt: &str,
) -> Result<SemanticTurnOutput, SemanticTurnFailure> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let callback_consumer_id = next_background_callback_id("semantic-rewrite");
    session
        .register_background_callback(thread_id, callback_consumer_id.as_str(), tx)
        .await;

    let turn_params = build_turn_start_params(
        thread_id,
        vec![json!({ "type": "text", "text": prompt })],
        cwd,
        "never",
        json!({ "type": "readOnly" }),
        None,
        Some("high".to_string()),
        None,
        None,
    );

    let mut trace_stats = SemanticAttemptTraceStats::default();
    let mut event_trace = Vec::<SemanticEventTraceEntry>::new();
    let mut item_agent_delta = String::new();
    let mut codex_content_delta = String::new();
    let mut codex_agent_delta = String::new();
    let mut generic_delta = String::new();
    let mut completed_text_candidates = Vec::<String>::new();
    let mut turn_completed_candidate: Option<String> = None;

    let turn_start_result = match session.send_request("turn/start", turn_params).await {
        Ok(result) => result,
        Err(error) => {
            session
                .unregister_background_callback(thread_id, callback_consumer_id.as_str())
                .await;
            trace_stats.error_seen = true;
            trace_stats.first_error_message = Some(error.clone());
            return Err(SemanticTurnFailure {
                error,
                output: String::new(),
                event_trace,
                trace_stats,
            });
        }
    };
    if let Some(error) = turn_start_result.get("error") {
        let message = error
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("Semantic rewrite turn failed to start");
        session
            .unregister_background_callback(thread_id, callback_consumer_id.as_str())
            .await;
        trace_stats.error_seen = true;
        trace_stats.first_error_message = Some(message.to_string());
        return Err(SemanticTurnFailure {
            error: message.to_string(),
            output: String::new(),
            event_trace,
            trace_stats,
        });
    }

    let active_turn_id = turn_start_result
        .get("result")
        .and_then(|result| result.get("turn"))
        .and_then(|turn| turn.get("id"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    let collect_result = timeout(Duration::from_secs(75), async {
        loop {
            let Some(event) = rx.recv().await else {
                break Ok::<(), String>(());
            };
            let method = event
                .get("method")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if !method.is_empty() && !trace_stats.methods_seen.iter().any(|entry| entry == method) {
                if trace_stats.methods_seen.len() < 64 {
                    trace_stats.methods_seen.push(method.to_string());
                }
            }

            let thread_candidates = extract_thread_ids_from_event(&event);
            let turn_id = extract_turn_id_from_event(&event);
            let matches_thread = if thread_candidates.is_empty() {
                true
            } else {
                thread_candidates
                    .iter()
                    .any(|candidate| candidate == thread_id)
            };
            let matches_turn = active_turn_id
                .as_ref()
                .map(|expected| {
                    turn_id
                        .as_deref()
                        .map(|value| value == expected)
                        .unwrap_or(true)
                })
                .unwrap_or(true);

            let normalized = normalize_semantic_event_method(method).map(|value| value.to_string());
            let delta_preview =
                extract_delta_text(&event).map(|value| truncate_trace_preview(value.as_str()));
            let error_preview = extract_error_message_from_event(&event)
                .map(|value| truncate_trace_preview(value.as_str()));
            let params_preview = event
                .get("params")
                .or_else(|| event.get("result"))
                .map(|value| truncate_trace_preview(value.to_string().as_str()));
            let (item_type, item_id) = extract_item_identifiers_from_event(&event);

            if event_trace.len() < SEMANTIC_EVENT_TRACE_LIMIT {
                event_trace.push(SemanticEventTraceEntry {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    method: method.to_string(),
                    normalized_method: normalized.clone(),
                    thread_ids: thread_candidates.clone(),
                    turn_id: turn_id.clone(),
                    item_type,
                    item_id,
                    params_preview,
                    delta_preview: delta_preview.clone(),
                    error_preview: error_preview.clone(),
                });
            }

            match normalized.as_deref() {
                Some("AgentDelta") if matches_thread && matches_turn => {
                    if let Some(delta) =
                        extract_delta_text(&event).filter(|value| !value.trim().is_empty())
                    {
                        trace_stats.agent_delta_seen = true;
                        match method {
                            "item/agentMessage/delta" => item_agent_delta.push_str(delta.as_str()),
                            "codex/event/agent_message_content_delta" => {
                                codex_content_delta.push_str(delta.as_str())
                            }
                            "codex/event/agent_message_delta" => {
                                codex_agent_delta.push_str(delta.as_str())
                            }
                            _ => generic_delta.push_str(delta.as_str()),
                        }
                    }
                }
                Some("AgentCompletedText") if matches_thread && matches_turn => {
                    if let Some(full_text) = extract_agent_message_text_from_event(&event)
                        .filter(|value| !value.trim().is_empty())
                    {
                        trace_stats.agent_completed_seen = true;
                        completed_text_candidates.push(full_text);
                    }
                }
                Some("TurnCompleted") if matches_thread && matches_turn => {
                    trace_stats.turn_completed_seen = true;
                    if let Some(full_text) = extract_agent_message_text_from_event(&event)
                        .filter(|value| !value.trim().is_empty())
                    {
                        turn_completed_candidate = Some(full_text);
                    }
                    break Ok::<(), String>(());
                }
                Some("TurnError") if matches_thread && matches_turn => {
                    trace_stats.error_seen = true;
                    let message = extract_error_message_from_event(&event)
                        .unwrap_or_else(|| "Semantic rewrite turn failed".to_string());
                    if trace_stats.first_error_message.is_none() {
                        trace_stats.first_error_message = Some(message.clone());
                    }
                    break Err(message);
                }
                _ => {}
            }
        }
    })
    .await;

    session
        .unregister_background_callback(thread_id, callback_consumer_id.as_str())
        .await;

    let collect_error = match collect_result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some("Timed out waiting for semantic rewrite response".to_string()),
    };

    let assembled_output = if let Some((source, value)) = choose_semantic_delta_output(
        item_agent_delta.as_str(),
        codex_content_delta.as_str(),
        codex_agent_delta.as_str(),
        generic_delta.as_str(),
    ) {
        trace_stats.output_source = Some(source);
        value
    } else if let Some(completed) = completed_text_candidates
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .max_by_key(|value| value.len())
    {
        trace_stats.output_source = Some("agent_completed_text".to_string());
        completed.trim().to_string()
    } else if let Some(completed_turn) = turn_completed_candidate
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        trace_stats.output_source = Some("turn_completed_text".to_string());
        completed_turn
    } else {
        trace_stats.output_source = Some("none".to_string());
        String::new()
    };
    trace_stats.output_chars = assembled_output.chars().count();
    trace_stats.event_count = event_trace.len();

    if let Some(error) = collect_error {
        if trace_stats.first_error_message.is_none() {
            trace_stats.first_error_message = Some(error.clone());
        }
        return Err(SemanticTurnFailure {
            error,
            output: assembled_output,
            event_trace,
            trace_stats,
        });
    }

    if assembled_output.is_empty() {
        let methods = if trace_stats.methods_seen.is_empty() {
            "none".to_string()
        } else {
            trace_stats.methods_seen.join(", ")
        };
        let error = format!("Semantic rewrite returned empty output (events: {methods})");
        if trace_stats.first_error_message.is_none() {
            trace_stats.first_error_message = Some(error.clone());
        }
        return Err(SemanticTurnFailure {
            error,
            output: String::new(),
            event_trace,
            trace_stats,
        });
    }

    Ok(SemanticTurnOutput {
        output: assembled_output,
        event_trace,
        trace_stats,
    })
}

async fn ensure_semantic_day_thread(
    session: Arc<WorkspaceSession>,
    workspace_path: &str,
    obsidian_root: Option<&str>,
    obsidian: &ObsidianIO,
    date_iso: &str,
    day_cards: &[StreamCard],
    force_new_thread: bool,
) -> Result<SemanticDayThread, String> {
    NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
        .map_err(|error| format!("Invalid day thread date {date_iso}: {error}"))?;

    let runtime_state = if force_new_thread {
        None
    } else {
        obsidian
            .read_day_thread_runtime_state(workspace_path, obsidian_root, date_iso)
            .await
            .map_err(|error| error.to_string())?
    };

    let seed_hash = compute_day_seed_hash(day_cards);
    let mut thread_id = String::new();
    let mut needs_seed = true;

    if let Some(state) = runtime_state {
        if !state.thread_id.trim().is_empty()
            && resume_semantic_rewrite_day_thread(session.clone(), state.thread_id.as_str())
                .await
                .is_ok()
        {
            thread_id = state.thread_id;
            needs_seed = state.last_seed_hash.as_deref() != Some(seed_hash.as_str());
        }
    }

    if thread_id.is_empty() {
        thread_id = start_semantic_rewrite_day_thread(session.clone(), workspace_path).await?;
        needs_seed = true;
    }

    if needs_seed {
        let seed_prompt = build_day_context_seed_prompt(date_iso, day_cards, seed_hash.as_str());
        if run_codex_semantic_turn(
            session.clone(),
            workspace_path,
            thread_id.as_str(),
            seed_prompt.as_str(),
        )
        .await
        .is_err()
        {
            thread_id = start_semantic_rewrite_day_thread(session.clone(), workspace_path).await?;
            run_codex_semantic_turn(
                session.clone(),
                workspace_path,
                thread_id.as_str(),
                seed_prompt.as_str(),
            )
            .await
            .map_err(|error| {
                format!("Failed to seed day context for {date_iso}: {}", error.error)
            })?;
        }
    }

    let payload = DayThreadRuntimeState {
        date: date_iso.to_string(),
        thread_id: thread_id.clone(),
        last_seed_hash: Some(seed_hash.clone()),
        card_count: day_cards.len(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    obsidian
        .write_day_thread_runtime_state(workspace_path, obsidian_root, &payload)
        .await
        .map_err(|error| error.to_string())?;

    Ok(SemanticDayThread { thread_id })
}

fn build_rewrite_run_log(
    card_id: &str,
    date_iso: &str,
    thread_id: &str,
    started_at: &str,
    attempts: Vec<SemanticRewriteAttemptTrace>,
    failure_reason: Option<String>,
) -> SemanticRewriteRunLog {
    let (prompt, raw_response, parsed_json, trace_stats) = attempts
        .last()
        .map(|attempt| {
            (
                attempt.prompt.clone(),
                attempt.raw_response.clone(),
                attempt.parsed_json.clone(),
                attempt.trace_stats.clone(),
            )
        })
        .unwrap_or_else(|| (String::new(), String::new(), None, None));
    SemanticRewriteRunLog {
        card_id: card_id.to_string(),
        date: date_iso.to_string(),
        thread_id: thread_id.to_string(),
        prompt,
        raw_response,
        parsed_json,
        failure_reason,
        started_at: started_at.to_string(),
        completed_at: chrono::Utc::now().to_rfc3339(),
        attempts,
        trace_stats,
    }
}

fn to_semantic_log_attempt(attempt: &SemanticRewriteAttemptTrace) -> SemanticRewriteLogAttempt {
    SemanticRewriteLogAttempt {
        stage: attempt.stage.clone(),
        prompt: attempt.prompt.clone(),
        raw_response: attempt.raw_response.clone(),
        parsed_json: attempt.parsed_json.clone(),
        failure_reason: attempt.failure_reason.clone(),
        event_trace: attempt.event_trace.clone(),
        trace_stats: attempt.trace_stats.clone(),
    }
}

fn to_semantic_log_attempts(
    attempts: &[SemanticRewriteAttemptTrace],
) -> Vec<SemanticRewriteLogAttempt> {
    attempts
        .iter()
        .map(to_semantic_log_attempt)
        .collect::<Vec<_>>()
}

async fn rewrite_semantics_with_codex(
    session: Arc<WorkspaceSession>,
    workspace_path: &str,
    thread_id: &str,
    card: &StreamCard,
    input_text: &str,
    output_text: Option<&str>,
) -> Result<SemanticRewriteRunSuccess, SemanticRewriteRunFailure> {
    let date_iso = card_date_iso(card.occurred_at.as_str())
        .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
    let started_at = chrono::Utc::now().to_rfc3339();
    let mut attempts = Vec::new();

    let first_prompt = semantic_rewrite_prompt(card, input_text, output_text);
    let first_turn = match run_codex_semantic_turn(
        session.clone(),
        workspace_path,
        thread_id,
        first_prompt.as_str(),
    )
    .await
    {
        Ok(output) => output,
        Err(error) => {
            attempts.push(SemanticRewriteAttemptTrace {
                stage: "initial".to_string(),
                prompt: first_prompt.clone(),
                raw_response: error.output.clone(),
                parsed_json: None,
                failure_reason: Some(error.error.clone()),
                event_trace: error.event_trace.clone(),
                trace_stats: Some(error.trace_stats.clone()),
            });
            let log = build_rewrite_run_log(
                card.id.as_str(),
                date_iso.as_str(),
                thread_id,
                started_at.as_str(),
                attempts,
                Some(error.error.clone()),
            );
            return Err(SemanticRewriteRunFailure {
                error: error.error,
                log,
            });
        }
    };
    let first_raw = first_turn.output;
    if looks_like_prompt_echo(first_raw.as_str(), first_prompt.as_str()) {
        let error =
            "Semantic rewrite response echoed prompt input. Retained previous nodes.".to_string();
        attempts.push(SemanticRewriteAttemptTrace {
            stage: "initial".to_string(),
            prompt: first_prompt.clone(),
            raw_response: first_raw.clone(),
            parsed_json: None,
            failure_reason: Some(error.clone()),
            event_trace: first_turn.event_trace.clone(),
            trace_stats: Some(first_turn.trace_stats.clone()),
        });
        let log = build_rewrite_run_log(
            card.id.as_str(),
            date_iso.as_str(),
            thread_id,
            started_at.as_str(),
            attempts,
            Some(error.clone()),
        );
        return Err(SemanticRewriteRunFailure { error, log });
    }

    let first_parsed_json = parse_json_value_from_response(first_raw.as_str());
    let mut active_payload = match parse_llm_semantic_payload(first_raw.as_str()) {
        Ok(payload) => payload,
        Err(error) => {
            attempts.push(SemanticRewriteAttemptTrace {
                stage: "initial".to_string(),
                prompt: first_prompt.clone(),
                raw_response: first_raw.clone(),
                parsed_json: first_parsed_json.clone(),
                failure_reason: Some(error.clone()),
                event_trace: first_turn.event_trace.clone(),
                trace_stats: Some(first_turn.trace_stats.clone()),
            });
            let log = build_rewrite_run_log(
                card.id.as_str(),
                date_iso.as_str(),
                thread_id,
                started_at.as_str(),
                attempts,
                Some(error.clone()),
            );
            return Err(SemanticRewriteRunFailure { error, log });
        }
    };

    if let Some(violation) = payload_placeholder_violation(&active_payload) {
        let error = "LLM output invalid, retained previous nodes".to_string();
        attempts.push(SemanticRewriteAttemptTrace {
            stage: "initial".to_string(),
            prompt: first_prompt.clone(),
            raw_response: first_raw.clone(),
            parsed_json: first_parsed_json.clone(),
            failure_reason: Some(format!("{error}: {violation}")),
            event_trace: first_turn.event_trace.clone(),
            trace_stats: Some(first_turn.trace_stats.clone()),
        });
        let log = build_rewrite_run_log(
            card.id.as_str(),
            date_iso.as_str(),
            thread_id,
            started_at.as_str(),
            attempts,
            Some(format!("{error}: {violation}")),
        );
        return Err(SemanticRewriteRunFailure { error, log });
    }
    attempts.push(SemanticRewriteAttemptTrace {
        stage: "initial".to_string(),
        prompt: first_prompt.clone(),
        raw_response: first_raw.clone(),
        parsed_json: first_parsed_json.clone(),
        failure_reason: None,
        event_trace: first_turn.event_trace.clone(),
        trace_stats: Some(first_turn.trace_stats.clone()),
    });

    let mut semantic_mode = parse_semantic_mode(active_payload.semantic_mode.as_deref())
        .or_else(|| {
            card.causal
                .as_ref()
                .and_then(|causal| causal.semantic_mode.clone())
        })
        .unwrap_or(CausalSemanticMode::StatementWhy);
    let mut right_role = mode_to_roles(semantic_mode.clone()).1;
    let mut normalized_right_nodes =
        normalize_llm_right_nodes(card.id.as_str(), &active_payload.right, right_role.clone());

    if llm_nodes_need_retry(&normalized_right_nodes) {
        let retry_prompt =
            semantic_rewrite_retry_prompt(card, input_text, output_text, &normalized_right_nodes);
        let retry_turn = match run_codex_semantic_turn(
            session.clone(),
            workspace_path,
            thread_id,
            retry_prompt.as_str(),
        )
        .await
        {
            Ok(output) => output,
            Err(error) => {
                attempts.push(SemanticRewriteAttemptTrace {
                    stage: "retry".to_string(),
                    prompt: retry_prompt.clone(),
                    raw_response: error.output.clone(),
                    parsed_json: None,
                    failure_reason: Some(error.error.clone()),
                    event_trace: error.event_trace.clone(),
                    trace_stats: Some(error.trace_stats.clone()),
                });
                let log = build_rewrite_run_log(
                    card.id.as_str(),
                    date_iso.as_str(),
                    thread_id,
                    started_at.as_str(),
                    attempts,
                    Some(error.error.clone()),
                );
                return Err(SemanticRewriteRunFailure {
                    error: error.error,
                    log,
                });
            }
        };
        let retry_raw = retry_turn.output;
        if looks_like_prompt_echo(retry_raw.as_str(), retry_prompt.as_str()) {
            let error = "Semantic rewrite response echoed prompt input. Retained previous nodes."
                .to_string();
            attempts.push(SemanticRewriteAttemptTrace {
                stage: "retry".to_string(),
                prompt: retry_prompt.clone(),
                raw_response: retry_raw.clone(),
                parsed_json: None,
                failure_reason: Some(error.clone()),
                event_trace: retry_turn.event_trace.clone(),
                trace_stats: Some(retry_turn.trace_stats.clone()),
            });
            let log = build_rewrite_run_log(
                card.id.as_str(),
                date_iso.as_str(),
                thread_id,
                started_at.as_str(),
                attempts,
                Some(error.clone()),
            );
            return Err(SemanticRewriteRunFailure { error, log });
        }
        let retry_parsed_json = parse_json_value_from_response(retry_raw.as_str());
        let retry_payload = match parse_llm_semantic_payload(retry_raw.as_str()) {
            Ok(payload) => payload,
            Err(error) => {
                attempts.push(SemanticRewriteAttemptTrace {
                    stage: "retry".to_string(),
                    prompt: retry_prompt.clone(),
                    raw_response: retry_raw.clone(),
                    parsed_json: retry_parsed_json.clone(),
                    failure_reason: Some(error.clone()),
                    event_trace: retry_turn.event_trace.clone(),
                    trace_stats: Some(retry_turn.trace_stats.clone()),
                });
                let log = build_rewrite_run_log(
                    card.id.as_str(),
                    date_iso.as_str(),
                    thread_id,
                    started_at.as_str(),
                    attempts,
                    Some(error.clone()),
                );
                return Err(SemanticRewriteRunFailure { error, log });
            }
        };
        if let Some(violation) = payload_placeholder_violation(&retry_payload) {
            let error = "LLM output invalid, retained previous nodes".to_string();
            attempts.push(SemanticRewriteAttemptTrace {
                stage: "retry".to_string(),
                prompt: retry_prompt.clone(),
                raw_response: retry_raw.clone(),
                parsed_json: retry_parsed_json.clone(),
                failure_reason: Some(format!("{error}: {violation}")),
                event_trace: retry_turn.event_trace.clone(),
                trace_stats: Some(retry_turn.trace_stats.clone()),
            });
            let log = build_rewrite_run_log(
                card.id.as_str(),
                date_iso.as_str(),
                thread_id,
                started_at.as_str(),
                attempts,
                Some(format!("{error}: {violation}")),
            );
            return Err(SemanticRewriteRunFailure { error, log });
        }
        attempts.push(SemanticRewriteAttemptTrace {
            stage: "retry".to_string(),
            prompt: retry_prompt,
            raw_response: retry_raw,
            parsed_json: retry_parsed_json,
            failure_reason: None,
            event_trace: retry_turn.event_trace,
            trace_stats: Some(retry_turn.trace_stats),
        });
        active_payload = retry_payload;
        semantic_mode = parse_semantic_mode(active_payload.semantic_mode.as_deref())
            .or(Some(semantic_mode))
            .unwrap_or(CausalSemanticMode::StatementWhy);
        right_role = mode_to_roles(semantic_mode.clone()).1;
        normalized_right_nodes =
            normalize_llm_right_nodes(card.id.as_str(), &active_payload.right, right_role.clone());
    }

    if normalized_right_nodes.is_empty() || llm_nodes_need_retry(&normalized_right_nodes) {
        let error =
            "LLM rewrite returned low-quality semantic nodes. Try rebuilding again after refining source content."
                .to_string();
        let log = build_rewrite_run_log(
            card.id.as_str(),
            date_iso.as_str(),
            thread_id,
            started_at.as_str(),
            attempts,
            Some(error.clone()),
        );
        return Err(SemanticRewriteRunFailure { error, log });
    }

    let (left_role, _) = mode_to_roles(semantic_mode.clone());

    let base_left = card
        .causal
        .as_ref()
        .and_then(|causal| causal.left_nodes.first())
        .cloned();

    let left_payload = active_payload.left.as_ref();
    let left_headline = left_payload
        .and_then(|node| sanitize_semantic_text(node.headline.as_str(), 140))
        .or_else(|| {
            base_left
                .as_ref()
                .and_then(|node| node.headline.as_ref().cloned())
                .and_then(|value| sanitize_semantic_text(value.as_str(), 140))
        })
        .or_else(|| sanitize_semantic_text(card.title.as_str(), 140))
        .unwrap_or_else(|| "Summary".to_string());

    let left_bullets = left_payload
        .map(|node| {
            node.bullets
                .iter()
                .filter_map(|item| sanitize_semantic_text(item, 180))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());

    let left_summary = left_payload
        .and_then(|node| node.summary_line.as_deref())
        .and_then(|value| sanitize_semantic_text(value, 180));

    let left_node = CausalNode {
        id: format!("{}:left:0", card.id),
        text: input_text.trim().to_string(),
        headline: Some(left_headline.clone()),
        summary_line: left_summary,
        title: Some(left_headline),
        bullets: left_bullets,
        details: Some(input_text.trim().to_string()).filter(|value| !value.is_empty()),
        role: Some(left_role),
        rank: Some(1),
        group_type: Some(CausalGroupType::Primary),
        is_image_applicable: true,
        image: base_left.as_ref().and_then(|node| node.image.clone()),
        entity: base_left.and_then(|node| node.entity.clone()),
        occurred_at: Some(card.occurred_at.clone()),
    };

    let mut right_nodes = normalized_right_nodes;
    normalize_causal_node_titles(&mut right_nodes);

    let left_node_id = left_node.id.clone();
    let links = right_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| CausalLink {
            id: Some(format!("{}:link:{index}", card.id)),
            from_id: left_node_id.clone(),
            to_id: node.id.clone(),
            label: None,
            strength: Some(if index < DEFAULT_TOP_LINK_LIMIT {
                1.0
            } else {
                0.55
            }),
        })
        .collect::<Vec<_>>();

    let has_dense_right_nodes = right_nodes.len() > DEFAULT_VISIBLE_RIGHT_COUNT;
    let has_dense_links = links.len() > DEFAULT_TOP_LINK_LIMIT;
    let layout = if has_dense_right_nodes || has_dense_links {
        Some(CausalLayoutState {
            visible_right_count: has_dense_right_nodes
                .then_some(DEFAULT_VISIBLE_RIGHT_COUNT as u32),
            top_link_limit: has_dense_links.then_some(DEFAULT_TOP_LINK_LIMIT as u32),
            expanded: has_dense_right_nodes.then_some(false),
        })
    } else {
        None
    };
    let overflow_count = right_nodes
        .len()
        .checked_sub(DEFAULT_VISIBLE_RIGHT_COUNT)
        .map(|count| count as u32)
        .filter(|count| *count > 0);

    let causal = CausalCardContent {
        left_nodes: vec![left_node],
        right_nodes,
        links,
        layout,
        semantic_mode: Some(semantic_mode),
        compaction: Some(CausalCompactionState {
            enabled: overflow_count.is_some(),
            threshold: DEFAULT_VISIBLE_RIGHT_COUNT as u32,
            overflow_count,
        }),
        transcript_source: Some(CausalTranscriptSource::Both),
    };
    let log = build_rewrite_run_log(
        card.id.as_str(),
        date_iso.as_str(),
        thread_id,
        started_at.as_str(),
        attempts,
        None,
    );
    Ok(SemanticRewriteRunSuccess { causal, log })
}

#[cfg(test)]
mod semantic_rewrite_day_thread_tests {
    use super::*;
    use serde_json::json;

    fn sample_card(id: &str, occurred_at: &str, updated_at: &str, title: &str) -> StreamCard {
        StreamCard {
            id: id.to_string(),
            occurred_at: occurred_at.to_string(),
            created_at: occurred_at.to_string(),
            updated_at: updated_at.to_string(),
            version: 1,
            card_type: CardType::Thought,
            domain: DomainId::General,
            emoji: "💭".to_string(),
            layout_mode: LayoutMode::CauseEffect,
            causal: None,
            state: CardState::Complete,
            processing_step: None,
            processing_steps: None,
            title: title.to_string(),
            subtitle: None,
            summary: Some("Summary".to_string()),
            duration_ms: None,
            image: None,
            stats: None,
            entities: None,
            original_input: Some(title.to_string()),
            assistant_preview: None,
            request: None,
            source: None,
            expanded: None,
            clarification_options: None,
            error_message: None,
        }
    }

    #[test]
    fn compute_day_seed_hash_is_stable_and_changes_with_updates() {
        let card_a = sample_card(
            "card-a",
            "2026-02-06T10:00:00Z",
            "2026-02-06T10:00:00Z",
            "Card A",
        );
        let card_b = sample_card(
            "card-b",
            "2026-02-06T11:00:00Z",
            "2026-02-06T11:00:00Z",
            "Card B",
        );
        let hash_one = compute_day_seed_hash(&vec![card_a.clone(), card_b.clone()]);
        let hash_two = compute_day_seed_hash(&vec![card_b.clone(), card_a.clone()]);
        assert_eq!(hash_one, hash_two, "hash must be order-independent");

        let mut card_b_updated = card_b.clone();
        card_b_updated.updated_at = "2026-02-06T11:22:00Z".to_string();
        let hash_three = compute_day_seed_hash(&vec![card_a, card_b_updated]);
        assert_ne!(
            hash_one, hash_three,
            "hash must change when updatedAt changes"
        );
    }

    #[test]
    fn payload_placeholder_violation_detects_template_language() {
        let payload = LlmSemanticRewritePayload {
            semantic_mode: Some("statement_why".to_string()),
            left: Some(LlmSemanticNodePayload {
                headline: "short title".to_string(),
                summary_line: None,
                bullets: vec![],
            }),
            right: vec![LlmSemanticNodePayload {
                headline: "standalone thought".to_string(),
                summary_line: Some("optional one-liner".to_string()),
                bullets: vec!["2-5 concise bullets".to_string()],
            }],
        };
        let violation = payload_placeholder_violation(&payload);
        assert!(violation.is_some());
    }

    #[test]
    fn payload_placeholder_violation_allows_real_content() {
        let payload = LlmSemanticRewritePayload {
            semantic_mode: Some("statement_why".to_string()),
            left: Some(LlmSemanticNodePayload {
                headline: "Late meal timing raised overnight hunger".to_string(),
                summary_line: Some("Front-loading calories reduced cravings.".to_string()),
                bullets: vec!["A larger lunch made dinner easier to control".to_string()],
            }),
            right: vec![LlmSemanticNodePayload {
                headline: "Earlier protein intake improved dinner decision quality".to_string(),
                summary_line: Some(
                    "The 5pm crash was weaker after a high-protein lunch".to_string(),
                ),
                bullets: vec!["Decision fatigue dropped during dinner rush".to_string()],
            }],
        };
        let violation = payload_placeholder_violation(&payload);
        assert!(violation.is_none());
    }

    #[test]
    fn extract_agent_message_text_ignores_completed_user_message() {
        let event = json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "type": "userMessage",
                    "content": [{ "type": "text", "text": "echoed prompt text" }]
                }
            }
        });
        let parsed = extract_agent_message_text_from_event(&event);
        assert!(parsed.is_none());
    }

    #[test]
    fn extract_agent_message_text_reads_completed_agent_message() {
        let event = json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "type": "agentMessage",
                    "content": [{ "type": "text", "text": "{\"semanticMode\":\"statement_why\"}" }]
                }
            }
        });
        let parsed = extract_agent_message_text_from_event(&event);
        assert_eq!(
            parsed.as_deref(),
            Some("{\"semanticMode\":\"statement_why\"}")
        );
    }

    #[test]
    fn extract_agent_message_text_reads_completed_assistant_message_shape() {
        let event = json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "{\"semanticMode\":\"cause_effect\"}" }]
                }
            }
        });
        let parsed = extract_agent_message_text_from_event(&event);
        assert_eq!(
            parsed.as_deref(),
            Some("{\"semanticMode\":\"cause_effect\"}")
        );
    }

    #[test]
    fn extract_agent_message_text_reads_turn_completed_result_payload() {
        let event = json!({
            "method": "turn/completed",
            "params": {
                "threadId": "thread-1",
                "result": {
                    "output": [
                        {
                            "type": "message",
                            "role": "assistant",
                            "content": [{ "type": "output_text", "text": "{\"semanticMode\":\"action_reward\"}" }]
                        }
                    ]
                }
            }
        });
        let parsed = extract_agent_message_text_from_event(&event);
        assert_eq!(
            parsed.as_deref(),
            Some("{\"semanticMode\":\"action_reward\"}")
        );
    }

    #[test]
    fn extract_delta_text_reads_codex_event_delta_shape() {
        let event = json!({
            "method": "codex/event/agent_message_content_delta",
            "params": {
                "conversationId": "thread-1",
                "msg": {
                    "delta": [{ "type": "output_text_delta", "text": "{\"semanticMode\":\"statement_why\"}" }]
                }
            }
        });
        let parsed = extract_delta_text(&event).expect("delta text");
        assert!(parsed.contains("semanticMode"));
    }

    #[test]
    fn extract_agent_message_text_reads_codex_event_item_completed() {
        let event = json!({
            "method": "codex/event/item_completed",
            "params": {
                "conversationId": "thread-1",
                "msg": {
                    "item": {
                        "type": "AgentMessage",
                        "content": [{ "type": "output_text", "text": "{\"semanticMode\":\"statement_why\"}" }]
                    }
                }
            }
        });
        let parsed = extract_agent_message_text_from_event(&event);
        assert_eq!(
            parsed.as_deref(),
            Some("{\"semanticMode\":\"statement_why\"}")
        );
    }

    #[test]
    fn extract_agent_message_text_reads_codex_event_agent_message() {
        let event = json!({
            "method": "codex/event/agent_message",
            "params": {
                "conversationId": "thread-1",
                "msg": {
                    "message": {
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": "{\"semanticMode\":\"cause_effect\"}" }]
                    }
                }
            }
        });
        let parsed = extract_agent_message_text_from_event(&event);
        assert_eq!(
            parsed.as_deref(),
            Some("{\"semanticMode\":\"cause_effect\"}")
        );
    }

    #[test]
    fn extract_error_message_from_event_includes_additional_details() {
        let event = json!({
            "method": "error",
            "params": {
                "threadId": "thread-1",
                "error": {
                    "message": "Reconnecting... 1/5",
                    "additionalDetails": "unexpected status 401 Unauthorized"
                }
            }
        });
        let message = extract_error_message_from_event(&event).expect("error message");
        assert!(message.contains("Reconnecting"));
        assert!(message.contains("401 Unauthorized"));
    }

    #[test]
    fn normalize_semantic_event_method_treats_error_as_turn_error() {
        assert_eq!(normalize_semantic_event_method("error"), Some("TurnError"));
    }

    #[test]
    fn semantic_auth_error_marker_matches_unauthorized_details() {
        assert!(semantic_auth_error_marker(
            "unexpected status 401 Unauthorized: Missing bearer or basic authentication in header"
        ));
        assert!(!semantic_auth_error_marker(
            "Semantic rewrite returned empty output"
        ));
    }

    #[test]
    fn looks_like_prompt_echo_detects_identical_text() {
        let prompt = "Rewrite card X";
        assert!(looks_like_prompt_echo(prompt, prompt));
        assert!(!looks_like_prompt_echo("response body", prompt));
    }

    #[test]
    fn looks_like_prompt_echo_detects_template_marker_overlap() {
        let prompt =
            "Rewrite ONLY Card 1 below.\nReturn JSON only.\nstandalone thought, never a fragment";
        let raw = "Rewrite ONLY Card 1 below.\nReturn JSON only.\nSchema:\n\"semanticMode\"\nstandalone thought, never a fragment";
        assert!(looks_like_prompt_echo(raw, prompt));
    }

    #[test]
    fn choose_semantic_delta_output_prefers_item_stream_when_json_valid() {
        let item = "{\"semanticMode\":\"statement_why\"}";
        let codex_content = "{ { \"semanticMode\":\"statement_why\" }";
        let codex_agent = String::new();
        let generic = String::new();

        let chosen = choose_semantic_delta_output(
            item,
            codex_content,
            codex_agent.as_str(),
            generic.as_str(),
        )
        .expect("expected output");
        assert_eq!(chosen.0, "agent_delta:item");
        assert_eq!(chosen.1, item);
    }

    #[test]
    fn choose_semantic_delta_output_falls_back_when_no_json_candidate() {
        let chosen = choose_semantic_delta_output("   ", "first", "second", "third")
            .expect("expected fallback output");
        assert_eq!(chosen.0, "agent_delta:codex_content");
        assert_eq!(chosen.1, "first");
    }
}

fn normalize_node_key(node: &CausalNode) -> Option<String> {
    let candidate = node
        .headline
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .or_else(|| node.title.as_ref().map(|value| value.trim()))
        .or_else(|| Some(node.text.trim()))?;
    let normalized = candidate
        .to_lowercase()
        .replace(
            |ch: char| !ch.is_ascii_alphanumeric() && !ch.is_whitespace(),
            " ",
        )
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn preserve_semantic_images(existing: Option<&CausalCardContent>, rebuilt: &mut CausalCardContent) {
    let Some(existing) = existing else {
        return;
    };

    if let (Some(previous_left), Some(next_left)) =
        (existing.left_nodes.first(), rebuilt.left_nodes.first_mut())
    {
        if next_left.image.is_none() {
            next_left.image = previous_left.image.clone();
        }
        if next_left.entity.is_none() {
            next_left.entity = previous_left.entity.clone();
        }
        if next_left.image.is_some() {
            next_left.is_image_applicable = true;
        }
    }

    let mut by_key: HashMap<String, &CausalNode> = HashMap::new();
    for node in &existing.right_nodes {
        if let Some(key) = normalize_node_key(node) {
            by_key.entry(key).or_insert(node);
        }
    }

    for (index, node) in rebuilt.right_nodes.iter_mut().enumerate() {
        if node.image.is_some() && node.entity.is_some() {
            continue;
        }

        let match_by_key = normalize_node_key(node).and_then(|key| by_key.get(&key).copied());
        let match_by_rank = existing
            .right_nodes
            .iter()
            .find(|candidate| candidate.rank == node.rank)
            .or_else(|| existing.right_nodes.get(index));
        let source = match_by_key.or(match_by_rank);
        if let Some(previous) = source {
            if node.image.is_none() {
                node.image = previous.image.clone();
            }
            if node.entity.is_none() {
                node.entity = previous.entity.clone();
            }
            if node.image.is_some() {
                node.is_image_applicable = true;
            }
        }
    }
}

async fn emit_step(
    card_id: &str,
    step: &str,
    cards: &Arc<Mutex<HashMap<String, StreamCard>>>,
    emitter: &Option<tauri::AppHandle>,
    event_sink: &Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
) {
    let version = {
        let mut cards_guard = cards.lock().await;
        if let Some(card) = cards_guard.get_mut(card_id) {
            card.processing_step = Some(step.to_string());
            let steps = card.processing_steps.get_or_insert_with(Vec::new);
            steps.push(step.to_string());
            card.version += 1;
            card.version
        } else {
            return;
        }
    };

    let event = LifeStreamEvent::CardStep {
        card_id: card_id.to_string(),
        step: step.to_string(),
        version,
    };
    if let Some(app) = emitter {
        let _ = app.emit("life_stream_event", event.clone());
    }
    if let Some(sink) = event_sink {
        sink(event);
    }
}

#[derive(Debug, Clone)]
struct McpToolOutput {
    tool: String,
    text: Option<String>,
    raw: serde_json::Value,
}

async fn maybe_call_mcp_tool(
    mcp_bridge: &LifeMcpBridge,
    card_id: &str,
    card_type: &CardType,
    input: &str,
    cards: &Arc<Mutex<HashMap<String, StreamCard>>>,
    emitter: &Option<tauri::AppHandle>,
    event_sink: &Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
) -> Option<McpToolOutput> {
    if !mcp_bridge.is_enabled() {
        return None;
    }

    let (tool, params) = mcp_tool_for_card(card_type, input)?;

    emit_step(
        card_id,
        "Syncing with life-mcp...",
        cards,
        emitter,
        event_sink,
    )
    .await;

    let Ok(result) = mcp_bridge.call_tool(&tool, params).await else {
        return None;
    };
    let Some(raw) = result else {
        return None;
    };

    let text = extract_mcp_text(&raw);

    Some(McpToolOutput { tool, text, raw })
}

fn mcp_tool_for_card(card_type: &CardType, input: &str) -> Option<(String, serde_json::Value)> {
    match card_type {
        CardType::Meal => {
            let mut payload = serde_json::json!({ "input": input });
            if let Some(meal_type) = infer_meal_type(input) {
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("meal_type".to_string(), serde_json::json!(meal_type));
                }
            }
            Some(("log_meal_quick".to_string(), payload))
        }
        CardType::DeliveryOrder => Some((
            "advise_order".to_string(),
            serde_json::json!({ "stt_text": input, "format": "text" }),
        )),
        CardType::MediaAdd => {
            let title = parse_media_title(input);
            if title.is_empty() {
                return None;
            }
            let mut payload = serde_json::json!({
                "title": title,
                "type": infer_media_type(input),
            });
            if let Some(rating) = parse_media_rating(input) {
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("rating".to_string(), serde_json::json!(rating));
                }
            }
            Some(("media_add".to_string(), payload))
        }
        _ => None,
    }
}

fn infer_meal_type(input: &str) -> Option<&'static str> {
    let lower = input.to_lowercase();
    if lower.contains("breakfast") {
        Some("breakfast")
    } else if lower.contains("lunch") {
        Some("lunch")
    } else if lower.contains("dinner") {
        Some("dinner")
    } else if lower.contains("snack") {
        Some("snack")
    } else {
        None
    }
}

fn parse_media_title(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|word| {
            let lower = word.to_lowercase();
            !matches!(
                lower.as_str(),
                "movie"
                    | "film"
                    | "show"
                    | "series"
                    | "tv"
                    | "anime"
                    | "game"
                    | "book"
                    | "watched"
                    | "played"
                    | "read"
                    | "rating"
            )
        })
        .filter(|word| !word.parse::<u8>().map(|n| n <= 10).unwrap_or(false))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_media_rating(input: &str) -> Option<u8> {
    let lower = input.to_lowercase();
    let rating_re = Regex::new(r"(\\d+)\\s*(?:/\\s*10)?").ok()?;
    rating_re
        .captures(&lower)
        .and_then(|c| c.get(1)?.as_str().parse::<u8>().ok())
        .filter(|&rating| rating <= 10)
}

fn infer_media_type(input: &str) -> &'static str {
    let lower = input.to_lowercase();
    if lower.contains("anime") {
        "anime"
    } else if lower.contains("animation") {
        "animation"
    } else if lower.contains("comic") {
        "comic"
    } else if lower.contains("youtube") {
        "youtube"
    } else if lower.contains("book") || lower.contains("read") {
        "book"
    } else if lower.contains("game") || lower.contains("played") {
        "game"
    } else if lower.contains("show") || lower.contains("series") || lower.contains("tv") {
        "tv"
    } else {
        "film"
    }
}

fn extract_mcp_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    if let Some(text) = value.get("text").and_then(|value| value.as_str()) {
        return Some(text.to_string());
    }

    let content = value.get("content")?;
    if let Some(array) = content.as_array() {
        let mut parts = Vec::new();
        for item in array {
            if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
                parts.push(text.to_string());
            }
        }
        if !parts.is_empty() {
            return Some(parts.join("\n"));
        }
    } else if let Some(text) = content.get("text").and_then(|value| value.as_str()) {
        return Some(text.to_string());
    }

    None
}

fn apply_mcp_output(enriched: &mut EnrichedData, output: McpToolOutput, input: &str) {
    let text = output.text.unwrap_or_else(|| output.raw.to_string());
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }

    if enriched.summary.is_none() {
        enriched.summary = Some(truncate_summary(trimmed, 160));
    }

    let section = ExpandedSection {
        title: format!("life-mcp ({})", output.tool),
        body: text,
    };

    let expanded = enriched
        .expanded
        .get_or_insert_with(|| base_expanded(input));
    expanded.sections.push(section);
}

fn truncate_summary(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or(text);
    if line.chars().count() <= max {
        return line.to_string();
    }
    let truncated: String = line.chars().take(max).collect();
    format!("{truncated}...")
}

pub(crate) fn detect_intent(input: &str) -> (CardType, DomainId, String) {
    let lower = input.to_lowercase();

    let nutrition_keywords = [
        "ate",
        "had",
        "breakfast",
        "lunch",
        "dinner",
        "snack",
        "meal",
        "calories",
        "omelette",
        "eggs",
        "food",
    ];
    if nutrition_keywords.iter().any(|kw| lower.contains(kw)) {
        return (CardType::Meal, DomainId::Nutrition, "🍽️".to_string());
    }

    let delivery_keywords = [
        "order",
        "delivery",
        "shift",
        "doordash",
        "uber",
        "grubhub",
        "instacart",
        "took",
        "declined",
    ];
    if delivery_keywords.iter().any(|kw| lower.contains(kw)) {
        return (
            CardType::DeliveryOrder,
            DomainId::Delivery,
            "🚗".to_string(),
        );
    }

    let media_keywords = [
        "watched", "watching", "watch", "movie", "show", "anime", "film", "played", "game", "read",
        "book",
    ];
    if media_keywords.iter().any(|kw| lower.contains(kw)) {
        return (CardType::MediaAdd, DomainId::Media, "🎬".to_string());
    }

    let code_keywords = [
        "fix",
        "implement",
        "bug",
        "refactor",
        "add feature",
        "create",
        "update",
        "delete",
        "modify",
        "change",
        "write code",
        "code task",
        "programming",
    ];
    let tech_terms = [
        "function",
        "component",
        "file",
        "module",
        "class",
        "api",
        "endpoint",
        "database",
        "query",
        "rust",
        "react",
        "typescript",
        "swift",
        ".rs",
        ".ts",
        ".tsx",
        ".swift",
    ];
    if code_keywords.iter().any(|kw| lower.contains(kw))
        && tech_terms.iter().any(|term| lower.contains(term))
    {
        return (CardType::CodeTask, DomainId::General, "💻".to_string());
    }

    if lower.ends_with('?') || lower.starts_with("what ") || lower.starts_with("how ") {
        return (CardType::Query, DomainId::General, "🔍".to_string());
    }

    if lower.contains("thought")
        || lower.contains("thinking")
        || lower.contains("think")
        || lower.contains("idea")
        || lower.contains("feeling")
    {
        return (CardType::Thought, DomainId::General, "💭".to_string());
    }

    (CardType::Generic, DomainId::General, "📝".to_string())
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let mut iter = s.chars();
    let truncated: String = iter.by_ref().take(max).collect();

    if truncated.chars().count() < s.chars().count() {
        truncated
    } else {
        s.to_string()
    }
}

async fn handle_nutrition(
    card_id: &str,
    input: &str,
    occurred_at: &str,
    workspace_path: &str,
    obsidian_root: Option<&str>,
    clarification: Option<&str>,
    obsidian: &ObsidianIO,
    cards: &Arc<Mutex<HashMap<String, StreamCard>>>,
    emitter: &Option<tauri::AppHandle>,
    event_sink: &Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
) -> Result<Option<EnrichedData>, String> {
    let root = obsidian
        .resolve_root_path(workspace_path, obsidian_root)
        .map_err(|err| err.to_string())?;
    let handler = NutritionHandler::new(&root.to_string_lossy());
    let processed = handler.process(input, occurred_at).await?;

    if processed.foods.is_empty() {
        match clarification {
            Some("skip") => {
                return Ok(Some(EnrichedData {
                    title: processed.title,
                    subtitle: processed.subtitle,
                    summary: Some("Logged without nutrition details.".to_string()),
                    stats: None,
                    entities: None,
                    image: None,
                    expanded: Some(base_expanded(input)),
                    image_lookup: None,
                }));
            }
            Some("add") | Some("photo") => {
                return Ok(Some(EnrichedData {
                    title: processed.title,
                    subtitle: processed.subtitle,
                    summary: Some("Needs manual nutrition entry.".to_string()),
                    stats: None,
                    entities: None,
                    image: Some(CardImage {
                        url: None,
                        status: ImageStatus::UploadPrompt,
                        source: None,
                    }),
                    expanded: Some(base_expanded(input)),
                    image_lookup: None,
                }));
            }
            _ => {
                request_clarification(
                    card_id,
                    "I couldn't find that food. What would you like to do?",
                    vec![
                        ClarificationOption {
                            id: "add".into(),
                            label: "Add new food".into(),
                            emoji: Some("➕".into()),
                        },
                        ClarificationOption {
                            id: "photo".into(),
                            label: "Upload photo".into(),
                            emoji: Some("📷".into()),
                        },
                        ClarificationOption {
                            id: "skip".into(),
                            label: "Log without nutrition".into(),
                            emoji: Some("⏭️".into()),
                        },
                    ],
                    cards,
                    emitter,
                    event_sink,
                )
                .await;
                return Ok(None);
            }
        }
    }

    Ok(Some(EnrichedData {
        title: processed.title,
        subtitle: processed.subtitle,
        summary: None,
        stats: processed.stats,
        entities: processed.entities,
        image: Some(CardImage {
            url: None,
            status: ImageStatus::UploadPrompt,
            source: None,
        }),
        expanded: Some(ExpandedContent {
            original_input: Some(input.to_string()),
            sections: Vec::new(),
            entity_links: None,
            actions: Vec::new(),
        }),
        image_lookup: None,
    }))
}

async fn request_clarification(
    card_id: &str,
    message: &str,
    options: Vec<ClarificationOption>,
    cards: &Arc<Mutex<HashMap<String, StreamCard>>>,
    emitter: &Option<tauri::AppHandle>,
    event_sink: &Option<Arc<dyn Fn(LifeStreamEvent) + Send + Sync>>,
) {
    let _ = emit_patch(
        card_id,
        StreamCardPatch {
            state: Some(CardState::AwaitingInput),
            processing_step: Some(message.to_string()),
            clarification_options: Some(options),
            ..Default::default()
        },
        cards,
        emitter,
        event_sink,
    )
    .await;
}

async fn handle_generic(input: &str) -> Result<EnrichedData, String> {
    Ok(EnrichedData {
        title: truncate(input, 50),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(base_expanded(input)),
        image_lookup: None,
    })
}

async fn handle_delivery(
    input: &str,
    occurred_at: &str,
    workspace_path: &str,
    obsidian_root: Option<&str>,
    obsidian: &ObsidianIO,
) -> Result<EnrichedData, String> {
    let root = obsidian
        .resolve_root_path(workspace_path, obsidian_root)
        .map_err(|err| err.to_string())?;
    let handler = DeliveryHandler::new(&root.to_string_lossy());
    let processed = handler.process(input, occurred_at).await?;

    let entity_links = processed.entities.as_ref().map(|entities| {
        entities
            .iter()
            .map(|entity| EntityLink {
                name: entity.name.clone(),
                path: entity
                    .link
                    .clone()
                    .unwrap_or_else(|| format!("[[Entities/Delivery/{}]]", entity.name)),
                icon: Some("🚗".to_string()),
            })
            .collect::<Vec<_>>()
    });

    Ok(EnrichedData {
        title: processed.title,
        subtitle: processed.subtitle,
        summary: processed.summary,
        stats: processed.stats,
        entities: processed.entities,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some(input.to_string()),
            sections: Vec::new(),
            entity_links,
            actions: Vec::new(),
        }),
        image_lookup: None,
    })
}

async fn handle_media(
    input: &str,
    occurred_at: &str,
    workspace_path: &str,
    obsidian_root: Option<&str>,
    obsidian: &ObsidianIO,
) -> Result<EnrichedData, String> {
    let root = obsidian
        .resolve_root_path(workspace_path, obsidian_root)
        .map_err(|err| err.to_string())?;
    let handler = MediaHandler::new(&root.to_string_lossy());
    let processed = handler.process(input, occurred_at).await?;
    let title = processed.title.clone();

    let entity_links = processed.entities.as_ref().map(|entities| {
        entities
            .iter()
            .map(|entity| EntityLink {
                name: entity.name.clone(),
                path: entity
                    .link
                    .clone()
                    .unwrap_or_else(|| format!("[[Media/{}]]", entity.name)),
                icon: Some("🎬".to_string()),
            })
            .collect::<Vec<_>>()
    });

    // Image lookup/fetching is intentionally disabled while we rebuild the pipeline from scratch.
    let _ = title;
    let image_lookup = None;
    let image = None;

    Ok(EnrichedData {
        title: processed.title,
        subtitle: processed.subtitle,
        summary: processed.summary,
        stats: processed.stats,
        entities: processed.entities,
        image,
        expanded: Some(ExpandedContent {
            original_input: Some(input.to_string()),
            sections: Vec::new(),
            entity_links,
            actions: Vec::new(),
        }),
        image_lookup,
    })
}

async fn handle_thought(input: &str) -> Result<EnrichedData, String> {
    let handler = ThoughtHandler::new();
    let processed = handler.process(input).await?;

    let expanded = ExpandedContent {
        original_input: Some(input.to_string()),
        sections: if input.trim().is_empty() {
            Vec::new()
        } else {
            vec![ExpandedSection {
                title: "Full Note".to_string(),
                body: input.to_string(),
            }]
        },
        entity_links: None,
        actions: Vec::new(),
    };

    Ok(EnrichedData {
        title: processed.title,
        subtitle: processed.subtitle,
        summary: processed.summary,
        stats: processed.stats,
        entities: processed.entities,
        image: None,
        expanded: Some(expanded),
        image_lookup: None,
    })
}

async fn handle_query(input: &str) -> Result<EnrichedData, String> {
    let handler = QueryHandler::new();
    let processed = handler.process(input).await?;

    Ok(EnrichedData {
        title: processed.title,
        subtitle: processed.subtitle,
        summary: processed.summary,
        stats: processed.stats,
        entities: processed.entities,
        image: None,
        expanded: Some(base_expanded(input)),
        image_lookup: None,
    })
}

async fn handle_code_task(input: &str) -> Result<EnrichedData, String> {
    let handler = CodeTaskHandler::new();
    let processed = handler.process(input).await?;

    Ok(EnrichedData {
        title: processed.title,
        subtitle: processed.subtitle,
        summary: processed.summary,
        stats: processed.stats,
        entities: processed.entities,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some(input.to_string()),
            sections: Vec::new(),
            entity_links: None,
            actions: vec![
                CardAction {
                    id: "view_thread".to_string(),
                    label: "View Thread".to_string(),
                    icon: Some("🔗".to_string()),
                    style: Some("primary".to_string()),
                },
                CardAction {
                    id: "cancel".to_string(),
                    label: "Cancel".to_string(),
                    icon: Some("✕".to_string()),
                    style: Some("danger".to_string()),
                },
            ],
        }),
        image_lookup: None,
    })
}

fn base_expanded(input: &str) -> ExpandedContent {
    ExpandedContent {
        original_input: Some(input.to_string()),
        sections: Vec::new(),
        entity_links: None,
        actions: Vec::new(),
    }
}

pub(crate) struct EnrichedData {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) stats: Option<HashMap<String, CardStatValue>>,
    pub(crate) entities: Option<Vec<EntityRef>>,
    pub(crate) image: Option<CardImage>,
    pub(crate) expanded: Option<ExpandedContent>,
    pub(crate) image_lookup: Option<ImageLookup>,
}

const DEFAULT_VISIBLE_RIGHT_COUNT: usize = 3;
const DEFAULT_TOP_LINK_LIMIT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CausalFrameProfile {
    CauseEffect,
    ActionReward,
    StatementWhy,
    QuestionResponse,
}

fn causal_frame_profile(card_type: &CardType) -> CausalFrameProfile {
    match card_type {
        CardType::DeliveryOrder | CardType::DeliverySession | CardType::Meal => {
            CausalFrameProfile::ActionReward
        }
        CardType::Query => CausalFrameProfile::QuestionResponse,
        CardType::Thought | CardType::MediaAdd | CardType::Music => {
            CausalFrameProfile::StatementWhy
        }
        _ => CausalFrameProfile::CauseEffect,
    }
}

fn resolve_causal_frame_profile(
    card_type: &CardType,
    input: &str,
    enriched: &EnrichedData,
) -> CausalFrameProfile {
    let explicit = causal_frame_profile(card_type);
    if explicit != CausalFrameProfile::CauseEffect {
        return explicit;
    }

    if looks_like_question_input(input) {
        return CausalFrameProfile::QuestionResponse;
    }

    if looks_like_statement_or_claim_input(input) {
        return CausalFrameProfile::StatementWhy;
    }

    if expanded_contains_markdown_headings(enriched) {
        return CausalFrameProfile::StatementWhy;
    }

    CausalFrameProfile::CauseEffect
}

fn infer_semantic_mode(frame: CausalFrameProfile) -> CausalSemanticMode {
    match frame {
        CausalFrameProfile::CauseEffect => CausalSemanticMode::CauseEffect,
        CausalFrameProfile::ActionReward => CausalSemanticMode::ActionReward,
        CausalFrameProfile::StatementWhy => CausalSemanticMode::StatementWhy,
        CausalFrameProfile::QuestionResponse => CausalSemanticMode::QuestionResponse,
    }
}

pub(crate) fn build_causal_content(
    card_id: &str,
    card_type: &CardType,
    input: &str,
    occurred_at: &str,
    enriched: &EnrichedData,
) -> CausalCardContent {
    let frame = resolve_causal_frame_profile(card_type, input, enriched);
    let semantic_mode = infer_semantic_mode(frame);
    let left_node_id = format!("{card_id}:left:0");
    let left_text = if input.trim().is_empty() {
        enriched.title.clone()
    } else {
        input.trim().to_string()
    };
    let left_parts = summarize_node_parts(&left_text, Some(enriched.title.as_str()), 88, 4);
    let left_headline = derive_node_headline(
        Some(left_parts.title.as_str()),
        Some(enriched.title.as_str()),
        left_text.as_str(),
        118,
    );
    let left_summary_line = derive_summary_line(
        left_parts
            .bullets
            .first()
            .map(|value| value.as_str())
            .or_else(|| Some(left_text.as_str())),
        left_headline.as_str(),
        130,
    );
    let left_entity = enriched
        .entities
        .as_ref()
        .and_then(|entities| entities.first())
        .cloned();

    let left_nodes = vec![CausalNode {
        id: left_node_id.clone(),
        text: left_text,
        headline: Some(left_headline.clone()),
        summary_line: left_summary_line.clone(),
        title: Some(left_parts.title),
        bullets: (!left_parts.bullets.is_empty()).then_some(left_parts.bullets),
        details: Some(input.trim().to_string()).filter(|value| !value.is_empty()),
        role: Some(infer_left_node_role(frame, card_type, input)),
        rank: Some(1),
        group_type: Some(CausalGroupType::Primary),
        is_image_applicable: true,
        image: None,
        entity: left_entity,
        occurred_at: Some(occurred_at.to_string()),
    }];

    let mut right_nodes = collect_right_nodes(card_id, frame, enriched);
    polish_semantic_nodes(&mut right_nodes, frame);

    if right_nodes.is_empty() {
        let fallback_text = enriched
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| truncate_summary(value, 200))
            .or_else(|| {
                enriched
                    .subtitle
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
            })
            .unwrap_or_else(|| enriched.title.clone());
        let fallback_parts = summarize_node_parts(
            fallback_text.as_str(),
            Some(enriched.title.as_str()),
            100,
            3,
        );
        let fallback_headline = derive_node_headline(
            Some(fallback_parts.title.as_str()),
            Some(enriched.title.as_str()),
            fallback_text.as_str(),
            118,
        );
        let fallback_summary = derive_summary_line(
            fallback_parts
                .bullets
                .first()
                .map(|value| value.as_str())
                .or_else(|| Some(fallback_text.as_str())),
            fallback_headline.as_str(),
            130,
        );

        right_nodes.push(CausalNode {
            id: format!("{card_id}:right:0"),
            text: fallback_text.clone(),
            headline: Some(fallback_headline),
            summary_line: fallback_summary,
            title: Some(fallback_parts.title),
            bullets: (!fallback_parts.bullets.is_empty()).then_some(fallback_parts.bullets),
            details: Some(fallback_text).filter(|value| !value.trim().is_empty()),
            role: Some(infer_right_node_role(frame)),
            rank: Some(1),
            group_type: Some(CausalGroupType::Primary),
            is_image_applicable: false,
            image: None,
            entity: None,
            occurred_at: None,
        });
    }

    if let Some(image) = enriched.image.clone() {
        if let Some(node) = right_nodes.first_mut() {
            if node.image.is_none() {
                node.image = Some(image);
                node.is_image_applicable = true;
            }
        }
    }

    if let Some(entity) = enriched
        .entities
        .as_ref()
        .and_then(|entities| entities.first())
        .cloned()
    {
        if let Some(node) = right_nodes.first_mut() {
            if node.entity.is_none() {
                node.entity = Some(entity);
            }
        }
    }

    let links = right_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| CausalLink {
            id: Some(format!("{card_id}:link:{index}")),
            from_id: left_node_id.clone(),
            to_id: node.id.clone(),
            label: None,
            strength: Some(if index < DEFAULT_TOP_LINK_LIMIT {
                1.0
            } else {
                0.55
            }),
        })
        .collect::<Vec<_>>();

    let has_dense_right_nodes = right_nodes.len() > DEFAULT_VISIBLE_RIGHT_COUNT;
    let has_dense_links = links.len() > DEFAULT_TOP_LINK_LIMIT;
    let layout = if has_dense_right_nodes || has_dense_links {
        Some(CausalLayoutState {
            visible_right_count: has_dense_right_nodes
                .then_some(DEFAULT_VISIBLE_RIGHT_COUNT as u32),
            top_link_limit: has_dense_links.then_some(DEFAULT_TOP_LINK_LIMIT as u32),
            expanded: has_dense_right_nodes.then_some(false),
        })
    } else {
        None
    };

    let overflow_count = right_nodes
        .len()
        .checked_sub(DEFAULT_VISIBLE_RIGHT_COUNT)
        .map(|count| count as u32)
        .filter(|count| *count > 0);
    let compaction = Some(CausalCompactionState {
        enabled: overflow_count.is_some(),
        threshold: DEFAULT_VISIBLE_RIGHT_COUNT as u32,
        overflow_count,
    });

    CausalCardContent {
        left_nodes,
        right_nodes,
        links,
        layout,
        semantic_mode: Some(semantic_mode),
        compaction,
        transcript_source: Some(CausalTranscriptSource::Both),
    }
}

fn build_primary_right_node(
    id: String,
    text: String,
    title: Option<String>,
    bullets: Option<Vec<String>>,
    details: Option<String>,
    role: CausalNodeRole,
    rank: usize,
) -> CausalNode {
    let headline = derive_node_headline(title.as_deref(), None, text.as_str(), 120);
    let summary_line = derive_summary_line(
        bullets
            .as_ref()
            .and_then(|items| items.first())
            .map(|value| value.as_str())
            .or_else(|| details.as_deref())
            .or_else(|| Some(text.as_str())),
        headline.as_str(),
        128,
    );

    CausalNode {
        id,
        text,
        headline: Some(headline),
        summary_line,
        title,
        bullets,
        details,
        role: Some(role),
        rank: Some((rank + 1) as u32),
        group_type: Some(CausalGroupType::Primary),
        is_image_applicable: false,
        image: None,
        entity: None,
        occurred_at: None,
    }
}

fn collect_right_nodes(
    card_id: &str,
    frame: CausalFrameProfile,
    enriched: &EnrichedData,
) -> Vec<CausalNode> {
    let mut right_nodes = Vec::new();
    let role = infer_right_node_role(frame);
    let mut index = 0usize;

    if let Some(expanded) = &enriched.expanded {
        for section in &expanded.sections {
            let title = section.title.trim();
            let body = section.body.trim();
            if title.is_empty() && body.is_empty() {
                continue;
            }

            if title.eq_ignore_ascii_case("completed orders") {
                for row in extract_markdown_table_rows(body) {
                    let parts =
                        summarize_node_parts(row.as_str(), Some("Delivery outcome"), 100, 3);
                    right_nodes.push(build_primary_right_node(
                        format!("{card_id}:right:{index}"),
                        row,
                        Some(parts.title),
                        (!parts.bullets.is_empty()).then_some(parts.bullets),
                        Some(body.to_string()).filter(|value| !value.trim().is_empty()),
                        CausalNodeRole::Reward,
                        index,
                    ));
                    index += 1;
                }
                continue;
            }

            if matches!(
                frame,
                CausalFrameProfile::StatementWhy | CausalFrameProfile::QuestionResponse
            ) {
                let grouped_nodes = extract_claim_response_node_groups(body);
                if !grouped_nodes.is_empty() {
                    for group in grouped_nodes {
                        right_nodes.push(build_primary_right_node(
                            format!("{card_id}:right:{index}"),
                            group.text,
                            Some(group.title),
                            (!group.bullets.is_empty()).then_some(group.bullets),
                            group.details,
                            role.clone(),
                            index,
                        ));
                        index += 1;
                    }
                    continue;
                }

                let emphasized_nodes = extract_emphasized_claim_lines(body);
                if !emphasized_nodes.is_empty() {
                    for item in emphasized_nodes {
                        let parts = summarize_node_parts(item.as_str(), Some(title), 108, 3);
                        right_nodes.push(build_primary_right_node(
                            format!("{card_id}:right:{index}"),
                            item.clone(),
                            Some(parts.title),
                            (!parts.bullets.is_empty()).then_some(parts.bullets),
                            Some(item).filter(|value| !value.trim().is_empty()),
                            role.clone(),
                            index,
                        ));
                        index += 1;
                    }
                    continue;
                }
            } else {
                let bullet_items = extract_bullet_lines(body);
                if bullet_items.len() > 1 {
                    for item in bullet_items {
                        let combined_text = if title.is_empty() {
                            item.clone()
                        } else {
                            format!("{title}: {item}")
                        };
                        let parts =
                            summarize_node_parts(combined_text.as_str(), Some(title), 108, 3);
                        right_nodes.push(build_primary_right_node(
                            format!("{card_id}:right:{index}"),
                            combined_text,
                            Some(parts.title),
                            (!parts.bullets.is_empty()).then_some(parts.bullets),
                            Some(body.to_string()).filter(|value| !value.trim().is_empty()),
                            role.clone(),
                            index,
                        ));
                        index += 1;
                    }
                    continue;
                }
            }

            let summary = summarize_section_body(body);
            if !summary.is_empty() {
                let include_title_prefix = !title.is_empty()
                    && !title.eq_ignore_ascii_case("codex response")
                    && !title.eq_ignore_ascii_case("response");
                let text = if include_title_prefix {
                    format!("{title}: {summary}")
                } else {
                    summary
                };
                let parts = summarize_node_parts(text.as_str(), Some(title), 108, 3);
                right_nodes.push(build_primary_right_node(
                    format!("{card_id}:right:{index}"),
                    text.clone(),
                    Some(parts.title),
                    (!parts.bullets.is_empty()).then_some(parts.bullets),
                    Some(text).filter(|value| !value.trim().is_empty()),
                    role.clone(),
                    index,
                ));
                index += 1;
            }
        }
    }

    normalize_causal_node_titles(&mut right_nodes);
    right_nodes
}

fn infer_left_node_role(
    frame: CausalFrameProfile,
    card_type: &CardType,
    input: &str,
) -> CausalNodeRole {
    match frame {
        CausalFrameProfile::ActionReward => CausalNodeRole::Action,
        CausalFrameProfile::QuestionResponse => CausalNodeRole::Question,
        CausalFrameProfile::StatementWhy => {
            if matches!(card_type, CardType::Query) || input.trim().contains('?') {
                CausalNodeRole::Question
            } else {
                CausalNodeRole::Cause
            }
        }
        CausalFrameProfile::CauseEffect => CausalNodeRole::Cause,
    }
}

fn infer_right_node_role(frame: CausalFrameProfile) -> CausalNodeRole {
    match frame {
        CausalFrameProfile::ActionReward => CausalNodeRole::Reward,
        CausalFrameProfile::StatementWhy | CausalFrameProfile::QuestionResponse => {
            CausalNodeRole::Response
        }
        CausalFrameProfile::CauseEffect => CausalNodeRole::Effect,
    }
}

fn looks_like_question_input(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains('?') {
        return true;
    }

    let lower = trimmed.to_lowercase();
    [
        "why",
        "how",
        "what if",
        "would it",
        "could it",
        "do you think",
        "should i",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn looks_like_statement_or_claim_input(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_lowercase();
    [
        "i think",
        "i feel",
        "i believe",
        "favorite",
        "should",
        "love",
        "hate",
        "best",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn expanded_contains_markdown_headings(enriched: &EnrichedData) -> bool {
    enriched
        .expanded
        .as_ref()
        .map(|expanded| {
            expanded.sections.iter().any(|section| {
                section
                    .body
                    .lines()
                    .any(|line| line.trim_start().starts_with('#'))
            })
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
struct ParsedNodeParts {
    title: String,
    bullets: Vec<String>,
}

#[derive(Debug, Clone)]
struct ClaimResponseNodeGroup {
    title: String,
    bullets: Vec<String>,
    text: String,
    details: Option<String>,
}

fn semantic_right_fallback(frame: CausalFrameProfile) -> &'static str {
    match frame {
        CausalFrameProfile::CauseEffect => "Effect",
        CausalFrameProfile::ActionReward => "Outcome",
        CausalFrameProfile::StatementWhy => "Why it matters",
        CausalFrameProfile::QuestionResponse => "Answer",
    }
}

fn semantic_headline_key(value: &str) -> String {
    value
        .to_lowercase()
        .replace(['’', '\''], "")
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_generic_node_title(value: &str) -> bool {
    let normalized = semantic_headline_key(value);
    matches!(
        normalized.as_str(),
        "response"
            | "cause"
            | "effect"
            | "action"
            | "reward"
            | "question"
            | "thought"
            | "note"
            | "codex response"
            | "details"
    )
}

fn is_weak_semantic_fragment(value: &str) -> bool {
    let normalized = semantic_headline_key(value);
    if normalized.is_empty() {
        return true;
    }
    let word_count = normalized.split_whitespace().count();
    if word_count < 3 {
        return true;
    }
    normalized.ends_with(" if")
        || normalized.ends_with(" if it")
        || normalized.ends_with(" because")
        || normalized.ends_with(" and")
        || normalized.ends_with(" but")
        || normalized.ends_with(" so")
        || normalized.ends_with(" youd gain")
        || normalized.ends_with(" you d gain")
        || normalized.ends_with(" youd lose")
        || normalized.ends_with(" you d lose")
        || normalized.ends_with(" you gain")
        || normalized.ends_with(" you lose")
        || normalized.ends_with(" trade off")
}

fn is_meta_boilerplate_headline(value: &str) -> bool {
    let normalized = semantic_headline_key(value);
    if normalized.is_empty() {
        return true;
    }

    [
        "jmwillis",
        "codex is responding",
        "status online",
        "next test idea",
        "send something short",
        "this week s criminal",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn rewrite_fragmentary_headline(title: &str, bullets: Option<&[String]>) -> String {
    let mut base = title.trim().trim_end_matches(':').trim().to_string();
    if base.is_empty() {
        return base;
    }

    if is_weak_semantic_fragment(base.as_str()) {
        if let Some(first_bullet) = bullets.and_then(|items| items.first()) {
            let first_bullet = first_bullet.trim();
            if !first_bullet.is_empty() {
                base = format!("{base} — {first_bullet}");
            }
        } else {
            let normalized = semantic_headline_key(base.as_str());
            if normalized.ends_with("youd gain")
                || normalized.ends_with("you d gain")
                || normalized.ends_with("you gain")
            {
                base = format!("{base} — key gains");
            } else if normalized.ends_with("youd lose")
                || normalized.ends_with("you d lose")
                || normalized.ends_with("you lose")
            {
                base = format!("{base} — key losses");
            } else if normalized.ends_with("trade off") {
                base = format!("{base} — tradeoff");
            } else if !base.ends_with(" details") {
                base = format!("{base} details");
            }
        }
    }

    normalize_title(base.as_str(), 120)
}

fn semantic_quality_score(headline: &str, summary: Option<&str>) -> i32 {
    let mut score = 100;
    let normalized_headline = semantic_headline_key(headline);
    if normalized_headline.is_empty() {
        score -= 70;
    }
    if is_weak_semantic_fragment(headline) {
        score -= 35;
    }
    if is_meta_boilerplate_headline(headline) {
        score -= 55;
    }
    if normalized_headline.split_whitespace().count() < 4 {
        score -= 12;
    }

    if let Some(summary) = summary {
        let normalized_summary = semantic_headline_key(summary);
        if normalized_summary == normalized_headline {
            score -= 20;
        }
    }

    score
}

fn derive_node_headline(
    preferred_title: Option<&str>,
    fallback_title: Option<&str>,
    source_text: &str,
    limit: usize,
) -> String {
    let preferred = preferred_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| !is_generic_node_title(value))
        .map(|value| normalize_title(value, limit));
    if let Some(value) = preferred {
        if !is_weak_semantic_fragment(value.as_str()) {
            return value;
        }
    }

    let sentence = first_sentence(source_text)
        .map(|value| normalize_title(value.as_str(), limit))
        .filter(|value| !value.is_empty())
        .filter(|value| !is_generic_node_title(value.as_str()));
    if let Some(value) = sentence {
        if !is_weak_semantic_fragment(value.as_str()) {
            return value;
        }
    }

    fallback_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| normalize_title(value, limit))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_title(source_text, limit))
}

fn derive_summary_line(source: Option<&str>, headline: &str, limit: usize) -> Option<String> {
    let source = source.map(str::trim).filter(|value| !value.is_empty())?;
    let normalized_headline = semantic_headline_key(headline);
    let mut candidate = first_sentence(source)
        .unwrap_or_else(|| source.to_string())
        .replace('\n', " ")
        .trim()
        .to_string();
    if candidate.is_empty() {
        return None;
    }
    candidate = truncate_summary(candidate.as_str(), limit);
    let normalized_candidate = semantic_headline_key(candidate.as_str());
    if normalized_candidate.is_empty() || normalized_candidate == normalized_headline {
        return None;
    }
    if normalized_candidate.starts_with(normalized_headline.as_str())
        || normalized_headline.starts_with(normalized_candidate.as_str())
    {
        return None;
    }
    Some(candidate)
}

fn polish_semantic_nodes(nodes: &mut Vec<CausalNode>, frame: CausalFrameProfile) {
    if nodes.is_empty() {
        return;
    }

    let original_nodes = nodes.clone();
    let mut seen = HashSet::new();
    let fallback = semantic_right_fallback(frame).to_string();
    let mut polished = Vec::new();
    let mut low_quality_detected = false;

    for (index, mut node) in nodes.drain(..).enumerate() {
        let preferred_title = node
            .headline
            .as_deref()
            .or(node.title.as_deref())
            .filter(|value| !value.trim().is_empty());
        let headline = derive_node_headline(
            preferred_title,
            Some(fallback.as_str()),
            node.text.as_str(),
            120,
        );

        // one regenerate pass for weak or fragmentary nodes
        let regenerated_headline = if is_weak_semantic_fragment(headline.as_str()) {
            derive_node_headline(
                first_sentence(node.details.as_deref().unwrap_or(node.text.as_str())).as_deref(),
                Some(fallback.as_str()),
                node.details.as_deref().unwrap_or(node.text.as_str()),
                120,
            )
        } else {
            headline
        };

        let rescued_headline =
            rewrite_fragmentary_headline(regenerated_headline.as_str(), node.bullets.as_deref());

        if is_weak_semantic_fragment(rescued_headline.as_str()) {
            low_quality_detected = true;
            continue;
        }

        if is_meta_boilerplate_headline(rescued_headline.as_str()) {
            low_quality_detected = true;
            continue;
        }

        let summary_line = derive_summary_line(
            node.summary_line
                .as_deref()
                .or_else(|| {
                    node.bullets
                        .as_ref()
                        .and_then(|items| items.first())
                        .map(|value| value.as_str())
                })
                .or_else(|| node.details.as_deref())
                .or_else(|| Some(node.text.as_str())),
            rescued_headline.as_str(),
            128,
        );

        let quality = semantic_quality_score(rescued_headline.as_str(), summary_line.as_deref());
        if quality < 58 {
            low_quality_detected = true;
            continue;
        }

        let dedupe_key = format!(
            "{}|{}",
            semantic_headline_key(rescued_headline.as_str()),
            semantic_headline_key(summary_line.as_deref().unwrap_or(""))
        );
        if dedupe_key.trim_matches('|').is_empty() {
            low_quality_detected = true;
            continue;
        }
        if !seen.insert(dedupe_key) {
            continue;
        }

        node.headline = Some(rescued_headline);
        node.summary_line = summary_line;
        node.rank = Some((index + 1) as u32);
        node.group_type = Some(CausalGroupType::Primary);
        node.is_image_applicable = false;
        polished.push(node);
    }

    let should_attempt_rescue = matches!(
        frame,
        CausalFrameProfile::StatementWhy | CausalFrameProfile::QuestionResponse
    ) && (low_quality_detected
        || polished.len() < std::cmp::min(3, original_nodes.len()));

    if should_attempt_rescue {
        polished = rescue_low_quality_semantic_nodes(&original_nodes, frame, fallback.as_str());
    }

    if polished.is_empty() {
        if let Some(mut fallback_node) = original_nodes.first().cloned() {
            let fallback_headline = derive_node_headline(
                fallback_node.title.as_deref(),
                Some(fallback.as_str()),
                fallback_node.text.as_str(),
                120,
            );
            fallback_node.headline = Some(fallback_headline.clone());
            fallback_node.summary_line = derive_summary_line(
                fallback_node
                    .details
                    .as_deref()
                    .or(Some(fallback_node.text.as_str())),
                fallback_headline.as_str(),
                128,
            );
            fallback_node.rank = Some(1);
            fallback_node.group_type = Some(CausalGroupType::Primary);
            fallback_node.is_image_applicable = false;
            polished.push(fallback_node);
        }
    }

    *nodes = polished;
}

fn rescue_low_quality_semantic_nodes(
    original_nodes: &[CausalNode],
    frame: CausalFrameProfile,
    fallback: &str,
) -> Vec<CausalNode> {
    let mut rescued = Vec::new();
    let mut seen = HashSet::new();

    for (index, original) in original_nodes.iter().enumerate() {
        let details_source = original
            .details
            .as_deref()
            .unwrap_or(original.text.as_str());
        let detail_sentence = first_sentence(details_source);
        let preferred = original
            .title
            .as_deref()
            .or(original.headline.as_deref())
            .or(detail_sentence.as_deref())
            .unwrap_or(fallback);

        let rescued_headline = rewrite_fragmentary_headline(preferred, original.bullets.as_deref());
        if rescued_headline.is_empty()
            || is_weak_semantic_fragment(rescued_headline.as_str())
            || is_meta_boilerplate_headline(rescued_headline.as_str())
        {
            continue;
        }

        let summary_sentence = first_sentence(details_source);
        let summary_line = derive_summary_line(
            original
                .bullets
                .as_ref()
                .and_then(|items| items.get(1).map(String::as_str))
                .or(original.summary_line.as_deref())
                .or(summary_sentence.as_deref())
                .or(Some(details_source)),
            rescued_headline.as_str(),
            128,
        );

        let dedupe_key = format!(
            "{}|{}",
            semantic_headline_key(rescued_headline.as_str()),
            semantic_headline_key(summary_line.as_deref().unwrap_or("")),
        );
        if dedupe_key.trim_matches('|').is_empty() || !seen.insert(dedupe_key) {
            continue;
        }

        let mut node = original.clone();
        node.headline = Some(rescued_headline);
        node.summary_line = summary_line;
        node.rank = Some((index + 1) as u32);
        node.group_type = Some(CausalGroupType::Primary);
        node.is_image_applicable = false;
        rescued.push(node);
    }

    if rescued.is_empty() {
        return Vec::new();
    }

    if matches!(frame, CausalFrameProfile::QuestionResponse) && rescued.len() > 5 {
        rescued.truncate(5);
    }

    rescued
}

fn summarize_node_parts(
    text: &str,
    fallback_title: Option<&str>,
    title_limit: usize,
    bullet_limit: usize,
) -> ParsedNodeParts {
    let trimmed = text.trim();
    let mut bullets = extract_bullet_lines(trimmed);
    if bullets.is_empty() {
        bullets = split_into_claim_bullets(trimmed, bullet_limit);
    } else if bullets.len() > bullet_limit {
        bullets.truncate(bullet_limit);
    }

    let fallback = fallback_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Details");
    let mut title = first_sentence(trimmed)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    title = normalize_title(title.as_str(), title_limit);

    ParsedNodeParts { title, bullets }
}

fn extract_claim_response_node_groups(body: &str) -> Vec<ClaimResponseNodeGroup> {
    let mut groups: Vec<(Option<String>, Vec<String>)> = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    let push_current = |groups: &mut Vec<(Option<String>, Vec<String>)>,
                        current_heading: &mut Option<String>,
                        current_lines: &mut Vec<String>| {
        if current_heading.is_none() && current_lines.is_empty() {
            return;
        }
        groups.push((current_heading.take(), std::mem::take(current_lines)));
    };

    for raw_line in body.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "---" {
            push_current(&mut groups, &mut current_heading, &mut current_lines);
            continue;
        }
        if trimmed.starts_with('#') {
            push_current(&mut groups, &mut current_heading, &mut current_lines);
            let heading = trimmed
                .trim_start_matches('#')
                .trim()
                .trim_matches('*')
                .trim_matches('_')
                .trim()
                .to_string();
            if !heading.is_empty() {
                current_heading = Some(heading);
            }
            continue;
        }
        current_lines.push(trimmed.to_string());
    }
    push_current(&mut groups, &mut current_heading, &mut current_lines);

    let has_headings = groups.iter().any(|(heading, _)| heading.is_some());
    if has_headings {
        let mut preface_lines: Vec<String> = Vec::new();
        let mut normalized_groups: Vec<(Option<String>, Vec<String>)> = Vec::new();

        for (heading, lines) in groups {
            if heading.is_none() {
                preface_lines.extend(lines);
                continue;
            }
            normalized_groups.push((heading, lines));
        }

        if !preface_lines.is_empty() {
            if let Some((_, first_lines)) = normalized_groups.first_mut() {
                let mut combined = preface_lines;
                combined.extend(first_lines.clone());
                *first_lines = combined;
            }
        }

        groups = normalized_groups;
    }

    let mut output = Vec::new();
    for (heading, lines) in groups {
        let details = lines.join("\n");
        let line_text = lines.join(" ");
        let summary_source = if line_text.is_empty() {
            heading.clone().unwrap_or_default()
        } else {
            line_text.clone()
        };
        if summary_source.trim().is_empty() {
            continue;
        }

        let bullets = {
            let explicit = extract_bullet_lines(details.as_str());
            if explicit.is_empty() {
                split_into_claim_bullets(summary_source.as_str(), 4)
            } else {
                explicit.into_iter().take(4).collect::<Vec<_>>()
            }
        };

        let title_seed = heading
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| first_sentence(summary_source.as_str()).unwrap_or_default());
        let title = normalize_title(title_seed.as_str(), 112);
        if title.is_empty() {
            continue;
        }

        let text = if bullets.is_empty() {
            title.clone()
        } else {
            format!("{} — {}", title, bullets[0])
        };
        output.push(ClaimResponseNodeGroup {
            title,
            bullets,
            text,
            details: Some(details).filter(|value| !value.trim().is_empty()),
        });
    }

    output
}

fn normalize_causal_node_titles(nodes: &mut [CausalNode]) {
    for index in 0..nodes.len() {
        let Some(title) = nodes[index].title.clone() else {
            continue;
        };
        let trimmed = title.trim();
        if trimmed.is_empty() {
            continue;
        }

        let ends_with_fragment = trimmed.ends_with(':') || trimmed.ends_with(',');
        let too_short = trimmed
            .split_whitespace()
            .filter(|word| !word.trim().is_empty())
            .count()
            < 4;
        if !(ends_with_fragment || too_short) {
            continue;
        }

        let mut supplemental: Option<String> = None;
        if let Some(bullets) = nodes[index].bullets.as_mut() {
            if let Some(first_bullet) = bullets.first().cloned() {
                supplemental = Some(first_bullet.clone());
                bullets.remove(0);
            }
        }
        if supplemental.is_none() {
            supplemental = nodes
                .get(index + 1)
                .and_then(|next| next.title.clone())
                .or_else(|| {
                    nodes
                        .get(index + 1)
                        .map(|next| truncate_summary(next.text.as_str(), 70))
                });
        }

        if let Some(extra) = supplemental {
            let merged = format!(
                "{} {}",
                trimmed.trim_end_matches(':').trim_end_matches(',').trim(),
                extra.trim()
            );
            nodes[index].title = Some(normalize_title(merged.as_str(), 112));
            if nodes[index].text.trim().eq_ignore_ascii_case(trimmed) {
                nodes[index].text = merged;
            }
        }
    }
}

fn extract_emphasized_claim_lines(body: &str) -> Vec<String> {
    let mut claims = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("**") && trimmed.ends_with("**")) {
            continue;
        }

        let inner = trimmed
            .trim_start_matches("**")
            .trim_end_matches("**")
            .trim();
        if inner.is_empty() {
            continue;
        }

        let word_count = inner
            .split_whitespace()
            .filter(|value| !value.trim().is_empty())
            .count();
        if word_count < 4 {
            continue;
        }

        claims.push(truncate_summary(inner, 160));
    }

    claims
}

fn summarize_section_body(body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "---" {
            continue;
        }
        if trimmed.starts_with('|') {
            continue;
        }

        let cleaned = trimmed.trim_start_matches('#').trim();
        if cleaned.is_empty() {
            continue;
        }

        return truncate_summary(cleaned, 200);
    }

    truncate_summary(body.trim(), 200)
}

fn extract_markdown_table_rows(body: &str) -> Vec<String> {
    let mut rows = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }

        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .filter(|cell| !cell.is_empty())
            .collect::<Vec<_>>();

        if cells.is_empty() {
            continue;
        }
        if cells.iter().all(|cell| {
            cell.chars()
                .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
        }) {
            continue;
        }

        rows.push(
            cells
                .iter()
                .take(4)
                .copied()
                .collect::<Vec<_>>()
                .join(" • "),
        );
    }

    if rows.len() > 1 {
        let header = rows[0].to_lowercase();
        if header.contains("order")
            || header.contains("merchant")
            || header.contains("pickup")
            || header.contains("dropoff")
            || header.contains("payout")
        {
            rows.remove(0);
        }
    }

    rows
}

fn extract_bullet_lines(body: &str) -> Vec<String> {
    let mut bullets = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let bullet = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .map(|item| item.trim().to_string())
            .or_else(|| {
                let (prefix, rest) = trimmed.split_once(". ")?;
                if prefix.chars().all(|ch| ch.is_ascii_digit()) {
                    Some(rest.trim().to_string())
                } else {
                    None
                }
            });

        if let Some(item) = bullet {
            if !item.is_empty() {
                bullets.push(item);
            }
        }
    }

    bullets
}

fn split_into_claim_bullets(text: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return Vec::new();
    }
    let mut bullets = Vec::new();

    for segment in text
        .split('\n')
        .flat_map(|line| line.split(" — "))
        .flat_map(|line| line.split(" - "))
        .flat_map(|line| line.split("; "))
        .flat_map(|line| line.split(". "))
    {
        let cleaned = segment
            .trim()
            .trim_start_matches('-')
            .trim_start_matches('*')
            .trim();
        if cleaned.len() < 8 {
            continue;
        }
        let normalized = truncate_summary(cleaned, 140);
        if normalized.is_empty() {
            continue;
        }
        if bullets.iter().any(|existing| existing == &normalized) {
            continue;
        }
        bullets.push(normalized);
        if bullets.len() >= max {
            break;
        }
    }

    bullets
}

fn first_sentence(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let candidate = trimmed
            .split_terminator(['.', '!', '?'])
            .next()
            .map(str::trim)
            .unwrap_or(trimmed);
        if candidate.is_empty() {
            continue;
        }
        return Some(candidate.to_string());
    }
    None
}

fn normalize_title(value: &str, limit: usize) -> String {
    let trimmed = value
        .trim()
        .trim_matches('*')
        .trim_matches('_')
        .trim_start_matches('#')
        .trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut title = truncate_summary(trimmed, limit);
    if title.ends_with(':') && title.len() > 1 {
        title = title.trim_end_matches(':').trim().to_string();
    }
    if title.ends_with(',') && title.len() > 1 {
        title = title.trim_end_matches(',').trim().to_string();
    }
    title
}

pub(crate) fn apply_restructure_action(
    card_id: &str,
    causal: &CausalCardContent,
    action: CausalRestructureAction,
    source_node_ids: &[String],
    target_mode: Option<&str>,
) -> CausalCardContent {
    let mut next = causal.clone();

    match action {
        CausalRestructureAction::SplitCause => {
            split_cause_nodes(card_id, &mut next, source_node_ids);
            relink_causal_edges(card_id, &mut next);
        }
        CausalRestructureAction::MergeEffects => {
            merge_effect_nodes(card_id, &mut next, source_node_ids);
            relink_causal_edges(card_id, &mut next);
        }
        CausalRestructureAction::RelinkArrows => {
            relink_causal_edges(card_id, &mut next);
        }
        CausalRestructureAction::ReframeMode => {
            reframe_causal_roles(&mut next, target_mode);
        }
    }

    refresh_node_ranks(&mut next);
    next
}

fn refresh_node_ranks(causal: &mut CausalCardContent) {
    for (index, node) in causal.left_nodes.iter_mut().enumerate() {
        node.rank = Some((index + 1) as u32);
        if node.group_type.is_none() {
            node.group_type = Some(CausalGroupType::Primary);
        }
    }
    for (index, node) in causal.right_nodes.iter_mut().enumerate() {
        node.rank = Some((index + 1) as u32);
        if node.group_type.is_none() {
            node.group_type = Some(CausalGroupType::Primary);
        }
    }
    if let Some(compaction) = causal.compaction.as_mut() {
        compaction.overflow_count = causal
            .right_nodes
            .len()
            .checked_sub(compaction.threshold as usize)
            .map(|count| count as u32)
            .filter(|count| *count > 0);
        compaction.enabled = compaction.overflow_count.is_some();
    }
}

fn split_cause_nodes(card_id: &str, causal: &mut CausalCardContent, source_node_ids: &[String]) {
    if causal.left_nodes.is_empty() {
        return;
    }

    let target_index = source_node_ids
        .first()
        .and_then(|target| causal.left_nodes.iter().position(|node| node.id == *target))
        .unwrap_or(0);
    let source = causal.left_nodes.get(target_index).cloned();
    let Some(source) = source else {
        return;
    };

    let segments = split_cause_text(&source.text);
    if segments.len() < 2 {
        return;
    }

    let mut next_left_nodes = Vec::new();
    for (index, node) in causal.left_nodes.iter().enumerate() {
        if index != target_index {
            next_left_nodes.push(node.clone());
            continue;
        }

        for (segment_index, segment) in segments.iter().enumerate() {
            let headline = derive_node_headline(
                Some(segment.as_str()),
                source.title.as_deref(),
                segment.as_str(),
                118,
            );
            next_left_nodes.push(CausalNode {
                id: format!("{card_id}:left:{segment_index}"),
                text: segment.clone(),
                headline: Some(headline.clone()),
                summary_line: derive_summary_line(
                    source
                        .bullets
                        .as_ref()
                        .and_then(|items| items.get(segment_index))
                        .map(|value| value.as_str())
                        .or_else(|| Some(segment.as_str())),
                    headline.as_str(),
                    128,
                ),
                title: Some(normalize_title(segment.as_str(), 88)),
                bullets: Some(split_into_claim_bullets(segment.as_str(), 3))
                    .filter(|value| !value.is_empty()),
                details: source.details.clone().or_else(|| Some(source.text.clone())),
                role: source.role.clone(),
                rank: Some((segment_index + 1) as u32),
                group_type: Some(CausalGroupType::Primary),
                is_image_applicable: source.is_image_applicable,
                image: source.image.clone(),
                entity: source.entity.clone(),
                occurred_at: source.occurred_at.clone(),
            });
        }
    }

    if !next_left_nodes.is_empty() {
        causal.left_nodes = next_left_nodes;
    }
}

fn split_cause_text(text: &str) -> Vec<String> {
    let mut segments = Vec::new();

    for line in text
        .split(['\n', ';'])
        .flat_map(|chunk| chunk.split(". "))
        .flat_map(|chunk| chunk.split(" and "))
    {
        let normalized = line.trim();
        if normalized.is_empty() {
            continue;
        }
        segments.push(truncate_summary(normalized, 140));
    }

    if segments.len() > 4 {
        segments.truncate(4);
    }

    segments
}

fn merge_effect_nodes(card_id: &str, causal: &mut CausalCardContent, source_node_ids: &[String]) {
    if causal.right_nodes.len() < 2 {
        return;
    }

    let merge_indices = if source_node_ids.len() >= 2 {
        let mut indices = source_node_ids
            .iter()
            .filter_map(|id| causal.right_nodes.iter().position(|node| node.id == *id))
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
    } else {
        vec![0, 1]
    };

    if merge_indices.len() < 2 {
        return;
    }

    let first = merge_indices[0];
    let second = merge_indices[1];
    if first >= causal.right_nodes.len() || second >= causal.right_nodes.len() {
        return;
    }

    let first_node = causal.right_nodes[first].clone();
    let second_node = causal.right_nodes[second].clone();

    let merged_text = format!("{}\n• {}", first_node.text.trim(), second_node.text.trim());
    let merged_headline = derive_node_headline(
        first_node
            .headline
            .as_deref()
            .or(first_node.title.as_deref()),
        Some("Merged response"),
        merged_text.as_str(),
        120,
    );

    let merged_node = CausalNode {
        id: format!("{card_id}:right:merged"),
        text: truncate_summary(&merged_text, 260),
        headline: Some(merged_headline.clone()),
        summary_line: derive_summary_line(
            first_node
                .summary_line
                .as_deref()
                .or(second_node.summary_line.as_deref())
                .or_else(|| first_node.details.as_deref())
                .or_else(|| Some(merged_text.as_str())),
            merged_headline.as_str(),
            128,
        ),
        title: Some(normalize_title(
            first_node
                .title
                .as_deref()
                .unwrap_or(first_node.text.as_str()),
            108,
        )),
        bullets: {
            let mut merged_bullets = Vec::new();
            if let Some(first_bullets) = first_node.bullets.clone() {
                merged_bullets.extend(first_bullets);
            } else {
                merged_bullets.extend(split_into_claim_bullets(first_node.text.as_str(), 2));
            }
            if let Some(second_bullets) = second_node.bullets.clone() {
                merged_bullets.extend(second_bullets);
            } else {
                merged_bullets.extend(split_into_claim_bullets(second_node.text.as_str(), 2));
            }
            merged_bullets.truncate(4);
            (!merged_bullets.is_empty()).then_some(merged_bullets)
        },
        details: Some(merged_text),
        role: first_node.role.clone().or(second_node.role.clone()),
        rank: first_node
            .rank
            .or(second_node.rank)
            .or(Some((first + 1) as u32)),
        group_type: Some(CausalGroupType::Primary),
        is_image_applicable: first_node.is_image_applicable || second_node.is_image_applicable,
        image: first_node.image.clone().or(second_node.image.clone()),
        entity: first_node.entity.clone().or(second_node.entity.clone()),
        occurred_at: first_node
            .occurred_at
            .clone()
            .or(second_node.occurred_at.clone()),
    };

    let mut next_right_nodes = Vec::new();
    for (index, node) in causal.right_nodes.iter().enumerate() {
        if index == first {
            next_right_nodes.push(merged_node.clone());
            continue;
        }
        if index == second {
            continue;
        }
        next_right_nodes.push(node.clone());
    }
    causal.right_nodes = next_right_nodes;
}

fn relink_causal_edges(card_id: &str, causal: &mut CausalCardContent) {
    if causal.left_nodes.is_empty() || causal.right_nodes.is_empty() {
        causal.links.clear();
        return;
    }

    let mut links = Vec::new();
    let left_count = causal.left_nodes.len();
    for (right_index, right_node) in causal.right_nodes.iter().enumerate() {
        let left_index = right_index % left_count;
        let left_node = &causal.left_nodes[left_index];
        links.push(CausalLink {
            id: Some(format!("{card_id}:link:{right_index}")),
            from_id: left_node.id.clone(),
            to_id: right_node.id.clone(),
            label: None,
            strength: Some(if right_index < DEFAULT_TOP_LINK_LIMIT {
                1.0
            } else {
                0.55
            }),
        });
    }
    causal.links = links;
}

fn reframe_causal_roles(causal: &mut CausalCardContent, target_mode: Option<&str>) {
    let desired = target_mode.unwrap_or_else(|| {
        if causal
            .right_nodes
            .iter()
            .any(|node| node.role == Some(CausalNodeRole::Reward))
        {
            "cause_effect"
        } else {
            "action_reward"
        }
    });

    if desired.eq_ignore_ascii_case("cause_effect") {
        causal.semantic_mode = Some(CausalSemanticMode::CauseEffect);
        for node in &mut causal.left_nodes {
            node.role = match node.role.clone() {
                Some(CausalNodeRole::Action) => Some(CausalNodeRole::Cause),
                Some(CausalNodeRole::Question) => Some(CausalNodeRole::Cause),
                other => other,
            };
        }
        for node in &mut causal.right_nodes {
            node.role = match node.role.clone() {
                Some(CausalNodeRole::Reward) => Some(CausalNodeRole::Effect),
                Some(CausalNodeRole::Response) => Some(CausalNodeRole::Effect),
                other => other,
            };
        }
        return;
    }

    causal.semantic_mode = Some(CausalSemanticMode::ActionReward);
    for node in &mut causal.left_nodes {
        node.role = match node.role.clone() {
            Some(CausalNodeRole::Cause) => Some(CausalNodeRole::Action),
            Some(CausalNodeRole::Question) => Some(CausalNodeRole::Action),
            other => other,
        };
    }
    for node in &mut causal.right_nodes {
        node.role = match node.role.clone() {
            Some(CausalNodeRole::Effect) => Some(CausalNodeRole::Reward),
            Some(CausalNodeRole::Response) => Some(CausalNodeRole::Reward),
            other => other,
        };
    }
}

fn hydrate_card_images_from_catalog(
    card: &mut StreamCard,
    obsidian_root: &std::path::Path,
    catalog: &super::image_catalog::ImageCatalog,
) {
    if !is_image_ready(card.image.as_ref()) {
        let resolved = resolve_entity_for_card(card, None);
        if let Some(relative_path) = primary_asset_path_for_context(
            catalog,
            &resolved.entity_key,
            resolved.context_text.as_deref(),
        ) {
            card.image = Some(CardImage {
                url: Some(
                    absolute_from_relative(obsidian_root, &relative_path)
                        .to_string_lossy()
                        .to_string(),
                ),
                status: ImageStatus::Ready,
                source: Some("catalog".to_string()),
            });
        }
    }

    let mut node_ids = Vec::new();
    if let Some(causal) = card.causal.as_ref() {
        node_ids.extend(causal.left_nodes.iter().map(|node| node.id.clone()));
        node_ids.extend(causal.right_nodes.iter().map(|node| node.id.clone()));
    }

    for node_id in node_ids {
        let resolved = resolve_entity_for_card(card, Some(&node_id));
        let Some(relative_path) = primary_asset_path_for_context(
            catalog,
            &resolved.entity_key,
            resolved.context_text.as_deref(),
        ) else {
            continue;
        };
        let image = CardImage {
            url: Some(
                absolute_from_relative(obsidian_root, &relative_path)
                    .to_string_lossy()
                    .to_string(),
            ),
            status: ImageStatus::Ready,
            source: Some("catalog".to_string()),
        };

        if let Some(causal) = card.causal.as_mut() {
            if let Some(node) = causal
                .left_nodes
                .iter_mut()
                .chain(causal.right_nodes.iter_mut())
                .find(|node| node.id == node_id)
            {
                if !is_image_ready(node.image.as_ref()) {
                    node.image = Some(image);
                }
                if node.entity.is_none() {
                    node.entity = Some(EntityRef {
                        entity_type: resolved.entity_type.clone(),
                        id: None,
                        name: resolved.entity_name.clone(),
                        link: Some(entity_link_for(
                            resolved.entity_type.as_str(),
                            resolved.entity_name.as_str(),
                        )),
                    });
                }
            }
        }
    }
}

fn is_image_ready(image: Option<&CardImage>) -> bool {
    image.is_some_and(|value| value.status == ImageStatus::Ready && value.url.is_some())
}

fn auto_image_target_node_id(card: &StreamCard) -> Option<String> {
    let causal = card.causal.as_ref()?;
    if let Some(node) = causal.left_nodes.iter().find(|node| {
        node.is_image_applicable
            && !is_image_ready(node.image.as_ref())
            && node.group_type != Some(CausalGroupType::OverflowSummary)
    }) {
        return Some(node.id.clone());
    }

    causal
        .right_nodes
        .iter()
        .find(|node| {
            node.is_image_applicable
                && !is_image_ready(node.image.as_ref())
                && node.group_type != Some(CausalGroupType::OverflowSummary)
        })
        .map(|node| node.id.clone())
}

fn entity_link_for(entity_type: &str, entity_name: &str) -> String {
    let folder = match entity_type {
        "media" => "Entities/Media",
        "game" | "games" => "Entities/Media",
        "book" | "books" => "Entities/Media",
        "food" => "Entities/Food",
        "delivery" => "Entities/Delivery",
        "fitness" => "Entities/Fitness",
        "finance" => "Entities/Finance",
        "people" => "Entities/People",
        "geography" => "Entities/Topics",
        "youtube" => "Entities/YouTube",
        _ => "Entities/Notes",
    };
    format!("[[{}/{}]]", folder, entity_name)
}

async fn is_cancelled(card_id: &str, cancelled_cards: &Arc<Mutex<HashSet<String>>>) -> bool {
    let guard = cancelled_cards.lock().await;
    guard.contains(card_id)
}

#[derive(Debug, Clone)]
pub(crate) struct ImageLookup {
    pub(crate) card_type: CardType,
    pub(crate) entity_name: String,
}

fn card_type_label(card_type: &CardType) -> &'static str {
    match card_type {
        CardType::Meal => "meal",
        CardType::DeliveryOrder => "delivery",
        CardType::DeliverySession => "delivery_session",
        CardType::MediaAdd => "media",
        CardType::Music => "media",
        CardType::Thought => "thought",
        CardType::Query => "query",
        CardType::CodeTask => "code_task",
        CardType::Generic => "generic",
    }
}
