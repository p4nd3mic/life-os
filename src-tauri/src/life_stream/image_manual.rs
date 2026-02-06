use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::fs;
use uuid::Uuid;

use super::images::{CandidateProviderConfig, CandidateProviderRegistry};
use super::types::{CardType, CausalNode, DomainId, ImageAssetRecord, ImageCandidate, StreamCard};

const DEFAULT_EXTERNAL_PHOTO_ROOT: &str = "/Volumes/YouTube 4TB/photos";
const MAX_CANDIDATES: usize = 64;
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif"];

#[derive(Debug, Clone)]
pub(crate) struct ResolvedEntity {
    pub entity_key: String,
    pub entity_type: String,
    pub entity_name: String,
    pub entity_slug: String,
    pub entity_link: Option<String>,
    pub context_text: Option<String>,
}

#[derive(Debug, Clone)]
struct RankedFile {
    path: PathBuf,
    file_name: String,
    source_kind: String,
    score: i64,
    reasons: Vec<String>,
    is_managed: bool,
    modified: Option<SystemTime>,
}

pub(crate) fn resolve_entity_for_card(card: &StreamCard, node_id: Option<&str>) -> ResolvedEntity {
    let target_node = node_id.and_then(|id| find_node(card, id));
    let context_text = target_node
        .map(|node| node.text.trim().to_string())
        .or_else(|| card.original_input.clone());

    let node_entity = target_node.and_then(|node| node.entity.clone());
    let card_entity = card.entities.as_ref().and_then(|entities| entities.first().cloned());
    let selected_entity = node_entity.as_ref().or(card_entity.as_ref());
    let wiki_entity = node_entity
        .as_ref()
        .and_then(|entity| parse_entity_from_link(entity.link.as_deref()))
        .or_else(|| parse_entity_from_text(target_node.map(|node| node.text.as_str()).unwrap_or("")));
    let fallback_wiki = parse_entity_from_text(card.title.as_str())
        .or_else(|| parse_entity_from_text(card.original_input.as_deref().unwrap_or("")));

    let (entity_type, entity_name) = if let Some(entity) = node_entity.as_ref().or(card_entity.as_ref()) {
        let resolved_type = normalize_entity_type(
            Some(entity.entity_type.as_str()),
            Some(card.card_type.clone()),
            Some(card.domain.clone()),
        );
        let resolved_name = entity
            .name
            .trim()
            .to_string();
        (resolved_type, resolved_name)
    } else if let Some((wiki_type, wiki_name)) = wiki_entity.or(fallback_wiki) {
        let resolved_type = normalize_entity_type(Some(wiki_type.as_str()), Some(card.card_type.clone()), Some(card.domain.clone()));
        (resolved_type, wiki_name)
    } else {
        let fallback_type = normalize_entity_type(None, Some(card.card_type.clone()), Some(card.domain.clone()));
        let fallback_name = target_node
            .map(|node| summarize_entity_name(&node.text))
            .unwrap_or_else(|| summarize_entity_name(&card.title));
        (fallback_type, fallback_name)
    };

    let entity_slug = slugify(&entity_name);
    let entity_key = format!("{entity_type}:{entity_slug}");

    ResolvedEntity {
        entity_key,
        entity_type,
        entity_name,
        entity_slug,
        entity_link: selected_entity.and_then(|entity| entity.link.clone()),
        context_text,
    }
}

