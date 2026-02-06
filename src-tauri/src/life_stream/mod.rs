mod events;
mod handlers;
mod image_catalog;
mod image_manual;
mod images;
mod mcp_bridge;
mod obsidian;
mod service;
mod task_dock;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod tests;
mod types;

pub use service::LifeStreamService;
pub use types::*;

use serde_json::json;
use tauri::{AppHandle, State};
use chrono::Local;

use crate::remote_backend;
use crate::state::AppState;

#[tauri::command]
pub async fn life_stream_load_day(
    workspace_id: String,
    date_iso: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<StreamCard>, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let response = remote_backend::call_remote(
            &*state,
            app,
            "life_stream_load_day",
            json!({ "workspaceId": workspace_id, "dateIso": date_iso }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|err| err.to_string());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    life_stream
        .load_day(&entry.path, obsidian_root, &date_iso)
        .await
}

#[tauri::command]
pub async fn life_stream_submit(
    workspace_id: String,
    card_id: String,
    input: String,
    occurred_at_iso: Option<String>,
    model_id: Option<String>,
    effort: Option<String>,
    access_mode: Option<String>,
    collaboration_mode: Option<serde_json::Value>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        remote_backend::call_remote(
            &*state,
            app,
            "life_stream_submit",
            json!({
                "workspaceId": workspace_id,
                "cardId": card_id,
                "input": input,
                "occurredAtIso": occurred_at_iso,
                "modelId": model_id,
                "effort": effort,
                "accessMode": access_mode,
                "collaborationMode": collaboration_mode,
            }),
        )
        .await?;
        return Ok(());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    let request = if model_id.is_some() || effort.is_some() || access_mode.is_some() {
        Some(crate::life_stream::CardRequestMeta {
            model: model_id,
            effort,
            access_mode,
        })
    } else {
        None
    };

    life_stream
        .submit(
            &workspace_id,
            &entry.path,
            obsidian_root,
            &card_id,
            &input,
            occurred_at_iso.as_deref(),
            request,
        )
        .await
}

#[tauri::command]
pub async fn life_stream_cancel(
    workspace_id: String,
    card_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        remote_backend::call_remote(
            &*state,
            app,
            "life_stream_cancel",
            json!({ "workspaceId": workspace_id, "cardId": card_id }),
        )
        .await?;
        return Ok(());
    }

    let life_stream = state.life_stream_service.lock().await;
    life_stream.cancel(&card_id).await
}

#[tauri::command]
pub async fn life_stream_retry(
    workspace_id: String,
    card_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        remote_backend::call_remote(
            &*state,
            app,
            "life_stream_retry",
            json!({ "workspaceId": workspace_id, "cardId": card_id }),
        )
        .await?;
        return Ok(());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    life_stream
        .retry(&workspace_id, &entry.path, obsidian_root, &card_id)
        .await
}

#[tauri::command]
pub async fn life_stream_clarify(
    workspace_id: String,
    card_id: String,
    option_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        remote_backend::call_remote(
            &*state,
            app,
            "life_stream_clarify",
            json!({ "workspaceId": workspace_id, "cardId": card_id, "optionId": option_id }),
        )
        .await?;
        return Ok(());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    life_stream
        .resume_with_clarification(
            &workspace_id,
            &entry.path,
            obsidian_root,
            &card_id,
            &option_id,
        )
        .await
}

