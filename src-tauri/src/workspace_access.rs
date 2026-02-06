use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::time::sleep;

use crate::types::WorkspaceEntry;

const ACCESS_GATE_TIMEOUT: Duration = Duration::from_secs(18);
const ACCESS_GATE_POLL_INTERVAL: Duration = Duration::from_millis(450);

pub(crate) fn workspace_access_paths(
    entry: &WorkspaceEntry,
    parent_entry: Option<&WorkspaceEntry>,
) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    let mut push_unique = |path: PathBuf| {
        if path.as_os_str().is_empty() {
            return;
        }
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            paths.push(path);
        }
    };

    push_unique(PathBuf::from(&entry.path));

    if let Some(obsidian_root) = entry.settings.obsidian_root.as_ref() {
        push_unique(PathBuf::from(obsidian_root));
    }

    if let Some(parent_entry) = parent_entry {
        push_unique(PathBuf::from(&parent_entry.path));
        if let Some(obsidian_root) = parent_entry.settings.obsidian_root.as_ref() {
            push_unique(PathBuf::from(obsidian_root));
        }
    }

    paths
}

pub(crate) async fn ensure_workspace_access_for_workspace(
    entry: &WorkspaceEntry,
    parent_entry: Option<&WorkspaceEntry>,
) -> Result<(), String> {
    let paths = workspace_access_paths(entry, parent_entry);
    ensure_workspace_access(&paths).await
}

pub(crate) async fn ensure_workspace_access(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    let start = Instant::now();

    loop {
        let mut blocked_path: Option<(PathBuf, std::io::Error)> = None;

        for path in paths {
            match probe_path_access(path) {
                Ok(()) => {}
                Err(error) if is_permission_error(&error) => {
                    blocked_path = Some((path.clone(), error));
                    break;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "Workspace path unavailable: {} ({})",
                        path.display(),
                        error
                    ));
                }
            }
        }

        if let Some((path, error)) = blocked_path {
            let _ = error;
            if start.elapsed() >= ACCESS_GATE_TIMEOUT {
                return Err(format!(
                    "macOS is blocking access to {}. Please click “Allow” for external drive access, then retry.",
                    path.display()
                ));
            }
            sleep(ACCESS_GATE_POLL_INTERVAL).await;
            continue;
        }

        return Ok(());
    }
}

fn probe_path_access(path: &Path) -> Result<(), std::io::Error> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        let _ = std::fs::read_dir(path)?;
    } else {
        let _ = std::fs::File::open(path)?;
    }
    Ok(())
}

fn is_permission_error(error: &std::io::Error) -> bool {
    if error.kind() == ErrorKind::PermissionDenied {
        return true;
    }
    matches!(error.raw_os_error(), Some(1) | Some(13))
}

#[cfg(test)]
mod tests {
    use super::workspace_access_paths;
    use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};

    fn workspace_entry(id: &str, path: &str, obsidian_root: Option<&str>) -> WorkspaceEntry {
        let mut settings = WorkspaceSettings::default();
        settings.obsidian_root = obsidian_root.map(str::to_string);

        WorkspaceEntry {
            id: id.to_string(),
            name: id.to_string(),
            path: path.to_string(),
            codex_bin: None,
            kind: WorkspaceKind::Main,
            parent_id: None,
            worktree: None,
            settings,
        }
    }

    #[test]
    fn workspace_access_paths_includes_workspace_and_obsidian_roots() {
        let entry = workspace_entry("main", "/tmp/workspace", Some("/tmp/obsidian"));
        let parent = workspace_entry("parent", "/tmp/parent", Some("/tmp/obsidian-parent"));

        let paths = workspace_access_paths(&entry, Some(&parent));
        let values = paths
            .iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(values.contains(&"/tmp/workspace".to_string()));
        assert!(values.contains(&"/tmp/obsidian".to_string()));
        assert!(values.contains(&"/tmp/parent".to_string()));
        assert!(values.contains(&"/tmp/obsidian-parent".to_string()));
    }

    #[test]
    fn workspace_access_paths_deduplicates_paths() {
        let entry = workspace_entry("main", "/tmp/workspace", Some("/tmp/workspace"));

        let paths = workspace_access_paths(&entry, None);
        let values = paths
            .iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "/tmp/workspace".to_string());
    }
}