pub(crate) async fn find_image_candidates(
    obsidian_root: &Path,
    entity: &ResolvedEntity,
    limit: usize,
    tmdb_api_key: Option<&str>,
) -> Result<Vec<ImageCandidate>, String> {
    let managed_dir = managed_entity_dir(obsidian_root, entity);
    let mut directories = vec![managed_dir.clone()];
    directories.extend(source_directories(entity).await?);

    let mut seen = HashSet::new();
    let mut ranked = Vec::new();
    for directory in directories {
        if !directory.exists() {
            continue;
        }
        let is_managed = directory.starts_with(obsidian_root);
        let mut files = collect_image_files(&directory)?;
        for file in files.drain(..) {
            let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());
            let key = canonical.to_string_lossy().to_string();
            if !seen.insert(key) {
                continue;
            }
            let Some(file_name) = file.file_name().map(|value| value.to_string_lossy().to_string()) else {
                continue;
            };
            let metadata = std::fs::metadata(&file).ok();
            let modified = metadata.and_then(|meta| meta.modified().ok());
            let source_kind = if is_managed {
                "managed_local".to_string()
            } else {
                "external_local".to_string()
            };
            let (score, reasons) = score_file(&file_name, entity, is_managed, source_kind.as_str());
            ranked.push(RankedFile {
                path: file,
                file_name,
                source_kind,
                score,
                reasons,
                is_managed,
                modified,
            });
        }
    }

    let remote_candidates =
        fetch_remote_candidates(obsidian_root, entity, tmdb_api_key, take_limit(limit)).await?;
    for candidate in remote_candidates {
        let canonical = candidate
            .path
            .canonicalize()
            .unwrap_or_else(|_| candidate.path.clone());
        let key = canonical.to_string_lossy().to_string();
        if seen.insert(key) {
            ranked.push(candidate);
        }
    }

    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.modified.cmp(&a.modified))
            .then_with(|| a.file_name.cmp(&b.file_name))
    });

    let take_limit = take_limit(limit);
    Ok(ranked
        .into_iter()
        .take(take_limit)
        .map(|item| ImageCandidate {
            source_path: item.path.to_string_lossy().to_string(),
            source_kind: item.source_kind,
            score: item.score,
            reason: item.reasons,
            file_name: item.file_name,
            is_managed: item.is_managed,
        })
        .collect())
}

pub(crate) async fn import_image_asset(
    obsidian_root: &Path,
    entity: &ResolvedEntity,
    source_path: &Path,
) -> Result<ImageAssetRecord, String> {
    let canonical_source = source_path.canonicalize().map_err(|error| {
        format!(
            "Image source not accessible {}: {}",
            source_path.display(),
            error
        )
    })?;
    validate_source_path(obsidian_root, &canonical_source)?;

    let extension = canonical_source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or("Image source missing file extension")?;
    if !IMAGE_EXTENSIONS.iter().any(|allowed| *allowed == extension) {
        return Err(format!("Unsupported image extension: .{}", extension));
    }

    let bytes = fs::read(&canonical_source).await.map_err(|error| {
        format!(
            "Failed to read image source {}: {}",
            canonical_source.display(),
            error
        )
    })?;
    let sha256 = hash_bytes(&bytes);
    let mime = mime_from_extension(extension.as_str()).to_string();

    let obsidian_root_canonical = obsidian_root.canonicalize().map_err(|error| {
        format!(
            "Cannot canonicalize obsidian root {}: {}",
            obsidian_root.display(),
            error
        )
    })?;
    let runtime_inbox_root = obsidian_root_canonical.join("Runtime").join("ImageInbox");
    let source_kind = if canonical_source.starts_with(&runtime_inbox_root) {
        "provider_tmdb".to_string()
    } else if canonical_source.starts_with(&obsidian_root_canonical) {
        "managed_local".to_string()
    } else {
        "external_local".to_string()
    };

    let managed_root = managed_entity_dir(&obsidian_root_canonical, entity);
    fs::create_dir_all(&managed_root).await.map_err(|error| {
        format!(
            "Failed creating managed image directory {}: {}",
            managed_root.display(),
            error
        )
    })?;

    let relative_path = if canonical_source.starts_with(&obsidian_root_canonical) {
        canonical_source
            .strip_prefix(&obsidian_root_canonical)
            .map_err(|error| format!("Failed building relative image path: {}", error))?
            .to_path_buf()
    } else {
        let source_stem = canonical_source
            .file_stem()
            .and_then(|value| value.to_str())
            .map(slugify)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "image".to_string());
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let file_name = format!("{timestamp}--{source_stem}.{extension}");
        let relative = PathBuf::from("Assets")
            .join("Entities")
            .join(&entity.entity_type)
            .join(&entity.entity_slug)
            .join(file_name);
        let destination = obsidian_root_canonical.join(&relative);
        fs::write(&destination, &bytes).await.map_err(|error| {
            format!(
                "Failed writing managed image {}: {}",
                destination.display(),
                error
            )
        })?;
        relative
    };

    let tags = tags_from_file_name(
        canonical_source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
    );

    Ok(ImageAssetRecord {
        id: format!("img_{}", Uuid::new_v4().simple()),
        relative_path: normalize_relative_path(&relative_path),
        source_path: canonical_source.to_string_lossy().to_string(),
        source_kind,
        sha256,
        mime,
        created_at: Utc::now().to_rfc3339(),
        tags,
    })
}

