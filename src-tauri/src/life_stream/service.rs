use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use tauri::Emitter;
use tokio::sync::{Mutex, Semaphore};

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
    absolute_from_relative, find_image_candidates, import_image_asset, resolve_entity_for_card,
    sync_entity_file_image, EntityFileSyncOptions, ResolvedEntity,
};
use super::images::ImageService;
use super::mcp_bridge::LifeMcpBridge;
use super::obsidian::ObsidianIO;
use super::types::*;

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
        let entity = resolve_entity_for_card(&existing_card, node_id);

        let imported_asset =
            import_image_asset(&root, &entity, std::path::Path::new(source_path)).await?;
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

    if let Some(lookup) = enriched.image_lookup {
        if let Some(root) = obsidian_root {
            let cards = Arc::clone(cards);
            let emitter = emitter.clone();
            let event_sink = event_sink.clone();
            let card_id = card_id.to_string();
            let tmdb_key = tmdb_api_key.map(|value| value.to_string());
            let root = PathBuf::from(root);
            tokio::spawn(async move {
                let image_service = ImageService::new(root, tmdb_key);
                let image = image_service
                    .fetch_image(card_type_label(&lookup.card_type), &lookup.entity_name)
                    .await;
                let _ = emit_patch(
                    &card_id,
                    StreamCardPatch {
                        image: Some(image),
                        ..Default::default()
                    },
                    &cards,
                    &emitter,
                    &event_sink,
                )
                .await;
            });
        }
    }

    Ok(())
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

    let image_lookup = if title.is_empty() {
        None
    } else {
        Some(ImageLookup {
            card_type: CardType::MediaAdd,
            entity_name: title.clone(),
        })
    };

    let image = image_lookup.as_ref().map(|_| CardImage {
        url: None,
        status: ImageStatus::Loading,
        source: None,
    });

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
    ClaimResponse,
}

