use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use tokio::fs;
use uuid::Uuid;

use super::types::ImageAssetRecord;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageCatalog {
    pub version: u32,
    pub updated_at: String,
    #[serde(default)]
    pub entities: HashMap<String, ImageCatalogEntity>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageCatalogEntity {
    pub entity_type: String,
    pub entity_name: String,
    pub entity_slug: String,
    #[serde(default)]
    pub primary_asset_id: Option<String>,
    #[serde(default)]
    pub assets: Vec<ImageAssetRecord>,
    #[serde(default)]
    pub context_overrides: Vec<ImageContextOverride>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageContextOverride {
    pub id: String,
    pub asset_id: String,
    #[serde(default)]
    pub match_tokens: Vec<String>,
    #[serde(default)]
    pub label: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for ImageCatalog {
    fn default() -> Self {
        Self {
            version: 1,
            updated_at: Utc::now().to_rfc3339(),
            entities: HashMap::new(),
        }
    }
}

pub(crate) fn catalog_path(obsidian_root: &Path) -> PathBuf {
    obsidian_root.join("Indexes").join("images.catalog.v1.json")
}

pub(crate) async fn load_catalog(obsidian_root: &Path) -> Result<ImageCatalog, String> {
    let path = catalog_path(obsidian_root);
    if !path.exists() {
        return Ok(ImageCatalog::default());
    }

    let content = fs::read_to_string(&path)
        .await
        .map_err(|error| format!("Failed reading image catalog {}: {}", path.display(), error))?;
    serde_json::from_str::<ImageCatalog>(&content).map_err(|error| {
        format!(
            "Failed parsing image catalog {}: {}",
            path.display(),
            error
        )
    })
}

pub(crate) async fn save_catalog(obsidian_root: &Path, catalog: &ImageCatalog) -> Result<(), String> {
    let path = catalog_path(obsidian_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(|error| {
            format!(
                "Failed creating image catalog directory {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(catalog)
        .map_err(|error| format!("Failed serializing image catalog: {}", error))?;
    let temp_path = path.with_extension(format!(
        "tmp-{}",
        Uuid::new_v4().to_string()
    ));
    fs::write(&temp_path, payload).await.map_err(|error| {
        format!(
            "Failed writing temporary image catalog {}: {}",
            temp_path.display(),
            error
        )
    })?;
    fs::rename(&temp_path, &path).await.map_err(|error| {
        format!(
            "Failed replacing image catalog {} with {}: {}",
            path.display(),
            temp_path.display(),
            error
        )
    })?;

    Ok(())
}

pub(crate) fn primary_asset_path(catalog: &ImageCatalog, entity_key: &str) -> Option<String> {
    let entity = catalog.entities.get(entity_key)?;
    if let Some(primary_id) = entity.primary_asset_id.as_ref() {
        if let Some(asset) = entity.assets.iter().find(|asset| &asset.id == primary_id) {
            return Some(asset.relative_path.clone());
        }
    }

    entity.assets.first().map(|asset| asset.relative_path.clone())
}

pub(crate) fn primary_asset_path_for_context(
    catalog: &ImageCatalog,
    entity_key: &str,
    context_text: Option<&str>,
) -> Option<String> {
    let entity = catalog.entities.get(entity_key)?;
    if let Some(context_text) = context_text {
        if let Some(asset_path) = context_override_asset_path(entity, context_text) {
            return Some(asset_path);
        }
    }
    primary_asset_path(catalog, entity_key)
}

pub(crate) fn upsert_entity_asset(
    catalog: &mut ImageCatalog,
    entity_key: &str,
    entity_type: &str,
    entity_name: &str,
    entity_slug: &str,
    mut asset: ImageAssetRecord,
    set_primary: bool,
) -> ImageAssetRecord {
    let entity = catalog
        .entities
        .entry(entity_key.to_string())
        .or_insert_with(|| ImageCatalogEntity {
            entity_type: entity_type.to_string(),
            entity_name: entity_name.to_string(),
            entity_slug: entity_slug.to_string(),
            primary_asset_id: None,
            assets: Vec::new(),
            context_overrides: Vec::new(),
        });

    entity.entity_type = entity_type.to_string();
    entity.entity_name = entity_name.to_string();
    entity.entity_slug = entity_slug.to_string();

    if let Some(existing) = entity
        .assets
        .iter_mut()
        .find(|existing| existing.sha256 == asset.sha256)
    {
        existing.source_path = asset.source_path.clone();
        existing.source_kind = asset.source_kind.clone();
        existing.tags = asset.tags.clone();
        asset = existing.clone();
    } else {
        entity.assets.push(asset.clone());
    }

    if set_primary || entity.primary_asset_id.is_none() {
        entity.primary_asset_id = Some(asset.id.clone());
    }

    catalog.updated_at = Utc::now().to_rfc3339();
    asset
}

pub(crate) fn upsert_context_override(
    catalog: &mut ImageCatalog,
    entity_key: &str,
    asset_id: &str,
    context_hint: &str,
    label: Option<String>,
) -> Option<()> {
    let entity = catalog.entities.get_mut(entity_key)?;
    let tokens = extract_context_tokens(context_hint);
    if tokens.is_empty() {
        return None;
    }

    if let Some(existing) = entity
        .context_overrides
        .iter_mut()
        .find(|override_item| override_item.match_tokens == tokens)
    {
        existing.asset_id = asset_id.to_string();
        existing.updated_at = Utc::now().to_rfc3339();
        if let Some(label) = label {
            existing.label = Some(label);
        }
        catalog.updated_at = Utc::now().to_rfc3339();
        return Some(());
    }

    entity.context_overrides.push(ImageContextOverride {
        id: format!("ctx_{}", Uuid::new_v4().simple()),
        asset_id: asset_id.to_string(),
        match_tokens: tokens,
        label,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    });
    catalog.updated_at = Utc::now().to_rfc3339();
    Some(())
}

fn context_override_asset_path(entity: &ImageCatalogEntity, context_text: &str) -> Option<String> {
    let context = context_text.to_ascii_lowercase();
    let mut best_score = 0usize;
    let mut best_asset_id: Option<&str> = None;

    for override_item in &entity.context_overrides {
        let score = override_item
            .match_tokens
            .iter()
            .filter(|token| context.contains(token.as_str()))
            .count();
        if score > best_score {
            best_score = score;
            best_asset_id = Some(override_item.asset_id.as_str());
        }
    }

    let asset_id = best_asset_id?;
    entity
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .map(|asset| asset.relative_path.clone())
}

pub(crate) fn extract_context_tokens(context_hint: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in context_hint
        .split(|ch: char| !ch.is_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
    {
        if raw.len() < 3 {
            continue;
        }
        if tokens.iter().any(|existing| existing == &raw) {
            continue;
        }
        tokens.push(raw);
        if tokens.len() >= 8 {
            break;
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::{
        primary_asset_path, primary_asset_path_for_context, upsert_context_override,
        upsert_entity_asset, ImageCatalog,
    };
    use crate::life_stream::types::ImageAssetRecord;

    fn sample_asset(id: &str, sha: &str, relative_path: &str) -> ImageAssetRecord {
        ImageAssetRecord {
            id: id.to_string(),
            relative_path: relative_path.to_string(),
            source_path: "/tmp/source.jpg".to_string(),
            source_kind: "manual_local".to_string(),
            sha256: sha.to_string(),
            mime: "image/jpeg".to_string(),
            created_at: "2026-02-06T23:00:00Z".to_string(),
            tags: vec!["cover".to_string()],
        }
    }

    #[test]
    fn upsert_entity_asset_dedupes_by_hash() {
        let mut catalog = ImageCatalog::default();
        let first = sample_asset("asset-1", "hash-1", "Assets/Entities/media/cowboy/first.jpg");
        let second = sample_asset("asset-2", "hash-1", "Assets/Entities/media/cowboy/second.jpg");

        upsert_entity_asset(
            &mut catalog,
            "media:cowboy",
            "media",
            "Cowboy",
            "cowboy",
            first,
            true,
        );
        let result = upsert_entity_asset(
            &mut catalog,
            "media:cowboy",
            "media",
            "Cowboy",
            "cowboy",
            second,
            true,
        );

        let entity = catalog.entities.get("media:cowboy").expect("entity");
        assert_eq!(entity.assets.len(), 1);
        assert_eq!(entity.primary_asset_id.as_deref(), Some(result.id.as_str()));
    }

    #[test]
    fn primary_asset_path_prefers_primary() {
        let mut catalog = ImageCatalog::default();
        let first = sample_asset("asset-1", "hash-1", "Assets/Entities/media/cowboy/one.jpg");
        let second = sample_asset("asset-2", "hash-2", "Assets/Entities/media/cowboy/two.jpg");

        upsert_entity_asset(
            &mut catalog,
            "media:cowboy",
            "media",
            "Cowboy",
            "cowboy",
            first,
            true,
        );
        upsert_entity_asset(
            &mut catalog,
            "media:cowboy",
            "media",
            "Cowboy",
            "cowboy",
            second,
            true,
        );

        let resolved = primary_asset_path(&catalog, "media:cowboy").expect("primary");
        assert!(resolved.ends_with("two.jpg"));
    }

    #[test]
    fn context_override_wins_for_matching_context() {
        let mut catalog = ImageCatalog::default();
        let first = sample_asset("asset-1", "hash-1", "Assets/Entities/media/cowboy/one.jpg");
        let second = sample_asset("asset-2", "hash-2", "Assets/Entities/media/cowboy/two.jpg");
        upsert_entity_asset(
            &mut catalog,
            "media:cowboy",
            "media",
            "Cowboy",
            "cowboy",
            first,
            true,
        );
        let latest = upsert_entity_asset(
            &mut catalog,
            "media:cowboy",
            "media",
            "Cowboy",
            "cowboy",
            second,
            true,
        );
        let _ = upsert_context_override(
            &mut catalog,
            "media:cowboy",
            &latest.id,
            "episode 5 ballad fallen angels",
            None,
        );

        let resolved = primary_asset_path_for_context(
            &catalog,
            "media:cowboy",
            Some("this is about episode 5"),
        )
        .expect("context primary");
        assert!(resolved.ends_with("two.jpg"));
    }
}
