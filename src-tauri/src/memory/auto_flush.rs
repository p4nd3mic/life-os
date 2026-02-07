use crate::backend::app_server::{next_background_callback_id, WorkspaceSession};
use crate::memory::MemoryService;
use crate::types::AutoMemorySettings;
use crate::utils::{git_env_path, resolve_git_binary};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[derive(Clone, Debug, Default)]
pub struct AutoMemoryRuntime {
    per_thread: HashMap<String, ThreadAutoState>,
}

#[derive(Clone, Debug, Default)]
struct ThreadAutoState {
    last_flush_at: Option<Instant>,
    last_seen_context_tokens: Option<u32>,
    last_compaction_epoch: u64,
    last_flush_epoch: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryFlushSnapshot {
    pub workspace_id: String,
    pub thread_id: String,
    pub created_at_ms: i64,
    pub model: Option<String>,
    pub context_tokens: u32,
    pub model_context_window: u32,
    pub turns: Vec<SnapshotTurn>,
    pub git_status: Option<String>,
    pub tool_tail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotTurn {
    pub role: String,
    pub text: String,
}

pub struct MemoryFlushResult {
    pub no_reply: bool,
    pub title: String,
    pub tags: Vec<String>,
    pub daily_markdown: String,
    pub curated_markdown: String,
}

pub fn should_flush(
    settings: &AutoMemorySettings,
    context_tokens: u32,
    model_context_window: u32,
) -> bool {
    if !settings.enabled || model_context_window == 0 {
        return false;
    }

    let usable_window = model_context_window.saturating_sub(settings.reserve_tokens_floor);
    if usable_window == 0 {
        return false;
    }

    context_tokens >= usable_window.saturating_sub(settings.soft_threshold_tokens)
}

pub fn detect_compaction_epoch(prev: Option<u32>, now: u32, epoch: u64) -> u64 {
    match prev {
        None => epoch,
        Some(prev_tokens) => {
            if now + (now / 2) < prev_tokens {
                epoch + 1
            } else {
                epoch
            }
        }
    }
}

impl AutoMemoryRuntime {
    pub fn update_and_check(
        &mut self,
        thread_key: &str,
        context_tokens: u32,
        model_context_window: u32,
        settings: &AutoMemorySettings,
    ) -> bool {
        if !settings.enabled {
            return false;
        }
        let state = self.per_thread.entry(thread_key.to_string()).or_default();
        let next_epoch = detect_compaction_epoch(
            state.last_seen_context_tokens,
            context_tokens,
            state.last_compaction_epoch,
        );
        state.last_compaction_epoch = next_epoch;
        state.last_seen_context_tokens = Some(context_tokens);

        if !should_flush(settings, context_tokens, model_context_window) {
            return false;
        }

        if let Some(last_flush_at) = state.last_flush_at {
            let elapsed = last_flush_at.elapsed().as_secs();
            if elapsed < settings.min_interval_seconds as u64 {
                return false;
            }
        }

        if state.last_flush_epoch == Some(state.last_compaction_epoch) {
            return false;
        }

        state.last_flush_at = Some(Instant::now());
        state.last_flush_epoch = Some(state.last_compaction_epoch);
        true
    }
}

pub async fn build_snapshot(
    session: &WorkspaceSession,
    workspace_id: &str,
    thread_id: &str,
    context_tokens: u32,
    model_context_window: u32,
    settings: &AutoMemorySettings,
) -> Result<MemoryFlushSnapshot, String> {
    let thread_response = session
        .send_request("thread/resume", json!({ "threadId": thread_id }))
        .await?;

    let turns_value = thread_response
        .pointer("/result/thread/turns")
        .or_else(|| thread_response.pointer("/thread/turns"))
        .cloned()
        .unwrap_or(Value::Array(vec![]));

    let mut turns: Vec<SnapshotTurn> = Vec::new();
    if let Value::Array(items) = turns_value {
        for turn in items.into_iter().rev().take(settings.max_turns).rev() {
            if let Some(turn_items) = turn.get("items").and_then(|v| v.as_array()) {
                for item in turn_items {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let role = match item_type {
                        "userMessage" => "user",
                        "agentMessage" => "assistant",
                        "toolOutput" => {
                            if settings.include_tool_output {
                                "tool"
                            } else {
                                continue;
                            }
                        }
                        _ => continue,
                    };
                    let text = extract_item_text(item);
                    if !text.trim().is_empty() {
                        turns.push(SnapshotTurn {
                            role: role.to_string(),
                            text,
                        });
                    }
                }
            }
        }
    }

    let git_status = if settings.include_git_status {
        Some(collect_git_status(&session.entry.path).await)
    } else {
        None
    };

    let tool_tail = if settings.include_tool_output {
        Some(
            turns
                .iter()
                .filter(|t| t.role == "tool")
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("\n")
                .chars()
                .take(settings.max_snapshot_chars)
                .collect(),
        )
    } else {
        None
    };

    Ok(MemoryFlushSnapshot {
        workspace_id: workspace_id.to_string(),
        thread_id: thread_id.to_string(),
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        model: thread_response
            .pointer("/result/thread/model")
            .or_else(|| thread_response.pointer("/thread/model"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        context_tokens,
        model_context_window,
        turns,
        git_status,
        tool_tail,
    })
}

fn extract_item_text(item: &Value) -> String {
    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(content) = item.get("content") {
        if let Some(text) = content
            .get(0)
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
        {
            return text.to_string();
        }
    }
    String::new()
}

async fn collect_git_status(path: &str) -> String {
    let mut output = String::new();
    let git_bin = match resolve_git_binary() {
        Ok(path) => path,
        Err(_) => return output,
    };
    let status = tokio::process::Command::new(git_bin.clone())
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("-sb")
        .env("PATH", git_env_path())
        .output()
        .await;
    if let Ok(status) = status {
        if status.status.success() {
            output.push_str(&String::from_utf8_lossy(&status.stdout));
        }
    }

    let diff = tokio::process::Command::new(git_bin)
        .arg("-C")
        .arg(path)
        .arg("diff")
        .arg("--stat")
        .env("PATH", git_env_path())
        .output()
        .await;
    if let Ok(diff) = diff {
        if diff.status.success() {
            if !output.is_empty() {
                output.push_str("\n");
            }
            output.push_str(&String::from_utf8_lossy(&diff.stdout));
        }
    }

    output.chars().take(4000).collect()
}

pub async fn run_memory_flush_summarizer(
    session: &WorkspaceSession,
    snapshot: &MemoryFlushSnapshot,
) -> Result<String, String> {
    let prompt = build_memory_flush_prompt(snapshot)?;

    let thread_params = json!({
        "cwd": session.entry.path,
        "approvalPolicy": "never"
    });
    let thread_result = session.send_request("thread/start", thread_params).await?;

    if let Some(error) = thread_result.get("error") {
        let error_msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error starting thread");
        return Err(error_msg.to_string());
    }

    let thread_id = thread_result
        .get("result")
        .and_then(|r| r.get("threadId"))
        .or_else(|| {
            thread_result
                .get("result")
                .and_then(|r| r.get("thread"))
                .and_then(|t| t.get("id"))
        })
        .or_else(|| thread_result.get("threadId"))
        .or_else(|| thread_result.get("thread").and_then(|t| t.get("id")))
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            format!(
                "Failed to get threadId from thread/start response: {:?}",
                thread_result
            )
        })?
        .to_string();

    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let callback_consumer_id = next_background_callback_id("memory-flush");
    session
        .register_background_callback(thread_id.as_str(), callback_consumer_id.as_str(), tx)
        .await;

    let turn_params = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": prompt }],
        "cwd": session.entry.path,
        "approvalPolicy": "never",
        "sandboxPolicy": { "type": "readOnly" },
    });
    let turn_result = session.send_request("turn/start", turn_params).await;
    let turn_result = match turn_result {
        Ok(result) => result,
        Err(error) => {
            session
                .unregister_background_callback(thread_id.as_str(), callback_consumer_id.as_str())
                .await;
            let archive_params = json!({ "threadId": thread_id.as_str() });
            let _ = session.send_request("thread/archive", archive_params).await;
            return Err(error);
        }
    };

