use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::service::EnrichedData;
use super::types::{
    CardImage, CardState, CardType, CausalCardContent, CausalLayoutState, CausalLink, CausalNode,
    CausalNodeRole, DomainId, ExpandedContent, ExpandedSection, LayoutMode, LifeStreamError,
    SemanticAttemptTraceStats, SemanticEventTraceEntry, StreamCard,
};

#[derive(Clone)]
pub struct ObsidianIO {
    obsidian_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DayThreadRuntimeState {
    pub date: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(default, rename = "lastSeedHash")]
    pub last_seed_hash: Option<String>,
    #[serde(default, rename = "cardCount")]
    pub card_count: usize,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRewriteLogEntry {
    #[serde(rename = "cardId")]
    pub card_id: String,
    pub date: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub prompt: String,
    #[serde(rename = "rawResponse")]
    pub raw_response: String,
    #[serde(default, rename = "parsedJson")]
    pub parsed_json: Option<serde_json::Value>,
    #[serde(default, rename = "failureReason")]
    pub failure_reason: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "completedAt")]
    pub completed_at: String,
    #[serde(default)]
    pub attempts: Vec<SemanticRewriteLogAttempt>,
    #[serde(default, rename = "traceStats")]
    pub trace_stats: Option<SemanticAttemptTraceStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRewriteLogAttempt {
    pub stage: String,
    pub prompt: String,
    #[serde(rename = "rawResponse")]
    pub raw_response: String,
    #[serde(default, rename = "parsedJson")]
    pub parsed_json: Option<serde_json::Value>,
    #[serde(default, rename = "failureReason")]
    pub failure_reason: Option<String>,
    #[serde(default, rename = "eventTrace")]
    pub event_trace: Vec<SemanticEventTraceEntry>,
    #[serde(default, rename = "traceStats")]
    pub trace_stats: Option<SemanticAttemptTraceStats>,
}

impl ObsidianIO {
    pub fn new(obsidian_root: Option<String>) -> Self {
        Self { obsidian_root }
    }

