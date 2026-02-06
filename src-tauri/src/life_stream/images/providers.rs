use super::fetchers::TmdbFetcher;

#[derive(Debug, Clone)]
pub struct RemoteImageCandidate {
    pub source_kind: String,
    pub file_name_hint: String,
    pub bytes: Vec<u8>,
    pub reasons: Vec<String>,
    pub score_boost: i64,
}

#[derive(Debug, Clone)]
pub struct CandidateProviderConfig {
    pub tmdb_api_key: Option<String>,
}

pub struct CandidateProviderRegistry {
    tmdb: TmdbFetcher,
}

impl CandidateProviderRegistry {
    pub fn new(config: CandidateProviderConfig) -> Self {
        Self {
            tmdb: TmdbFetcher::new(config.tmdb_api_key),
        }
    }

    pub async fn fetch_candidates(
        &self,
        entity_type: &str,
        entity_name: &str,
        context_hint: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RemoteImageCandidate>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut output = Vec::new();
        output.extend(
            self.fetch_tmdb_media(entity_type, entity_name, context_hint, limit)
                .await?,
        );

        // Stub adapter for future non-TMDB internet sources.
        // We keep it explicit so IMG-4.1 can slot in providers without touching ranking logic.
        output.extend(
            self.fetch_web_stub(entity_type, entity_name, context_hint, limit.saturating_sub(output.len()))
                .await,
        );

        if output.len() > limit {
            output.truncate(limit);
        }
        Ok(output)
    }

    async fn fetch_tmdb_media(
        &self,
        entity_type: &str,
        entity_name: &str,
        context_hint: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RemoteImageCandidate>, String> {
        if entity_type != "media" || limit == 0 {
            return Ok(Vec::new());
        }

        let Some(bytes) = self
            .tmdb
            .fetch(entity_name)
            .await
            .map_err(|error| format!("TMDB fetch failed for {entity_name}: {error}"))?
        else {
            return Ok(Vec::new());
        };

        let mut reasons = vec!["provider:tmdb".to_string(), "auto-fetch".to_string()];
        if let Some(context) = context_hint {
            if !context.trim().is_empty() {
                reasons.push("context-aware".to_string());
            }
        }

        Ok(vec![RemoteImageCandidate {
            source_kind: "provider_tmdb".to_string(),
            file_name_hint: format!("tmdb-{}.jpg", sanitize_filename(entity_name)),
            bytes,
            reasons,
            score_boost: 24,
        }])
    }

    async fn fetch_web_stub(
        &self,
        entity_type: &str,
        entity_name: &str,
        _context_hint: Option<&str>,
        _limit: usize,
    ) -> Vec<RemoteImageCandidate> {
        let _ = (entity_type, entity_name);
        Vec::new()
    }
}

fn sanitize_filename(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
