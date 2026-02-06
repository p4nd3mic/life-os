use std::path::{Path, PathBuf};

use tokio::fs;

use crate::types::WorkspaceEntry;

use super::types::{TaskDockItem, TaskDockPayload};

pub(crate) fn task_dock_path(entry: &WorkspaceEntry) -> PathBuf {
    let root = entry
        .settings
        .obsidian_root
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&entry.path));
    task_dock_path_from_root(&root)
}

pub(crate) fn task_dock_path_from_root(root: &Path) -> PathBuf {
    root.join("Runtime").join("life-stream.tasks.v1.json")
}

pub(crate) async fn load_task_dock(entry: &WorkspaceEntry) -> Result<TaskDockPayload, String> {
    let path = task_dock_path(entry);
    load_task_dock_from_path(&path).await
}

pub(crate) async fn load_task_dock_at_root(root: &Path) -> Result<TaskDockPayload, String> {
    let path = task_dock_path_from_root(root);
    load_task_dock_from_path(&path).await
}

async fn load_task_dock_from_path(path: &Path) -> Result<TaskDockPayload, String> {
    if !path.exists() {
        return Ok(TaskDockPayload::default());
    }

    let content = fs::read_to_string(&path)
        .await
        .map_err(|error| format!("Failed reading task dock {}: {}", path.display(), error))?;
    serde_json::from_str::<TaskDockPayload>(&content)
        .map_err(|error| format!("Failed parsing task dock {}: {}", path.display(), error))
}

pub(crate) async fn save_task_dock(
    entry: &WorkspaceEntry,
    payload: &TaskDockPayload,
) -> Result<(), String> {
    let path = task_dock_path(entry);
    save_task_dock_to_path(&path, payload).await
}

pub(crate) async fn save_task_dock_at_root(
    root: &Path,
    payload: &TaskDockPayload,
) -> Result<(), String> {
    let path = task_dock_path_from_root(root);
    save_task_dock_to_path(&path, payload).await
}

async fn save_task_dock_to_path(path: &Path, payload: &TaskDockPayload) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Failed creating task dock directory {}: {}", parent.display(), error))?;
    }

    let content = serde_json::to_string_pretty(payload)
        .map_err(|error| format!("Failed serializing task dock payload: {}", error))?;
    fs::write(&path, content)
        .await
        .map_err(|error| format!("Failed writing task dock {}: {}", path.display(), error))
}

pub(crate) async fn upsert_task_dock_item(
    entry: &WorkspaceEntry,
    item: TaskDockItem,
) -> Result<(), String> {
    let mut payload = load_task_dock(entry).await.unwrap_or_default();
    if payload.items.iter().any(|existing| existing.key == item.key) {
        return Ok(());
    }
    payload.items.insert(0, item);
    save_task_dock(entry, &payload).await
}

pub(crate) async fn upsert_task_dock_item_at_root(
    root: &Path,
    item: TaskDockItem,
) -> Result<(), String> {
    let mut payload = load_task_dock_at_root(root).await.unwrap_or_default();
    if payload.items.iter().any(|existing| existing.key == item.key) {
        return Ok(());
    }
    payload.items.insert(0, item);
    save_task_dock_at_root(root, &payload).await
}

#[cfg(test)]
mod tests {
    use super::{load_task_dock_at_root, save_task_dock_at_root, task_dock_path, task_dock_path_from_root};
    use crate::life_stream::types::{TaskDockItem, TaskDockItemKind, TaskDockPayload};
    use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};
    use tempfile::tempdir;

    fn workspace(path: &str, obsidian_root: Option<&str>) -> WorkspaceEntry {
        let mut settings = WorkspaceSettings::default();
        settings.obsidian_root = obsidian_root.map(str::to_string);
        WorkspaceEntry {
            id: "workspace".to_string(),
            name: "Workspace".to_string(),
            path: path.to_string(),
            codex_bin: None,
            kind: WorkspaceKind::Main,
            parent_id: None,
            worktree: None,
            settings,
        }
    }

    #[test]
    fn task_dock_path_prefers_obsidian_root() {
        let entry = workspace("/tmp/workspace", Some("/tmp/obsidian"));
        let path = task_dock_path(&entry);
        assert_eq!(
            path.to_string_lossy().to_string(),
            "/tmp/obsidian/Runtime/life-stream.tasks.v1.json".to_string()
        );
    }

    #[test]
    fn task_dock_path_falls_back_to_workspace_path() {
        let entry = workspace("/tmp/workspace", None);
        let path = task_dock_path(&entry);
        assert_eq!(
            path.to_string_lossy().to_string(),
            "/tmp/workspace/Runtime/life-stream.tasks.v1.json".to_string()
        );
    }

    #[test]
    fn task_dock_path_from_root_uses_runtime_file() {
        let path = task_dock_path_from_root(std::path::Path::new("/tmp/obsidian"));
        assert_eq!(
            path.to_string_lossy().to_string(),
            "/tmp/obsidian/Runtime/life-stream.tasks.v1.json".to_string()
        );
    }

    #[tokio::test]
    async fn save_and_load_task_dock_at_root_round_trip() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let payload = TaskDockPayload {
            version: 1,
            items: vec![TaskDockItem {
                id: "task-1".to_string(),
                key: "review-image-candidates:card:node".to_string(),
                text: "Review image candidates".to_string(),
                kind: TaskDockItemKind::Reminder,
                completed: false,
                created_at: "2026-02-07T00:00:00Z".to_string(),
                updated_at: "2026-02-07T00:00:00Z".to_string(),
                target_date: "2026-02-07".to_string(),
                source_card_id: Some("card-1".to_string()),
                source_node_id: Some("node-1".to_string()),
            }],
        };

        save_task_dock_at_root(root, &payload)
            .await
            .expect("save payload");
        let loaded = load_task_dock_at_root(root).await.expect("load payload");
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].key, "review-image-candidates:card:node");
    }
}