    pub async fn load_cards_for_date(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
    ) -> Result<Vec<StreamCard>, LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let date = NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
            .map_err(|err| LifeStreamError::Parse(format!("Invalid date {date_iso}: {err}")))?;
        let stream_file = stream_file_path(&root, date.year(), date.month());
        let stream_file = validate_path_within_vault(&root, &stream_file)?;
        if !stream_file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&stream_file)
            .await
            .map_err(|err| LifeStreamError::Io(format!("Failed to read stream file: {err}")))?;
        Ok(parse_cards_from_stream(&content, date, &stream_file))
    }

    pub async fn write_card(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
        occurred_at: &str,
        enriched: &EnrichedData,
    ) -> Result<(), LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let date_time = chrono::DateTime::parse_from_rfc3339(occurred_at)
            .map(|value| value.naive_local())
            .or_else(|_| NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%z"))
            .or_else(|_| NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%.f%z"))
            .or_else(|_| NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S"))
            .or_else(|_| NaiveDateTime::parse_from_str(occurred_at, "%Y-%m-%dT%H:%M:%S%.f"))
            .map_err(|err| {
                LifeStreamError::Parse(format!("Invalid timestamp {occurred_at}: {err}"))
            })?;
        let date = date_time.date();
        let stream_file = stream_file_path(&root, date.year(), date.month());
        let stream_file = validate_path_within_vault(&root, &stream_file)?;
        if let Some(parent) = stream_file.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                LifeStreamError::Io(format!("Failed to create stream directory: {err}"))
            })?;
        }

        let content = if stream_file.exists() {
            fs::read_to_string(&stream_file)
                .await
                .map_err(|err| LifeStreamError::Io(format!("Failed to read stream file: {err}")))?
        } else {
            String::new()
        };

        let updated = append_card_entry(&content, date, card_id, occurred_at, enriched);
        fs::write(&stream_file, updated)
            .await
            .map_err(|err| LifeStreamError::Io(format!("Failed to write stream file: {err}")))?;

        Ok(())
    }

    pub async fn write_note_semantic_payload(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        card_id: &str,
        occurred_at: &str,
        causal: &CausalCardContent,
    ) -> Result<(), LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let date_time = parse_occurred_at(occurred_at)?;
        let date = date_time.date();
        let stream_file = stream_file_path(&root, date.year(), date.month());
        let stream_file = validate_path_within_vault(&root, &stream_file)?;
        if !stream_file.exists() {
            return Err(LifeStreamError::Parse(format!(
                "Stream file does not exist for semantic persistence: {}",
                stream_file.display()
            )));
        }

        let content = fs::read_to_string(&stream_file)
            .await
            .map_err(|err| LifeStreamError::Io(format!("Failed to read stream file: {err}")))?;

        let task_id = task_id_for_card(card_id, occurred_at, &date_time);
        let semantic_json = serde_json::to_string(causal).map_err(|err| {
            LifeStreamError::Parse(format!("Failed to encode semantic payload: {err}"))
        })?;
        let semantic_b64 = BASE64_STANDARD.encode(semantic_json.as_bytes());
        let (updated, changed) =
            upsert_note_payload_comment(&content, &task_id, "semantic_b64", &semantic_b64)?;
        if !changed {
            return Ok(());
        }

        fs::write(&stream_file, updated)
            .await
            .map_err(|err| LifeStreamError::Io(format!("Failed to write stream file: {err}")))?;

        Ok(())
    }

    pub async fn read_day_thread_runtime_state(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
    ) -> Result<Option<DayThreadRuntimeState>, LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let runtime_path = day_thread_runtime_path(&root, date_iso)?;
        let runtime_path = validate_path_within_vault(&root, &runtime_path)?;
        if !runtime_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&runtime_path).await.map_err(|err| {
            LifeStreamError::Io(format!(
                "Failed to read day thread runtime file {}: {err}",
                runtime_path.display()
            ))
        })?;

        let payload = serde_json::from_str::<DayThreadRuntimeState>(&content).map_err(|err| {
            LifeStreamError::Parse(format!(
                "Failed to parse day thread runtime file {}: {err}",
                runtime_path.display()
            ))
        })?;
        Ok(Some(payload))
    }

    pub async fn write_day_thread_runtime_state(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        payload: &DayThreadRuntimeState,
    ) -> Result<(), LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let runtime_path = day_thread_runtime_path(&root, payload.date.as_str())?;
        let runtime_path = validate_path_within_vault(&root, &runtime_path)?;
        if let Some(parent) = runtime_path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                LifeStreamError::Io(format!(
                    "Failed to create day thread runtime directory {}: {err}",
                    parent.display()
                ))
            })?;
        }

        let content = serde_json::to_string_pretty(payload).map_err(|err| {
            LifeStreamError::Parse(format!(
                "Failed serializing day thread runtime payload: {err}"
            ))
        })?;
        fs::write(&runtime_path, content).await.map_err(|err| {
            LifeStreamError::Io(format!(
                "Failed to write day thread runtime file {}: {err}",
                runtime_path.display()
            ))
        })?;
        Ok(())
    }

    pub async fn delete_day_thread_runtime_state(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
    ) -> Result<(), LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let runtime_path = day_thread_runtime_path(&root, date_iso)?;
        let runtime_path = validate_path_within_vault(&root, &runtime_path)?;
        if !runtime_path.exists() {
            return Ok(());
        }
        fs::remove_file(&runtime_path).await.map_err(|err| {
            LifeStreamError::Io(format!(
                "Failed to delete day thread runtime file {}: {err}",
                runtime_path.display()
            ))
        })?;
        Ok(())
    }

    pub async fn write_semantic_rewrite_log(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
        date_iso: &str,
        card_id: &str,
        payload: &SemanticRewriteLogEntry,
    ) -> Result<(), LifeStreamError> {
        let root = self.resolve_root(workspace_path, obsidian_root)?;
        let log_path = semantic_rewrite_log_path(&root, date_iso, card_id)?;
        let log_path = validate_path_within_vault(&root, &log_path)?;
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                LifeStreamError::Io(format!(
                    "Failed to create semantic rewrite log directory {}: {err}",
                    parent.display()
                ))
            })?;
        }

        let content = serde_json::to_string_pretty(payload).map_err(|err| {
            LifeStreamError::Parse(format!(
                "Failed serializing semantic rewrite log payload: {err}"
            ))
        })?;
        fs::write(&log_path, content).await.map_err(|err| {
            LifeStreamError::Io(format!(
                "Failed to write semantic rewrite log {}: {err}",
                log_path.display()
            ))
        })?;
        Ok(())
    }

    fn resolve_root(
        &self,
        workspace_path: &str,
        obsidian_root: Option<&str>,
    ) -> Result<PathBuf, LifeStreamError> {
        self.resolve_root_path(workspace_path, obsidian_root)
    }
}

impl ObsidianIO {
    pub fn resolve_root_path(
        &self,
        _workspace_path: &str,
        obsidian_root: Option<&str>,
    ) -> Result<PathBuf, LifeStreamError> {
        if let Some(root) = obsidian_root {
            return Ok(PathBuf::from(root));
        }
        if let Some(root) = &self.obsidian_root {
            return Ok(PathBuf::from(root));
        }
        Err(LifeStreamError::Configuration(
            "obsidian_root not configured in workspace settings".to_string(),
        ))
    }
}

/// Validates that a path stays within the vault root.
fn validate_path_within_vault(
    vault_root: &Path,
    requested_path: &Path,
) -> Result<PathBuf, LifeStreamError> {
    let canonical_root = vault_root
        .canonicalize()
        .map_err(|e| LifeStreamError::Io(format!("Cannot canonicalize vault root: {e}")))?;

    let canonical_requested = if requested_path.exists() {
        requested_path
            .canonicalize()
            .map_err(|e| LifeStreamError::Io(format!("Cannot canonicalize path: {e}")))?
    } else {
        let relative = requested_path.strip_prefix(vault_root).map_err(|_| {
            LifeStreamError::Security(format!(
                "Path traversal attempt: {} is outside vault root {}",
                requested_path.display(),
                vault_root.display()
            ))
        })?;
        canonical_root.join(relative)
    };

    if !canonical_requested.starts_with(&canonical_root) {
        return Err(LifeStreamError::Security(format!(
            "Path traversal attempt: {} is outside vault root {}",
            canonical_requested.display(),
            canonical_root.display()
        )));
    }

    Ok(canonical_requested)
}

