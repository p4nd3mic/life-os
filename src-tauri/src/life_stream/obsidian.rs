use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use tokio::fs;

use super::service::EnrichedData;
use super::types::{
    CardImage, CardState, CardType, DomainId, ExpandedContent, ExpandedSection, LifeStreamError,
    StreamCard,
};

#[derive(Clone)]
pub struct ObsidianIO {
    obsidian_root: Option<String>,
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

            StreamCard {
                id: entry.task_id.clone(),
                occurred_at: entry.occurred_at.clone(),
                created_at: entry.occurred_at.clone(),
                updated_at: entry.occurred_at.clone(),
                version: 1,
                card_type: CardType::Generic,
                domain: DomainId::General,
                emoji: "📝".to_string(),
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

#[derive(Debug, Clone, Default, PartialEq)]
struct NotePayload {
    prompt: Option<String>,
    response: Option<String>,
    body: Option<String>,
    duration_ms: Option<u64>,
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
}