pub(crate) fn absolute_from_relative(obsidian_root: &Path, relative: &str) -> PathBuf {
    obsidian_root.join(relative)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EntityFileSyncOptions {
    pub update_frontmatter: bool,
    pub update_embed_block: bool,
}

pub(crate) async fn sync_entity_file_image(
    obsidian_root: &Path,
    entity: &ResolvedEntity,
    relative_path: &str,
    options: EntityFileSyncOptions,
) -> Result<bool, String> {
    if !options.update_frontmatter && !options.update_embed_block {
        return Ok(false);
    }

    let Some(entity_path) = resolve_entity_file_path(obsidian_root, entity) else {
        return Ok(false);
    };
    if !entity_path.exists() {
        return Ok(false);
    }

    let current = fs::read_to_string(&entity_path).await.map_err(|error| {
        format!(
            "Failed reading entity file {}: {}",
            entity_path.display(),
            error
        )
    })?;
    let next = update_entity_markdown_content(&current, relative_path, options);
    if next == current {
        return Ok(false);
    }

    fs::write(&entity_path, next).await.map_err(|error| {
        format!(
            "Failed writing entity file {}: {}",
            entity_path.display(),
            error
        )
    })?;

    Ok(true)
}

pub(crate) fn update_entity_markdown_content(
    content: &str,
    relative_path: &str,
    options: EntityFileSyncOptions,
) -> String {
    let image_value = format!("\"{}\"", relative_path);
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();

    if options.update_frontmatter {
        lines = upsert_frontmatter_image(lines, image_value.as_str());
    }

    if options.update_embed_block {
        lines = upsert_embed_block(lines, relative_path);
    }

    let mut output = lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn find_node<'a>(card: &'a StreamCard, node_id: &str) -> Option<&'a CausalNode> {
    let causal = card.causal.as_ref()?;
    causal
        .left_nodes
        .iter()
        .chain(causal.right_nodes.iter())
        .find(|node| node.id == node_id)
}

fn parse_entity_from_link(link: Option<&str>) -> Option<(String, String)> {
    let link = link?;
    let trimmed = link.trim().trim_start_matches("[[").trim_end_matches("]]");
    if trimmed.is_empty() {
        return None;
    }
    let mut segments = trimmed.split('/').collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }
    if segments[0].eq_ignore_ascii_case("Entities") && segments.len() >= 3 {
        let entity_type = segments.get(1)?.to_string();
        let name = segments.get(2..)?.join("/");
        return Some((entity_type, name));
    }
    if segments.len() >= 2 {
        let entity_type = segments.remove(0).to_string();
        let name = segments.join("/");
        return Some((entity_type, name));
    }
    None
}

fn parse_entity_from_text(text: &str) -> Option<(String, String)> {
    let start = text.find("[[")?;
    let rest = &text[start + 2..];
    let end = rest.find("]]")?;
    parse_entity_from_link(Some(&rest[..end]))
}