fn stream_file_path(root: &Path, year: i32, month: u32) -> PathBuf {
    let filename = format!("{year}-{month:02}.md");
    root.join("Stream").join(filename)
}

fn day_thread_runtime_path(root: &Path, date_iso: &str) -> Result<PathBuf, LifeStreamError> {
    NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
        .map_err(|err| LifeStreamError::Parse(format!("Invalid date {date_iso}: {err}")))?;
    let filename = format!("life-stream.day-thread.{date_iso}.json");
    Ok(root.join("Runtime").join(filename))
}

fn sanitize_filename_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn semantic_rewrite_log_path(
    root: &Path,
    date_iso: &str,
    card_id: &str,
) -> Result<PathBuf, LifeStreamError> {
    NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
        .map_err(|err| LifeStreamError::Parse(format!("Invalid date {date_iso}: {err}")))?;
    let card_component = sanitize_filename_component(card_id);
    Ok(root
        .join("Runtime")
        .join("semantic-rewrite")
        .join(date_iso)
        .join(format!("{card_component}.json")))
}

fn append_card_entry(
    content: &str,
    date: NaiveDate,
    card_id: &str,
    occurred_at: &str,
    enriched: &EnrichedData,
) -> String {
    let header = format!("## {}", date.format("%a %b %d"));
    let time_label = occurred_at.get(11..16).unwrap_or("00:00");
    let compact_time = time_label.replace(':', "");
    let task_id = format!("{}-{}-{}", date.format("%Y-%m-%d"), compact_time, card_id);
    let entry_line = format!(
        "| -- | {} {} | + | <!--task:{}-->",
        time_label, enriched.title, task_id
    );
    let note_line = format!("<!--note:{}-->", task_id);
    let note_body = enriched
        .summary
        .clone()
        .unwrap_or_else(|| enriched.subtitle.clone().unwrap_or_default());

    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    if lines.is_empty() {
        lines.push(header.clone());
        lines.push("| Plan | Actual | Delta |".to_string());
        lines.push("|------|--------|---|".to_string());
        lines.push(entry_line);
        lines.push("---".to_string());
        lines.push(note_line);
        if !note_body.is_empty() {
            lines.push(note_body);
        }
        return lines.join("\n") + "\n";
    }

    let mut output = Vec::new();
    let mut inserted = false;
    let mut idx = 0;
    while idx < lines.len() {
        let line = &lines[idx];
        if !inserted && line.trim() == header {
            output.push(line.clone());
            idx += 1;
            while idx < lines.len() {
                let current = &lines[idx];
                if current.starts_with("## ") && current.trim() != header {
                    break;
                }
                if !inserted && current.trim() == "---" {
                    output.push(entry_line.clone());
                    inserted = true;
                }
                output.push(current.clone());
                idx += 1;
            }
            if !inserted {
                output.push(entry_line.clone());
                inserted = true;
            }
            output.push("---".to_string());
            output.push(note_line.clone());
            if !note_body.is_empty() {
                output.push(note_body.clone());
            }
            continue;
        }
        output.push(line.clone());
        idx += 1;
    }

    if !inserted {
        output.push("".to_string());
        output.push(header);
        output.push("| Plan | Actual | Delta |".to_string());
        output.push("|------|--------|---|".to_string());
        output.push(entry_line);
        output.push("---".to_string());
        output.push(note_line);
        if !note_body.is_empty() {
            output.push(note_body);
        }
    }

    output.join("\n") + "\n"
}

fn parse_occurred_at(value: &str) -> Result<NaiveDateTime, LifeStreamError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.naive_local())
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%z"))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f"))
        .map_err(|err| LifeStreamError::Parse(format!("Invalid timestamp {value}: {err}")))
}

fn looks_like_task_id(card_id: &str) -> bool {
    let bytes = card_id.as_bytes();
    if bytes.len() < 18 {
        return false;
    }
    bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes.get(10) == Some(&b'-')
        && bytes.get(15) == Some(&b'-')
}

fn task_id_for_card(card_id: &str, occurred_at_raw: &str, occurred_at: &NaiveDateTime) -> String {
    if looks_like_task_id(card_id) {
        return card_id.to_string();
    }
    let compact_time = occurred_at_raw
        .get(11..16)
        .map(|value| value.replace(':', ""))
        .filter(|value| value.len() == 4)
        .unwrap_or_else(|| {
            format!(
                "{:02}{:02}",
                occurred_at.time().hour(),
                occurred_at.time().minute()
            )
        });
    format!(
        "{}-{}-{}",
        occurred_at.date().format("%Y-%m-%d"),
        compact_time,
        card_id
    )
}