#[tauri::command]
pub async fn life_stream_task_dock_load(
    workspace_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TaskDockPayload, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let response = remote_backend::call_remote(
            &*state,
            app,
            "life_stream_task_dock_load",
            json!({ "workspaceId": workspace_id }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|err| err.to_string());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    task_dock::load_task_dock(entry).await
}

#[tauri::command]
pub async fn life_stream_task_dock_save(
    workspace_id: String,
    payload: TaskDockPayload,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        remote_backend::call_remote(
            &*state,
            app,
            "life_stream_task_dock_save",
            json!({ "workspaceId": workspace_id, "payload": payload }),
        )
        .await?;
        return Ok(());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    task_dock::save_task_dock(entry, &payload).await
}

#[tauri::command]
pub async fn life_stream_restructure(
    workspace_id: String,
    card_id: String,
    action: CausalRestructureAction,
    source_node_ids: Option<Vec<String>>,
    target_mode: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<CausalRestructureResult, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let response = remote_backend::call_remote(
            &*state,
            app,
            "life_stream_restructure",
            json!({
                "workspaceId": workspace_id,
                "cardId": card_id,
                "action": action,
                "sourceNodeIds": source_node_ids,
                "targetMode": target_mode,
            }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|error| error.to_string());
    }

    let life_stream = state.life_stream_service.lock().await;
    life_stream
        .restructure(
            &card_id,
            action,
            source_node_ids.unwrap_or_default(),
            target_mode,
        )
        .await
}

#[tauri::command]
pub async fn life_stream_image_candidates(
    workspace_id: String,
    card_id: String,
    node_id: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ImageCandidateResponse, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let response = remote_backend::call_remote(
            &*state,
            app,
            "life_stream_image_candidates",
            json!({
                "workspaceId": workspace_id,
                "cardId": card_id,
                "nodeId": node_id,
            }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|error| error.to_string());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    let response = life_stream
        .image_candidates(&entry.path, obsidian_root, &card_id, node_id.as_deref())
        .await?;
    if response
        .candidates
        .iter()
        .any(|candidate| candidate.source_kind.starts_with("provider_"))
    {
        let target_date = Local::now().format("%Y-%m-%d").to_string();
        let task = TaskDockItem {
            id: format!("task_{}", uuid::Uuid::new_v4().simple()),
            key: format!(
                "review-image-candidates:{}:{}",
                card_id,
                node_id.as_deref().unwrap_or("card")
            ),
            text: format!("Review image candidates for {}", response.entity_name),
            kind: TaskDockItemKind::Reminder,
            completed: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            target_date,
            source_card_id: Some(card_id.clone()),
            source_node_id: node_id.clone(),
        };
        task_dock::upsert_task_dock_item(entry, task).await?;
    }

    Ok(response)
}

#[tauri::command]
pub async fn life_stream_image_attach(
    workspace_id: String,
    card_id: String,
    node_id: Option<String>,
    source_path: String,
    set_primary: Option<bool>,
    set_context_override: Option<bool>,
    context_hint: Option<String>,
    update_entity_file: Option<bool>,
    update_entity_embed: Option<bool>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ImageAttachResult, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let response = remote_backend::call_remote(
            &*state,
            app,
            "life_stream_image_attach",
            json!({
                "workspaceId": workspace_id,
                "cardId": card_id,
                "nodeId": node_id,
                "sourcePath": source_path,
                "setPrimary": set_primary,
                "setContextOverride": set_context_override,
                "contextHint": context_hint,
                "updateEntityFile": update_entity_file,
                "updateEntityEmbed": update_entity_embed,
            }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|error| error.to_string());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    life_stream
        .attach_image(
            &entry.path,
            obsidian_root,
            &card_id,
            node_id.as_deref(),
            &source_path,
            set_primary.unwrap_or(true),
            set_context_override.unwrap_or(false),
            context_hint.as_deref(),
            update_entity_file.unwrap_or(true),
            update_entity_embed.unwrap_or(false),
        )
        .await
}

#[tauri::command]
pub async fn life_stream_image_backfill(
    workspace_id: String,
    update_embed_block: Option<bool>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ImageBackfillSummary, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let response = remote_backend::call_remote(
            &*state,
            app,
            "life_stream_image_backfill",
            json!({
                "workspaceId": workspace_id,
                "updateEmbedBlock": update_embed_block,
            }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|error| error.to_string());
    }

    let workspaces = state.workspaces.lock().await;
    let entry = workspaces.get(&workspace_id).ok_or("workspace not found")?;
    let obsidian_root = entry.settings.obsidian_root.as_deref();

    let life_stream = state.life_stream_service.lock().await;
    life_stream
        .backfill_entity_images(
            &entry.path,
            obsidian_root,
            update_embed_block.unwrap_or(false),
        )
        .await
}