fn causal_frame_profile(card_type: &CardType) -> CausalFrameProfile {
    match card_type {
        CardType::DeliveryOrder | CardType::DeliverySession | CardType::Meal => {
            CausalFrameProfile::ActionReward
        }
        CardType::Thought | CardType::Query | CardType::MediaAdd | CardType::Music => {
            CausalFrameProfile::ClaimResponse
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

    if looks_like_claim_or_question_input(input) {
        return CausalFrameProfile::ClaimResponse;
    }

    if expanded_contains_markdown_headings(enriched) {
        return CausalFrameProfile::ClaimResponse;
    }

    CausalFrameProfile::CauseEffect
}

pub(crate) fn build_causal_content(
    card_id: &str,
    card_type: &CardType,
    input: &str,
    occurred_at: &str,
    enriched: &EnrichedData,
) -> CausalCardContent {
    let frame = resolve_causal_frame_profile(card_type, input, enriched);
    let left_node_id = format!("{card_id}:left:0");
    let left_text = if input.trim().is_empty() {
        enriched.title.clone()
    } else {
        input.trim().to_string()
    };
    let left_entity = enriched
        .entities
        .as_ref()
        .and_then(|entities| entities.first())
        .cloned();

    let left_nodes = vec![CausalNode {
        id: left_node_id.clone(),
        text: left_text,
        role: Some(infer_left_node_role(frame, card_type, input)),
        image: None,
        entity: left_entity,
        occurred_at: Some(occurred_at.to_string()),
    }];

    let mut right_nodes = collect_right_nodes(card_id, frame, enriched);

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

        right_nodes.push(CausalNode {
            id: format!("{card_id}:right:0"),
            text: fallback_text,
            role: Some(infer_right_node_role(frame)),
            image: None,
            entity: None,
            occurred_at: None,
        });
    }

    if let Some(image) = enriched.image.clone() {
        if let Some(node) = right_nodes.first_mut() {
            if node.image.is_none() {
                node.image = Some(image);
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

    CausalCardContent {
        left_nodes,
        right_nodes,
        links,
        layout,
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
                    right_nodes.push(CausalNode {
                        id: format!("{card_id}:right:{index}"),
                        text: row,
                        role: Some(CausalNodeRole::Reward),
                        image: None,
                        entity: None,
                        occurred_at: None,
                    });
                    index += 1;
                }
                continue;
            }

            if frame == CausalFrameProfile::ClaimResponse {
                let heading_nodes = extract_markdown_heading_nodes(body);
                if !heading_nodes.is_empty() {
                    for heading in heading_nodes {
                        right_nodes.push(CausalNode {
                            id: format!("{card_id}:right:{index}"),
                            text: heading,
                            role: Some(role.clone()),
                            image: None,
                            entity: None,
                            occurred_at: None,
                        });
                        index += 1;
                    }
                    continue;
                }

                let emphasized_nodes = extract_emphasized_claim_lines(body);
                if !emphasized_nodes.is_empty() {
                    for item in emphasized_nodes {
                        right_nodes.push(CausalNode {
                            id: format!("{card_id}:right:{index}"),
                            text: item,
                            role: Some(role.clone()),
                            image: None,
                            entity: None,
                            occurred_at: None,
                        });
                        index += 1;
                    }
                    continue;
                }
            } else {
                let bullet_items = extract_bullet_lines(body);
                if bullet_items.len() > 1 {
                    for item in bullet_items {
                        right_nodes.push(CausalNode {
                            id: format!("{card_id}:right:{index}"),
                            text: if title.is_empty() {
                                item
                            } else {
                                format!("{title}: {item}")
                            },
                            role: Some(role.clone()),
                            image: None,
                            entity: None,
                            occurred_at: None,
                        });
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
                right_nodes.push(CausalNode {
                    id: format!("{card_id}:right:{index}"),
                    text: if include_title_prefix {
                        format!("{title}: {summary}")
                    } else {
                        summary
                    },
                    role: Some(role.clone()),
                    image: None,
                    entity: None,
                    occurred_at: None,
                });
                index += 1;
            }
        }
    }

    right_nodes
}

fn infer_left_node_role(
    frame: CausalFrameProfile,
    card_type: &CardType,
    input: &str,
) -> CausalNodeRole {
    match frame {
        CausalFrameProfile::ActionReward => CausalNodeRole::Action,
        CausalFrameProfile::ClaimResponse => {
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
        CausalFrameProfile::ClaimResponse => CausalNodeRole::Response,
        CausalFrameProfile::CauseEffect => CausalNodeRole::Effect,
    }
}

fn looks_like_claim_or_question_input(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains('?') {
        return true;
    }

    let lower = trimmed.to_lowercase();
    [
        "i think",
        "i feel",
        "i believe",
        "favorite",
        "should",
        "why",
        "how",
        "what if",
        "would it",
        "could it",
        "do you think",
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

fn extract_markdown_heading_nodes(body: &str) -> Vec<String> {
    let mut headings = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }

        let heading_text = trimmed.trim_start_matches('#').trim();
        if heading_text.is_empty() {
            continue;
        }

        let cleaned = heading_text.trim_matches('*').trim_matches('_').trim();
        if cleaned.is_empty() {
            continue;
        }

        headings.push(truncate_summary(cleaned, 140));
    }

    headings
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

    next
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
            next_left_nodes.push(CausalNode {
                id: format!("{card_id}:left:{segment_index}"),
                text: segment.clone(),
                role: source.role.clone(),
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

    let merged_text = format!(
        "{}\n• {}",
        first_node.text.trim(),
        second_node.text.trim()
    );

    let merged_node = CausalNode {
        id: format!("{card_id}:right:merged"),
        text: truncate_summary(&merged_text, 260),
        role: first_node.role.clone().or(second_node.role.clone()),
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
    image
        .is_some_and(|value| value.status == ImageStatus::Ready && value.url.is_some())
}

fn entity_link_for(entity_type: &str, entity_name: &str) -> String {
    let folder = match entity_type {
        "media" => "Entities/Media",
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