fn upsert_note_payload_comment(
    content: &str,
    note_id: &str,
    key: &str,
    value: &str,
) -> Result<(String, bool), LifeStreamError> {
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let marker = format!("<!--note:{note_id}-->");
    let marker_index = lines
        .iter()
        .position(|line| line.trim() == marker)
        .ok_or_else(|| LifeStreamError::Parse(format!("note marker not found: {note_id}")))?;

    let payload_start = marker_index + 1;
    let mut payload_end = lines.len();
    let mut index = payload_start;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.starts_with("<!--note:") || trimmed.starts_with("## ") {
            payload_end = index;
            break;
        }
        index += 1;
    }

    let new_line = format!("<!--{key}:{value}-->");
    let mut changed = false;
    let mut replacement_index: Option<usize> = None;
    for line_index in payload_start..payload_end {
        if let Some((existing_key, existing_value)) =
            parse_comment_key_value(lines[line_index].trim())
        {
            if existing_key == key {
                replacement_index = Some(line_index);
                if existing_value != value {
                    lines[line_index] = new_line.clone();
                    changed = true;
                }
                break;
            }
        }
    }

    if replacement_index.is_none() {
        lines.insert(payload_start, new_line);
        changed = true;
    }

    Ok((lines.join("\n") + "\n", changed))
}

fn parse_cards_from_stream(content: &str, date: NaiveDate, stream_file: &Path) -> Vec<StreamCard> {
    let lines: Vec<&str> = content.lines().collect();
    let mut entries = Vec::new();
    let mut notes_by_id: HashMap<String, NotePayload> = HashMap::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if let Some(parsed) = parse_header_date(line, date.year()) {
            current_date = Some(parsed);
            index += 1;
            continue;
        }

        if current_date != Some(date) {
            index += 1;
            continue;
        }

        if line.trim().starts_with("## ") {
            break;
        }

        if let Some(entry) = parse_table_entry(line, date) {
            entries.push(entry);
            index += 1;
            continue;
        }

        if let Some(note_id) = parse_note_marker(line) {
            index += 1;
            let mut block_lines = Vec::new();
            while index < lines.len() {
                let next = lines[index];
                if next.trim().starts_with("<!--note:") || next.trim().starts_with("## ") {
                    break;
                }
                block_lines.push(next.to_string());
                index += 1;
            }
            notes_by_id.insert(note_id, parse_note_payload(&block_lines));
            continue;
        }

        index += 1;
    }

    entries
        .into_iter()
        .map(|entry| {
            let note = notes_by_id.get(&entry.task_id).cloned().unwrap_or_default();
            let response_text = note.response_text();
            let summary = note.summary_text();
            let expanded = response_text.as_ref().map(|response| ExpandedContent {
                original_input: note.prompt.clone(),
                sections: vec![ExpandedSection {
                    title: "Codex Response".to_string(),
                    body: response.to_string(),
                }],
                entity_links: None,
                actions: Vec::new(),
            });
            let causal = note
                .semantic
                .clone()
                .unwrap_or_else(|| build_legacy_causal_content(&entry, &note));

            StreamCard {
                id: entry.task_id.clone(),
                occurred_at: entry.occurred_at.clone(),
                created_at: entry.occurred_at.clone(),
                updated_at: entry.occurred_at.clone(),
                version: 1,
                card_type: CardType::Generic,
                domain: DomainId::General,
                emoji: "📝".to_string(),
                layout_mode: LayoutMode::CauseEffect,
                causal: Some(causal),
                state: CardState::Complete,
                processing_step: None,
                processing_steps: None,
                title: entry.title.clone(),
                subtitle: None,
                summary,
                duration_ms: note.duration_ms,
                image: Some(CardImage {
                    url: None,
                    status: super::types::ImageStatus::Missing,
                    source: None,
                }),
                stats: None,
                entities: None,
                original_input: note.prompt,
                assistant_preview: None,
                request: None,
                source: Some(super::types::CardSource {
                    stream_file: Some(stream_file.to_string_lossy().to_string()),
                    stream_anchor: Some(entry.task_id.clone()),
                }),
                expanded,
                clarification_options: None,
                error_message: None,
            }
        })
        .collect()
}