fn resolve_entity_file_path(obsidian_root: &Path, entity: &ResolvedEntity) -> Option<PathBuf> {
    if let Some(link) = entity.entity_link.as_ref() {
        if let Some(path) = entity_path_from_link(obsidian_root, link) {
            return Some(path);
        }
    }

    let folder = entity_folder_for_type(entity.entity_type.as_str());
    Some(
        obsidian_root
            .join("Entities")
            .join(folder)
            .join(format!("{}.md", entity.entity_name)),
    )
}

fn entity_path_from_link(obsidian_root: &Path, link: &str) -> Option<PathBuf> {
    let trimmed = link.trim().trim_start_matches("[[").trim_end_matches("]]");
    if trimmed.is_empty() {
        return None;
    }

    let mut without_alias = trimmed.split('|').next()?.trim().to_string();
    if without_alias.ends_with(".md") {
        without_alias = without_alias.trim_end_matches(".md").to_string();
    }

    let path = if without_alias.starts_with("Entities/") {
        obsidian_root.join(format!("{without_alias}.md"))
    } else if without_alias.contains('/') {
        obsidian_root.join(format!("Entities/{without_alias}.md"))
    } else {
        obsidian_root.join(format!("Entities/Notes/{without_alias}.md"))
    };
    Some(path)
}

fn entity_folder_for_type(entity_type: &str) -> &'static str {
    match entity_type {
        "media" => "Media",
        "food" => "Food",
        "delivery" => "Delivery",
        "fitness" => "Fitness",
        "finance" => "Finance",
        "people" => "People",
        "youtube" => "YouTube",
        "geography" => "Topics",
        _ => "Notes",
    }
}

fn upsert_frontmatter_image(mut lines: Vec<String>, image_value: &str) -> Vec<String> {
    if lines.is_empty() {
        return vec![
            "---".to_string(),
            format!("image: {image_value}"),
            "---".to_string(),
            String::new(),
        ];
    }

    if lines.first().is_some_and(|line| line.trim() == "---") {
        let closing = lines
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(index, line)| (line.trim() == "---").then_some(index));
        if let Some(closing_index) = closing {
            let mut replaced = false;
            for line in lines.iter_mut().take(closing_index).skip(1) {
                if line.trim_start().starts_with("image:") {
                    *line = format!("image: {image_value}");
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                lines.insert(closing_index, format!("image: {image_value}"));
            }
            return lines;
        }
    }

    let mut next = Vec::with_capacity(lines.len() + 4);
    next.push("---".to_string());
    next.push(format!("image: {image_value}"));
    next.push("---".to_string());
    next.push(String::new());
    next.extend(lines);
    next
}

fn upsert_embed_block(mut lines: Vec<String>, relative_path: &str) -> Vec<String> {
    let marker = "<!--life-stream:image-embed-->";
    let embed_line = format!("![[{relative_path}]]");

    if let Some(index) = lines.iter().position(|line| line.trim() == marker) {
        if lines.get(index + 1).is_some_and(|line| line.trim().starts_with("![[")) {
            lines[index + 1] = embed_line;
        } else {
            lines.insert(index + 1, embed_line);
        }
        return lines;
    }

    if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.push(String::new());
    }
    lines.push("## Image".to_string());
    lines.push(marker.to_string());
    lines.push(embed_line);
    lines
}

