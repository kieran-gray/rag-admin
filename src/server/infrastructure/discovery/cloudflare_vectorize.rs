use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::server::application::discovery::VectorIndexDiscoveryPort;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::{CloudflareApi, CLOUDFLARE_API_BASE};
use crate::shared::contracts::DiscoveredIndexDto;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct CloudflareVectorizeDiscoveryAdapter {
    api: Arc<CloudflareApi>,
}

impl CloudflareVectorizeDiscoveryAdapter {
    pub fn new(api: Arc<CloudflareApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

#[derive(Debug, Deserialize)]
struct IndexesEnvelope {
    success: bool,
    #[serde(default)]
    errors: Option<Value>,
    #[serde(default)]
    result: Option<Vec<IndexEntry>>,
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    config: Option<IndexConfig>,
}

#[derive(Debug, Deserialize)]
struct IndexConfig {
    #[serde(default)]
    dimensions: Option<u32>,
    #[serde(default)]
    metric: Option<String>,
}

#[async_trait]
impl VectorIndexDiscoveryPort for CloudflareVectorizeDiscoveryAdapter {
    async fn discover(&self) -> Result<Vec<DiscoveredIndexDto>, AppError> {
        let url = format!(
            "{}/accounts/{}/vectorize/v2/indexes",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
        );
        let body = self
            .api
            .request_with_timeout(
                Method::GET,
                &url,
                Vec::new(),
                "application/json",
                "cf vectorize indexes",
                DISCOVERY_TIMEOUT,
            )
            .await?;
        parse_indexes(&body)
    }
}

pub(crate) fn parse_indexes(body: &str) -> Result<Vec<DiscoveredIndexDto>, AppError> {
    let envelope: IndexesEnvelope = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse cf vectorize indexes: {e}")))?;
    if !envelope.success {
        let detail = envelope
            .errors
            .map(|e| e.to_string())
            .unwrap_or_else(|| "<no error body>".into());
        return Err(AppError::Upstream(format!(
            "cf vectorize success=false: {detail}"
        )));
    }
    let entries = envelope.result.unwrap_or_default();
    let mut indexes = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(dimensions) = entry.config.as_ref().and_then(|c| c.dimensions) else {
            warn!(name = %entry.name, "discovery dropping vectorize index without dimensions");
            continue;
        };
        indexes.push(DiscoveredIndexDto {
            name: entry.name,
            dimensions,
            metric: entry.config.and_then(|c| c.metric),
            description: entry.description,
        });
    }
    Ok(indexes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "success": true,
      "errors": [],
      "result": [
        {
          "name": "prod-docs-1024",
          "description": "Production corpus.",
          "config": { "dimensions": 1024, "metric": "cosine" }
        },
        {
          "name": "legacy-768",
          "config": { "dimensions": 768, "metric": "euclidean" }
        }
      ]
    }"#;

    #[test]
    fn parses_indexes_fixture() {
        let indexes = parse_indexes(FIXTURE).unwrap();
        assert_eq!(indexes.len(), 2);

        let first = &indexes[0];
        assert_eq!(first.name, "prod-docs-1024");
        assert_eq!(first.dimensions, 1024);
        assert_eq!(first.metric.as_deref(), Some("cosine"));
        assert_eq!(first.description.as_deref(), Some("Production corpus."));

        assert_eq!(indexes[1].name, "legacy-768");
        assert_eq!(indexes[1].dimensions, 768);
        assert_eq!(indexes[1].description, None);
    }

    #[test]
    fn drops_indexes_without_dimensions() {
        let body = r#"{
          "success": true,
          "result": [
            { "name": "no-config" },
            { "name": "ok", "config": { "dimensions": 256 } }
          ]
        }"#;
        let indexes = parse_indexes(body).unwrap();
        assert_eq!(indexes.len(), 1);
        assert_eq!(indexes[0].name, "ok");
    }

    #[test]
    fn surfaces_success_false_as_upstream() {
        let body = r#"{ "success": false, "errors": ["nope"] }"#;
        let err = parse_indexes(body).unwrap_err();
        match err {
            AppError::Upstream(message) => assert!(message.contains("success=false")),
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    #[test]
    fn returns_empty_when_result_missing() {
        let body = r#"{ "success": true }"#;
        let indexes = parse_indexes(body).unwrap();
        assert!(indexes.is_empty());
    }
}