fn build_legacy_causal_content(entry: &ParsedEntry, note: &NotePayload) -> CausalCardContent {
    let frame = infer_legacy_frame(note);
    let left_role = infer_legacy_left_role(frame, note.prompt.as_deref().unwrap_or(""));
    let right_role = infer_legacy_right_role(frame);
    let left_id = format!("{}:left:0", entry.task_id);
    let left_text = note
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| entry.title.clone());
    let left_title = infer_legacy_node_title(left_text.as_str(), entry.title.as_str());
    let left_summary = note
        .prompt
        .as_deref()
        .and_then(|value| first_sentence(value))
        .filter(|value| !value.eq_ignore_ascii_case(left_title.as_str()));

    let left_nodes = vec![CausalNode {
        id: left_id.clone(),
        text: left_text,
        headline: Some(left_title.clone()),
        summary_line: left_summary,
        title: Some(left_title),
        bullets: note
            .prompt
            .as_deref()
            .map(extract_legacy_bullets)
            .filter(|items| !items.is_empty()),
        details: note.prompt.clone(),
        role: Some(left_role),
        rank: Some(1),
        group_type: Some(super::types::CausalGroupType::Primary),
        is_image_applicable: true,
        image: None,
        entity: None,
        occurred_at: Some(entry.occurred_at.clone()),
    }];

    let mut right_texts = Vec::new();
    if let Some(response) = note.response_text() {
        let extracted = extract_legacy_points(&response, frame);
        if extracted.is_empty() {
            right_texts.push(response);
        } else {
            right_texts.extend(extracted);
        }
    }

    if let Some(summary) = note.summary_text() {
        let summary_trimmed = summary.trim();
        if !summary_trimmed.is_empty()
            && !right_texts
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(summary_trimmed))
        {
            right_texts.push(summary_trimmed.to_string());
        }
    }

    if right_texts.is_empty() {
        right_texts.push(entry.title.clone());
    }

    let right_nodes = right_texts
        .iter()
        .enumerate()
        .map(|(index, text)| CausalNode {
            id: format!("{}:right:{index}", entry.task_id),
            text: truncate_text(text, 220),
            headline: Some(truncate_text(text, 118)),
            summary_line: first_sentence(text),
            title: Some(truncate_text(text, 110)),
            bullets: Some(extract_legacy_bullets(text)).filter(|items| !items.is_empty()),
            details: Some(text.to_string()).filter(|value| !value.trim().is_empty()),
            role: Some(right_role.clone()),
            rank: Some((index + 1) as u32),
            group_type: Some(super::types::CausalGroupType::Primary),
            is_image_applicable: false,
            image: None,
            entity: None,
            occurred_at: None,
        })
        .collect::<Vec<_>>();

    let links = right_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| CausalLink {
            id: Some(format!("{}:link:{index}", entry.task_id)),
            from_id: left_id.clone(),
            to_id: node.id.clone(),
            label: None,
            strength: Some(if index < 3 { 1.0 } else { 0.55 }),
        })
        .collect::<Vec<_>>();

    let layout = if right_nodes.len() > 3 {
        Some(CausalLayoutState {
            visible_right_count: Some(3),
            top_link_limit: Some(3),
            expanded: Some(false),
        })
    } else {
        None
    };
    let right_count = right_nodes.len();

    CausalCardContent {
        left_nodes,
        right_nodes,
        links,
        layout,
        semantic_mode: Some(infer_legacy_semantic_mode(frame)),
        compaction: Some(super::types::CausalCompactionState {
            enabled: right_count > 3,
            threshold: 3,
            overflow_count: right_count
                .checked_sub(3)
                .map(|count| count as u32)
                .filter(|count| *count > 0),
        }),
        transcript_source: Some(super::types::CausalTranscriptSource::Both),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyFrameProfile {
    CauseEffect,
    ActionReward,
    ClaimResponse,
}

fn infer_legacy_semantic_mode(frame: LegacyFrameProfile) -> super::types::CausalSemanticMode {
    match frame {
        LegacyFrameProfile::CauseEffect => super::types::CausalSemanticMode::CauseEffect,
        LegacyFrameProfile::ActionReward => super::types::CausalSemanticMode::ActionReward,
        LegacyFrameProfile::ClaimResponse => super::types::CausalSemanticMode::StatementWhy,
    }
}

fn infer_legacy_frame(note: &NotePayload) -> LegacyFrameProfile {
    let prompt = note.prompt.as_deref().unwrap_or("").to_lowercase();

    if [
        "delivery",
        "shift",
        "order",
        "doordash",
        "uber",
        "grubhub",
        "instacart",
    ]
    .iter()
    .any(|keyword| prompt.contains(keyword))
    {
        return LegacyFrameProfile::ActionReward;
    }

    let has_headings = note
        .response
        .as_deref()
        .map(|text| text.lines().any(|line| line.trim_start().starts_with('#')))
        .unwrap_or(false);

    if has_headings
        || prompt.contains('?')
        || [
            "i think",
            "i feel",
            "i believe",
            "favorite",
            "should",
            "why",
            "how",
            "what if",
        ]
        .iter()
        .any(|marker| prompt.contains(marker))
    {
        return LegacyFrameProfile::ClaimResponse;
    }

    LegacyFrameProfile::CauseEffect
}

fn infer_legacy_left_role(frame: LegacyFrameProfile, prompt: &str) -> CausalNodeRole {
    match frame {
        LegacyFrameProfile::ActionReward => CausalNodeRole::Action,
        LegacyFrameProfile::ClaimResponse => {
            if prompt.contains('?') {
                CausalNodeRole::Question
            } else {
                CausalNodeRole::Cause
            }
        }
        LegacyFrameProfile::CauseEffect => CausalNodeRole::Cause,
    }
}

fn infer_legacy_right_role(frame: LegacyFrameProfile) -> CausalNodeRole {
    match frame {
        LegacyFrameProfile::ActionReward => CausalNodeRole::Reward,
        LegacyFrameProfile::ClaimResponse => CausalNodeRole::Response,
        LegacyFrameProfile::CauseEffect => CausalNodeRole::Effect,
    }
}

fn extract_legacy_points(text: &str, frame: LegacyFrameProfile) -> Vec<String> {
    if frame == LegacyFrameProfile::ClaimResponse {
        let headings = extract_legacy_headings(text);
        if !headings.is_empty() {
            return headings;
        }
    }

    let mut points = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let item = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .map(str::trim)
            .or_else(|| {
                let (prefix, rest) = trimmed.split_once(". ")?;
                if prefix.chars().all(|ch| ch.is_ascii_digit()) {
                    Some(rest.trim())
                } else {
                    None
                }
            });

        if let Some(value) = item {
            if !value.is_empty() {
                points.push(value.to_string());
            }
        }
    }

    points
}