    if let Some(error) = turn_result.get("error") {
        let error_msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error starting turn");
        session
            .unregister_background_callback(thread_id.as_str(), callback_consumer_id.as_str())
            .await;
        let archive_params = json!({ "threadId": thread_id.as_str() });
        let _ = session.send_request("thread/archive", archive_params).await;
        return Err(error_msg.to_string());
    }

    let mut output = String::new();
    let timeout_duration = Duration::from_secs(60);
    let collect_result = timeout(timeout_duration, async {
        while let Some(event) = rx.recv().await {
            let method = event.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "item/agentMessage/delta" => {
                    if let Some(params) = event.get("params") {
                        if let Some(delta) = params.get("delta").and_then(|d| d.as_str()) {
                            output.push_str(delta);
                        }
                    }
                }
                "turn/completed" => break,
                "turn/error" => {
                    if let Some(params) = event.get("params") {
                        let error = params
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error during memory flush");
                        return Err(error.to_string());
                    }
                }
                _ => {}
            }
        }
        Ok::<(), String>(())
    })
    .await;

    session
        .unregister_background_callback(thread_id.as_str(), callback_consumer_id.as_str())
        .await;
    let archive_params = json!({ "threadId": thread_id.as_str() });
    let _ = session.send_request("thread/archive", archive_params).await;

    match collect_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err),
        Err(_) => return Err("Timeout waiting for memory flush".to_string()),
    }

    if output.trim().is_empty() {
        return Err("No output from memory flush".to_string());
    }

    Ok(output.trim().to_string())
}