fn normalize_entity_type(
    entity_type: Option<&str>,
    card_type: Option<CardType>,
    domain: Option<DomainId>,
) -> String {
    if let Some(entity_type) = entity_type {
        let lowered = entity_type.to_ascii_lowercase();
        let normalized = lowered
            .replace("entities/", "")
            .replace("entity/", "")
            .replace(' ', "");
        return match normalized.as_str() {
            "anime" | "movie" | "show" | "tv" | "game" | "mediaadd" => "media".to_string(),
            "person" | "people" => "people".to_string(),
            "location" | "place" | "geography" => "geography".to_string(),
            "food" | "meal" => "food".to_string(),
            other if !other.is_empty() => other.to_string(),
            _ => "general".to_string(),
        };
    }

    if let Some(card_type) = card_type {
        return match card_type {
            CardType::MediaAdd | CardType::Music => "media".to_string(),
            CardType::Meal => "food".to_string(),
            CardType::DeliveryOrder | CardType::DeliverySession => "delivery".to_string(),
            CardType::Query | CardType::Thought | CardType::Generic => "general".to_string(),
            CardType::CodeTask => "project".to_string(),
        };
    }

    if let Some(domain) = domain {
        return match domain {
            DomainId::Media => "media".to_string(),
            DomainId::Nutrition => "food".to_string(),
            DomainId::Delivery => "delivery".to_string(),
            DomainId::Fitness => "fitness".to_string(),
            DomainId::Finance => "finance".to_string(),
            DomainId::Youtube => "youtube".to_string(),
            DomainId::General => "general".to_string(),
        };
    }

    "general".to_string()
}

fn summarize_entity_name(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Untitled".to_string();
    }

    trimmed
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

async fn source_directories(entity: &ResolvedEntity) -> Result<Vec<PathBuf>, String> {
    let mut directories = Vec::new();

    let root = PathBuf::from(DEFAULT_EXTERNAL_PHOTO_ROOT);
    if !root.exists() {
        return Ok(directories);
    }

    let mapped_folder = source_folder_for_entity_type(&entity.entity_type);
    let mut parents = vec![root.join(mapped_folder)];
    let lower_folder = mapped_folder.to_ascii_lowercase();
    if lower_folder != mapped_folder {
        parents.push(root.join(lower_folder));
    }

    for parent in parents {
        directories.push(parent.join(&entity.entity_name));
        directories.push(parent.join(&entity.entity_slug));

        if parent.exists() {
            let entries = std::fs::read_dir(&parent).map_err(|error| {
                format!(
                    "Failed reading image source directory {}: {}",
                    parent.display(),
                    error
                )
            })?;
            for entry in entries {
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let slug = slugify(&name);
                if slug == entity.entity_slug {
                    directories.push(path);
                }
            }
        }
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for directory in directories {
        let key = directory.to_string_lossy().to_string();
        if seen.insert(key) {
            deduped.push(directory);
        }
    }

    Ok(deduped)
}

fn source_folder_for_entity_type(entity_type: &str) -> &'static str {
    match entity_type {
        "media" => "Media",
        "people" => "People",
        "geography" => "Geography",
        "delivery" => "Delivery",
        "food" => "Food",
        "fitness" => "Fitness",
        _ => "Media",
    }
}

fn managed_entity_dir(obsidian_root: &Path, entity: &ResolvedEntity) -> PathBuf {
    obsidian_root
        .join("Assets")
        .join("Entities")
        .join(&entity.entity_type)
        .join(&entity.entity_slug)
}

fn collect_image_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(directory).map_err(|error| {
        format!(
            "Failed reading candidate directory {}: {}",
            directory.display(),
            error
        )
    })?;

    let mut files = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        if extension
            .as_deref()
            .is_some_and(|value| IMAGE_EXTENSIONS.iter().any(|allowed| *allowed == value))
        {
            files.push(path);
        }
    }

    Ok(files)
}