fn extract_legacy_headings(text: &str) -> Vec<String> {
    let mut headings = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }

        let value = trimmed.trim_start_matches('#').trim();
        if value.is_empty() {
            continue;
        }

        headings.push(truncate_text(value, 180));
    }

    headings
}

fn extract_legacy_bullets(text: &str) -> Vec<String> {
    let mut bullets = Vec::new();
    for segment in text
        .split('\n')
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
        bullets.push(truncate_text(cleaned, 140));
        if bullets.len() >= 4 {
            break;
        }
    }
    bullets
}

fn first_sentence(text: &str) -> Option<String> {
    text.split('\n')
        .map(str::trim)
        .find(|line| !line.is_empty())
        .and_then(|line| line.split_terminator(['.', '!', '?']).next())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn infer_legacy_node_title(left_text: &str, fallback_title: &str) -> String {
    let fallback = fallback_title.trim();
    let source = left_text.trim();

    let first_sentence = first_sentence(source).unwrap_or_else(|| fallback.to_string());

    let normalized = first_sentence
        .to_lowercase()
        .replace(|ch: char| !ch.is_alphanumeric() && !ch.is_whitespace(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let is_generic = matches!(
        normalized.as_str(),
        "response" | "cause" | "effect" | "action" | "reward" | "question"
    );

    if is_generic {
        let first_bullet = extract_legacy_bullets(source)
            .into_iter()
            .next()
            .unwrap_or_else(|| source.to_string());
        return truncate_text(first_bullet.as_str(), 88);
    }

    truncate_text(first_sentence.as_str(), 88)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }

    let truncated: String = trimmed.chars().take(max_chars).collect();
    format!("{truncated}...")
}

#[derive(Debug, Clone, Default)]
struct NotePayload {
    prompt: Option<String>,
    response: Option<String>,
    body: Option<String>,
    duration_ms: Option<u64>,
    semantic: Option<CausalCardContent>,
}

impl NotePayload {
    fn response_text(&self) -> Option<String> {
        self.response
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn summary_text(&self) -> Option<String> {
        self.body
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| self.response_text())
    }
}

fn parse_note_marker(line: &str) -> Option<String> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("<!--note:")
        .and_then(|value| value.strip_suffix("-->"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_note_payload(lines: &[String]) -> NotePayload {
    let mut payload = NotePayload::default();
    let mut body_lines: Vec<String> = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some((key, value)) = parse_comment_key_value(trimmed) {
            match key {
                "prompt_b64" => payload.prompt = decode_b64(value),
                "response_b64" => payload.response = decode_b64(value),
                "prompt" => payload.prompt = Some(value.to_string()),
                "response" => payload.response = Some(value.to_string()),
                "duration_ms" => payload.duration_ms = value.parse::<u64>().ok(),
                "semantic_b64" => {
                    payload.semantic = decode_b64(value).and_then(|decoded| {
                        serde_json::from_str::<CausalCardContent>(&decoded).ok()
                    })
                }
                "semantic" => {
                    payload.semantic = serde_json::from_str::<CausalCardContent>(value).ok();
                }
                _ => {}
            }
            continue;
        }

        if trimmed == "---" && body_lines.is_empty() {
            continue;
        }
        body_lines.push(line.to_string());
    }

    while body_lines
        .last()
        .map(|line| line.trim())
        .is_some_and(|line| line.is_empty() || line == "---")
    {
        body_lines.pop();
    }

    let body = body_lines.join("\n").trim().to_string();
    if !body.is_empty() {
        payload.body = Some(body);
    }

    payload
}

fn parse_comment_key_value(line: &str) -> Option<(&str, &str)> {
    let inner = line.strip_prefix("<!--")?.strip_suffix("-->")?;
    let (key, value) = inner.split_once(':')?;
    Some((key.trim(), value.trim()))
}

fn decode_b64(value: &str) -> Option<String> {
    let decoded = BASE64_STANDARD.decode(value).ok()?;
    String::from_utf8(decoded)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

#[derive(Debug, PartialEq)]
pub(crate) struct ParsedEntry {
    pub(crate) task_id: String,
    pub(crate) occurred_at: String,
    pub(crate) title: String,
}

pub(crate) fn parse_table_entry(line: &str, date: NaiveDate) -> Option<ParsedEntry> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.contains("<!--task:") {
        return None;
    }
    let parts: Vec<&str> = trimmed.split('|').collect();
    if parts.len() < 4 {
        return None;
    }
    let actual = parts.get(2)?.trim();
    let time_title: Vec<&str> = actual.split_whitespace().collect();
    if time_title.is_empty() {
        return None;
    }
    let time = time_title[0];
    let title = time_title[1..].join(" ");
    let task_id = trimmed
        .split("<!--task:")
        .nth(1)
        .and_then(|v| v.split("-->").next())
        .unwrap_or("")
        .to_string();
    if task_id.is_empty() {
        return None;
    }
    let time_24 = normalize_time(time).unwrap_or_else(|| "00:00".to_string());
    let occurred_at = format!("{}T{}:00", date.format("%Y-%m-%d"), time_24);
    Some(ParsedEntry {
        task_id,
        occurred_at,
        title,
    })
}

fn parse_header_date(line: &str, year: i32) -> Option<NaiveDate> {
    let trimmed = line.trim();
    if !trimmed.starts_with("## ") {
        return None;
    }
    let parts: Vec<&str> = trimmed
        .trim_start_matches("## ")
        .split_whitespace()
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let month = month_number(parts[1])?;
    let day: u32 = parts[2].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn month_number(name: &str) -> Option<u32> {
    match name {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

pub(crate) fn normalize_time(value: &str) -> Option<String> {
    let lower = value.to_lowercase();
    let (time_part, suffix) = if lower.ends_with("am") {
        (lower.trim_end_matches("am"), "am")
    } else if lower.ends_with("pm") {
        (lower.trim_end_matches("pm"), "pm")
    } else {
        (lower.as_str(), "")
    };
    let parts: Vec<&str> = time_part.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let mut hour: i32 = parts[0].parse().ok()?;
    let minute = parts[1].parse::<i32>().ok()?;
    if suffix == "pm" && hour < 12 {
        hour += 12;
    }
    if suffix == "am" && hour == 12 {
        hour = 0;
    }
    Some(format!("{:02}:{:02}", hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn parses_table_entries_with_note_payload_metadata() {
        let content = r#"
# February 2026

## Mon Feb 02
| Plan | Actual | Delta |
|------|--------|---|
| -- | 06:37  | + | <!--task:2026-02-02-0637-test-->
| -- | 07:53 running test | + | <!--task:2026-02-02-0753-test-->
---
<!--note:2026-02-02-0637-test-->
<!--prompt_b64:SGVsbG8gd29ybGQ=-->
<!--response_b64:VGhpcyBpcyBhIHJlc3BvbnNlLg==-->
<!--duration_ms:1234-->
This is a response.
---
<!--note:2026-02-02-0753-test-->
<!--prompt_b64:VGVzdCBpbnB1dA==-->
Logged entry.

## Tue Feb 03
| Plan | Actual | Delta |
|------|--------|---|
"#;

        let date = NaiveDate::from_ymd_opt(2026, 2, 2).expect("valid date");
        let cards = parse_cards_from_stream(content, date, Path::new("/vault/Stream/2026-02.md"));
        assert_eq!(cards.len(), 2);

        let first = &cards[0];
        assert_eq!(first.id, "2026-02-02-0637-test");
        assert_eq!(first.original_input.as_deref(), Some("Hello world"));
        assert_eq!(first.duration_ms, Some(1234));
        assert_eq!(first.summary.as_deref(), Some("This is a response."));
        let expanded = first.expanded.as_ref().expect("expanded response section");
        assert_eq!(expanded.sections.len(), 1);
        assert_eq!(expanded.sections[0].title, "Codex Response");
        assert_eq!(expanded.sections[0].body, "This is a response.");

        let second = &cards[1];
        assert_eq!(second.id, "2026-02-02-0753-test");
        assert_eq!(second.original_input.as_deref(), Some("Test input"));
        assert_eq!(second.summary.as_deref(), Some("Logged entry."));
        assert!(second.expanded.is_none());
    }

    #[test]
    fn legacy_causal_content_prefers_markdown_headings_for_claim_response() {
        let content = r#"
# February 2026

## Mon Feb 02
| Plan | Actual | Delta |
|------|--------|---|
| -- | 22:32 Response | + | <!--task:2026-02-02-2232-test-->
---
<!--note:2026-02-02-2232-test-->
<!--prompt_b64:SSB0aGluayBlcGlzb2RlIDUgb2YgQ293Ym95IEJlYm9wIGlzIG15IGZhdm9yaXRlLiBTaG91bGQgaXQgaGF2ZSBiZWVuIGVwaXNvZGUgb25lPw==-->
<!--response_b64:IyMgV2h5IEVwIDUgZmVlbHMgbGlrZSB0aGUgcmVhbCBzaG93IHN0YXJ0cyBoZXJlCi0gYm91bnRpZXMKLSBzdHlsZQotIGphenoKCiMjIEl0IHNob3VsZCd2ZSBiZWVuIEVwaXNvZGUgMSDigJQgdHJhZGVvZmYKLSBZb3UgZ2FpbiBtb21lbnR1bQ==-->
"#;

        let date = NaiveDate::from_ymd_opt(2026, 2, 2).expect("valid date");
        let cards = parse_cards_from_stream(content, date, Path::new("/vault/Stream/2026-02.md"));
        assert_eq!(cards.len(), 1);

        let causal = cards[0].causal.as_ref().expect("causal graph");
        assert_eq!(causal.left_nodes.len(), 1);
        assert_eq!(causal.left_nodes[0].role, Some(CausalNodeRole::Question));
        assert!(causal.right_nodes.len() >= 2);
        assert_eq!(causal.right_nodes[0].role, Some(CausalNodeRole::Response));
        assert!(causal.right_nodes[0]
            .text
            .to_lowercase()
            .contains("real show starts here"));
        assert!(causal.right_nodes[1]
            .text
            .to_lowercase()
            .contains("tradeoff"));
    }

    #[test]
    fn semantic_payload_preferred_when_present() {
        let semantic = json!({
            "leftNodes": [{
                "id": "2026-02-02-2232-test:left:0",
                "text": "Input",
                "headline": "Custom statement title",
                "title": "Custom statement title",
                "role": "cause",
                "rank": 1,
                "groupType": "primary",
                "isImageApplicable": true
            }],
            "rightNodes": [{
                "id": "2026-02-02-2232-test:right:0",
                "text": "Custom reason",
                "headline": "Custom reason",
                "title": "Custom reason",
                "bullets": ["Point A", "Point B"],
                "role": "response",
                "rank": 1,
                "groupType": "primary",
                "isImageApplicable": false
            }],
            "links": [{
                "id": "2026-02-02-2232-test:link:0",
                "fromId": "2026-02-02-2232-test:left:0",
                "toId": "2026-02-02-2232-test:right:0",
                "strength": 1.0
            }],
            "semanticMode": "statement_why",
            "compaction": {
                "enabled": false,
                "threshold": 3
            },
            "transcriptSource": "both"
        });
        let semantic_b64 = BASE64_STANDARD.encode(semantic.to_string());

        let content = format!(
            r#"
# February 2026

## Mon Feb 02
| Plan | Actual | Delta |
|------|--------|---|
| -- | 22:32 Response | + | <!--task:2026-02-02-2232-test-->
---
<!--note:2026-02-02-2232-test-->
<!--prompt:Input -->
<!--semantic_b64:{}-->
"#,
            semantic_b64
        );

        let date = NaiveDate::from_ymd_opt(2026, 2, 2).expect("valid date");
        let cards = parse_cards_from_stream(
            content.as_str(),
            date,
            Path::new("/vault/Stream/2026-02.md"),
        );
        assert_eq!(cards.len(), 1);
        let causal = cards[0].causal.as_ref().expect("causal graph");
        assert_eq!(
            causal.left_nodes[0].headline.as_deref(),
            Some("Custom statement title")
        );
        assert_eq!(
            causal.right_nodes[0].headline.as_deref(),
            Some("Custom reason")
        );
        assert_eq!(
            causal.right_nodes[0]
                .bullets
                .as_ref()
                .map(|value| value.len()),
            Some(2)
        );
    }

    #[test]
    fn upsert_note_payload_comment_replaces_existing_value() {
        let content = r#"
## Mon Feb 02
| Plan | Actual | Delta |
|------|--------|---|
| -- | 22:32 Response | + | <!--task:2026-02-02-2232-test-->
---
<!--note:2026-02-02-2232-test-->
<!--prompt:Input -->
<!--semantic_b64:old-->
Body line
"#;
        let (updated, changed) = upsert_note_payload_comment(
            content,
            "2026-02-02-2232-test",
            "semantic_b64",
            "new-value",
        )
        .expect("updated");
        assert!(changed);
        assert!(updated.contains("<!--semantic_b64:new-value-->"));
        assert!(!updated.contains("<!--semantic_b64:old-->"));
    }

    #[tokio::test]
    async fn day_thread_runtime_state_round_trip() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().to_string_lossy().to_string();
        let io = ObsidianIO::new(Some(root.clone()));
        let payload = DayThreadRuntimeState {
            date: "2026-02-06".to_string(),
            thread_id: "thread_abc123".to_string(),
            last_seed_hash: Some("seed-hash".to_string()),
            card_count: 18,
            updated_at: "2026-02-06T18:42:00Z".to_string(),
        };

        io.write_day_thread_runtime_state("/tmp/workspace", Some(&root), &payload)
            .await
            .expect("write payload");

        let loaded = io
            .read_day_thread_runtime_state("/tmp/workspace", Some(&root), "2026-02-06")
            .await
            .expect("read payload")
            .expect("payload exists");
        assert_eq!(loaded, payload);
    }

    #[tokio::test]
    async fn write_semantic_rewrite_log_sanitizes_card_filename() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().to_string_lossy().to_string();
        let io = ObsidianIO::new(Some(root.clone()));
        let payload = SemanticRewriteLogEntry {
            card_id: "2026-02-06-1234-card".to_string(),
            date: "2026-02-06".to_string(),
            thread_id: "thread_abc123".to_string(),
            prompt: "Prompt body".to_string(),
            raw_response: "{\"ok\":true}".to_string(),
            parsed_json: Some(json!({ "ok": true })),
            failure_reason: None,
            started_at: "2026-02-06T18:40:00Z".to_string(),
            completed_at: "2026-02-06T18:40:01Z".to_string(),
            attempts: vec![SemanticRewriteLogAttempt {
                stage: "initial".to_string(),
                prompt: "Prompt body".to_string(),
                raw_response: "{\"ok\":true}".to_string(),
                parsed_json: Some(json!({ "ok": true })),
                failure_reason: None,
                event_trace: Vec::new(),
                trace_stats: None,
            }],
            trace_stats: None,
        };

        io.write_semantic_rewrite_log(
            "/tmp/workspace",
            Some(&root),
            "2026-02-06",
            "card/unsafe:name",
            &payload,
        )
        .await
        .expect("write log");

        let expected_path = Path::new(&root)
            .join("Runtime")
            .join("semantic-rewrite")
            .join("2026-02-06")
            .join("card_unsafe_name.json");
        assert!(
            expected_path.exists(),
            "expected log file at {}",
            expected_path.display()
        );
    }
}