fn build_memory_flush_prompt(snapshot: &MemoryFlushSnapshot) -> Result<String, String> {
    let snapshot_json = serde_json::to_string_pretty(snapshot).map_err(|err| err.to_string())?;
    Ok(format!(
        "You are CodexMonitor Auto-Memory.\n\n\
TASK:\n\
- Extract durable facts, decisions, TODOs, and project state worth remembering.\n\
- Output STRICT JSON ONLY, no markdown, no prose.\n\n\
OUTPUT JSON SCHEMA:\n\
{{\n  \"no_reply\": boolean,\n  \"title\": string,\n  \"tags\": string[],\n  \"daily_markdown\": string,\n  \"curated_markdown\": string\n}}\n\n\
RULES:\n\
- If nothing worth storing, set no_reply=true and leave other fields empty.\n\
- daily_markdown: short append-only log entry (timestamped), bullet-heavy.\n\
- curated_markdown: stable facts (names, endpoints, commands, gotchas), omit ephemeral chatter.\n\
- Keep each field <= 1500 chars.\n\n\
SNAPSHOT:\n\
{snapshot_json}\n"
    ))
}

pub fn parse_memory_flush_result(raw: &str) -> MemoryFlushResult {
    let parsed: Result<MemoryFlushResult, _> = serde_json::from_str(raw);
    if let Ok(result) = parsed {
        return result;
    }

    MemoryFlushResult {
        no_reply: false,
        title: "Auto Memory".to_string(),
        tags: vec!["auto_memory_parse_error".to_string()],
        daily_markdown: raw.to_string(),
        curated_markdown: String::new(),
    }
}

impl<'de> Deserialize<'de> for MemoryFlushResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(default)]
            no_reply: bool,
            #[serde(default)]
            title: String,
            #[serde(default)]
            tags: Vec<String>,
            #[serde(default)]
            daily_markdown: String,
            #[serde(default)]
            curated_markdown: String,
        }

        let helper = Helper::deserialize(deserializer)?;
        Ok(MemoryFlushResult {
            no_reply: helper.no_reply,
            title: helper.title,
            tags: helper.tags,
            daily_markdown: helper.daily_markdown,
            curated_markdown: helper.curated_markdown,
        })
    }
}

pub async fn write_memory_flush(
    memory: &MemoryService,
    snapshot: &MemoryFlushSnapshot,
    result: &MemoryFlushResult,
    settings: &AutoMemorySettings,
) -> Result<(), String> {
    if result.no_reply {
        return Ok(());
    }

    let mut tags = result.tags.clone();
    tags.push("auto_memory".to_string());
    tags.push(format!("workspace:{}", snapshot.workspace_id));
    tags.push(format!("thread:{}", snapshot.thread_id));

    if settings.write_daily && !result.daily_markdown.trim().is_empty() {
        let _ = memory
            .append(
                "daily",
                result.daily_markdown.trim(),
                tags.clone(),
                Some(snapshot.workspace_id.clone()),
            )
            .await?;
    }

    if settings.write_curated && !result.curated_markdown.trim().is_empty() {
        let _ = memory
            .append(
                "curated",
                result.curated_markdown.trim(),
                tags.clone(),
                Some(snapshot.workspace_id.clone()),
            )
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_flush_respects_thresholds() {
        let mut settings = AutoMemorySettings::default();
        settings.enabled = true;
        settings.reserve_tokens_floor = 10_000;
        settings.soft_threshold_tokens = 2_000;
        let model_window = 32_000;

        // usable window = 22k, trigger when context >= 20k
        assert!(!should_flush(&settings, 19_500, model_window));
        assert!(should_flush(&settings, 20_000, model_window));
        assert!(should_flush(&settings, 25_000, model_window));
    }

    #[test]
    fn should_flush_disabled_is_false() {
        let mut settings = AutoMemorySettings::default();
        settings.enabled = false;
        assert!(!should_flush(&settings, 50_000, 128_000));
    }

    #[test]
    fn compaction_epoch_increments_on_drop() {
        let epoch = 3;
        let next = detect_compaction_epoch(Some(20_000), 8_000, epoch);
        assert_eq!(next, epoch + 1);
        let same = detect_compaction_epoch(Some(20_000), 16_000, epoch);
        assert_eq!(same, epoch);
    }

    #[test]
    fn parse_memory_flush_result_handles_invalid_json() {
        let raw = "not json";
        let result = parse_memory_flush_result(raw);
        assert!(!result.no_reply);
        assert!(result.daily_markdown.contains("not json"));
        assert!(result
            .tags
            .iter()
            .any(|tag| tag == "auto_memory_parse_error"));
    }
}
