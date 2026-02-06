use std::path::PathBuf;

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
    root.join("Runtime").join("life-stream.tasks.v1.json")
}

pub(crate) async fn load_task_dock(entry: &WorkspaceEntry) -> Result<TaskDockPayload, String> {
    let path = task_dock_path(entry);
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

#[cfg(test)]
mod tests {
    use super::task_dock_path;
    use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};

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
}