fn score_file(
    file_name: &str,
    entity: &ResolvedEntity,
    is_managed: bool,
    source_kind: &str,
) -> (i64, Vec<String>) {
    let mut score = 0i64;
    let mut reasons = Vec::new();
    let lower = file_name.to_ascii_lowercase();

    if lower.contains("(fav)") || lower.contains("favorite") || lower.contains("fav") {
        score += 120;
        reasons.push("favorite-tag".to_string());
    }
    if lower.contains("main") {
        score += 70;
        reasons.push("main-tag".to_string());
    }
    if lower.contains("cover") || lower.contains("poster") {
        score += 60;
        reasons.push("cover-tag".to_string());
    }
    if lower.contains("banner") || lower.contains("keyart") || lower.contains("key-art") {
        score += 36;
        reasons.push("keyart-tag".to_string());
    }
    if is_managed {
        score += 20;
        reasons.push("managed-asset".to_string());
    }
    if source_kind.starts_with("provider_") {
        score += 28;
        reasons.push(source_kind.to_string());
    }

    let entity_tokens = extract_search_tokens(&entity.entity_name, 8);
    let mut entity_hits = 0;
    for token in &entity_tokens {
        if lower.contains(token) {
            entity_hits += 1;
        }
    }
    if entity_hits > 0 {
        score += (entity_hits as i64) * 14;
        reasons.push(format!("entity-match:{entity_hits}"));
    } else if lower.contains(entity.entity_slug.as_str()) {
        score += 12;
        reasons.push("entity-slug-match".to_string());
    }

    if let Some(context) = entity.context_text.as_ref() {
        let mut context_hits = 0usize;
        for token in extract_search_tokens(context, 10) {
            if lower.contains(&token) {
                context_hits += 1;
            }
        }
        if context_hits > 0 {
            score += (context_hits as i64) * 12;
            reasons.push(format!("context-match:{context_hits}"));
        }
    }

    if lower.ends_with(".webp") {
        score += 5;
    } else if lower.ends_with(".png") {
        score += 4;
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        score += 3;
    }

    (score, reasons)
}

fn take_limit(limit: usize) -> usize {
    limit.min(MAX_CANDIDATES).max(1)
}

fn extract_search_tokens(value: &str, max: usize) -> Vec<String> {
    let mut tokens = Vec::new();
    for token in value
        .split(|ch: char| !ch.is_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
    {
        if token.len() < 3 {
            continue;
        }
        if tokens.iter().any(|existing| existing == &token) {
            continue;
        }
        tokens.push(token);
        if tokens.len() >= max {
            break;
        }
    }
    tokens
}

async fn fetch_remote_candidates(
    obsidian_root: &Path,
    entity: &ResolvedEntity,
    tmdb_api_key: Option<&str>,
    limit: usize,
) -> Result<Vec<RankedFile>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let registry = CandidateProviderRegistry::new(CandidateProviderConfig {
        tmdb_api_key: tmdb_api_key.map(|value| value.to_string()),
    });
    let remote = registry
        .fetch_candidates(
            entity.entity_type.as_str(),
            entity.entity_name.as_str(),
            entity.context_text.as_deref(),
            limit,
        )
        .await?;

    if remote.is_empty() {
        return Ok(Vec::new());
    }

    let inbox_root = obsidian_root
        .join("Runtime")
        .join("ImageInbox")
        .join(&entity.entity_type)
        .join(&entity.entity_slug);
    fs::create_dir_all(&inbox_root).await.map_err(|error| {
        format!(
            "Failed creating image inbox {}: {}",
            inbox_root.display(),
            error
        )
    })?;

    let mut results = Vec::new();
    for (index, candidate) in remote.into_iter().enumerate() {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let base_name = candidate
            .file_name_hint
            .trim()
            .trim_end_matches(".jpg")
            .trim_end_matches(".jpeg")
            .trim_end_matches(".png")
            .to_string();
        let base_name = if base_name.is_empty() {
            "provider-candidate".to_string()
        } else {
            let slug = slugify(base_name.as_str());
            if slug.is_empty() {
                "provider-candidate".to_string()
            } else {
                slug
            }
        };
        let file_name = format!(
            "{timestamp}-{index:02}-{base_name}.jpg",
            index = index,
            base_name = base_name
        );
        let path = inbox_root.join(file_name.clone());
        fs::write(&path, candidate.bytes).await.map_err(|error| {
            format!(
                "Failed writing fetched image candidate {}: {}",
                path.display(),
                error
            )
        })?;

        let (score, mut reasons) =
            score_file(&file_name, entity, false, candidate.source_kind.as_str());
        reasons.extend(candidate.reasons);

        results.push(RankedFile {
            path,
            file_name,
            source_kind: candidate.source_kind,
            score: score + candidate.score_boost,
            reasons,
            is_managed: false,
            modified: Some(SystemTime::now()),
        });
    }

    Ok(results)
}

fn validate_source_path(obsidian_root: &Path, source_path: &Path) -> Result<(), String> {
    let obsidian_root = obsidian_root.canonicalize().map_err(|error| {
        format!(
            "Cannot canonicalize obsidian root {}: {}",
            obsidian_root.display(),
            error
        )
    })?;
    let mut allowed_roots = vec![obsidian_root];

    let photos_root = PathBuf::from(DEFAULT_EXTERNAL_PHOTO_ROOT);
    if photos_root.exists() {
        if let Ok(canonical) = photos_root.canonicalize() {
            allowed_roots.push(canonical);
        }
    }

    if allowed_roots
        .iter()
        .any(|allowed| source_path.starts_with(allowed))
    {
        return Ok(());
    }

    Err(format!(
        "Image source {} is outside allowed roots",
        source_path.display()
    ))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn mime_from_extension(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn tags_from_file_name(file_name: &str) -> Vec<String> {
    let lower = file_name.to_ascii_lowercase();
    let mut tags = Vec::new();
    if lower.contains("(fav)") || lower.contains("favorite") || lower.contains("fav") {
        tags.push("fav".to_string());
    }
    if lower.contains("cover") {
        tags.push("cover".to_string());
    }
    if lower.contains("poster") {
        tags.push("poster".to_string());
    }
    if lower.contains("main") {
        tags.push("main".to_string());
    }
    if lower.contains("episode") {
        tags.push("episode".to_string());
    }
    tags
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_entity_type, parse_entity_from_link, slugify, update_entity_markdown_content,
        EntityFileSyncOptions,
    };
    use crate::life_stream::types::{CardType, DomainId};

    #[test]
    fn slugify_collapses_symbols() {
        assert_eq!(slugify("Cowboy Bebop"), "cowboy-bebop");
        assert_eq!(slugify("Episode 5: Ballad!"), "episode-5-ballad");
    }

    #[test]
    fn parse_entity_from_wiki_link() {
        let parsed = parse_entity_from_link(Some("[[Entities/Media/Cowboy Bebop]]")).expect("parsed");
        assert_eq!(parsed.0, "Media");
        assert_eq!(parsed.1, "Cowboy Bebop");
    }

    #[test]
    fn normalize_entity_type_from_card_type() {
        let entity_type = normalize_entity_type(None, Some(CardType::MediaAdd), Some(DomainId::General));
        assert_eq!(entity_type, "media");
    }

    #[test]
    fn update_entity_markdown_sets_frontmatter_image() {
        let original = "---\ntitle: Cowboy Bebop\n---\n\nBody\n";
        let updated = update_entity_markdown_content(
            original,
            "Assets/Entities/media/cowboy/cover.jpg",
            EntityFileSyncOptions {
                update_frontmatter: true,
                update_embed_block: false,
            },
        );
        assert!(updated.contains("image: \"Assets/Entities/media/cowboy/cover.jpg\""));
    }

    #[test]
    fn update_entity_markdown_updates_embed_marker() {
        let original = "## Image\n<!--life-stream:image-embed-->\n![[old/path.jpg]]\n";
        let updated = update_entity_markdown_content(
            original,
            "Assets/Entities/media/cowboy/new.jpg",
            EntityFileSyncOptions {
                update_frontmatter: false,
                update_embed_block: true,
            },
        );
        assert!(updated.contains("![[Assets/Entities/media/cowboy/new.jpg]]"));
        assert!(!updated.contains("old/path.jpg"));
    }
}
