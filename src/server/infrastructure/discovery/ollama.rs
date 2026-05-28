use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde::Deserialize;
use tracing::warn;

use crate::server::application::discovery::ModelDiscoveryPort;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::OllamaApi;
use crate::shared::contracts::DiscoveredModelDto;
use crate::shared::reference_data::{AiProviderKind, ModelCapability};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const PROVIDER: AiProviderKind = AiProviderKind::Ollama;

pub struct OllamaDiscoveryAdapter {
    api: Arc<OllamaApi>,
}

impl OllamaDiscoveryAdapter {
    pub fn new(api: Arc<OllamaApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Debug, Deserialize)]
struct TagEntry {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    details: Option<TagDetails>,
}

#[derive(Debug, Deserialize)]
struct TagDetails {
    #[serde(default)]
    parameter_size: Option<String>,
    #[serde(default)]
    quantization_level: Option<String>,
    #[serde(default)]
    family: Option<String>,
}

#[async_trait]
impl ModelDiscoveryPort for OllamaDiscoveryAdapter {
    async fn discover(
        &self,
        _capability: ModelCapability,
    ) -> Result<Vec<DiscoveredModelDto>, AppError> {
        let url = format!("{}/api/tags", self.api.base_url);
        let body = self
            .api
            .request_with_timeout(
                Method::GET,
                &url,
                Vec::new(),
                "application/json",
                "ollama tags",
                DISCOVERY_TIMEOUT,
            )
            .await?;
        parse_tags(&body)
    }
}

pub(crate) fn parse_tags(body: &str) -> Result<Vec<DiscoveredModelDto>, AppError> {
    let parsed: TagsResponse = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse ollama /api/tags: {e}")))?;

    let mut models = Vec::with_capacity(parsed.models.len());
    for entry in parsed.models {
        if !PROVIDER.model_id_well_formed(&entry.name) {
            warn!(
                provider = PROVIDER.as_str(),
                id = %entry.name,
                "discovery dropping model with malformed id"
            );
            continue;
        }
        let (parameter_size, quantization, family) = match entry.details {
            Some(d) => (d.parameter_size, d.quantization_level, d.family),
            None => (None, None, None),
        };
        let capability_hint = Some(heuristic_capability(&entry.name));
        models.push(DiscoveredModelDto {
            id: entry.name.clone(),
            display_name: Some(entry.name),
            description: None,
            dimensions: None,
            context_length: None,
            parameter_size,
            quantization,
            family,
            size_bytes: entry.size,
            capability_hint,
        });
    }
    Ok(models)
}

pub(crate) fn heuristic_capability(name: &str) -> ModelCapability {
    let lower = name.to_ascii_lowercase();
    if lower.contains("embed") {
        ModelCapability::Embedding
    } else if lower.contains("rerank") {
        ModelCapability::Reranker
    } else {
        ModelCapability::Generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAGS_FIXTURE: &str = r#"{
      "models": [
        {
          "name": "qwen2.5:7b",
          "size": 4683073280,
          "details": {
            "parameter_size": "7B",
            "quantization_level": "Q4_K_M",
            "family": "qwen2"
          }
        },
        {
          "name": "nomic-embed-text",
          "size": 274000000,
          "details": {
            "parameter_size": "0.1B",
            "quantization_level": "Q4_0",
            "family": "nomic-bert"
          }
        },
        {
          "name": "bge-reranker-v2-m3",
          "size": 1000000000,
          "details": null
        }
      ]
    }"#;

    #[test]
    fn parses_tags_fixture() {
        let models = parse_tags(TAGS_FIXTURE).unwrap();
        assert_eq!(models.len(), 3);

        let gen = &models[0];
        assert_eq!(gen.id, "qwen2.5:7b");
        assert_eq!(gen.parameter_size.as_deref(), Some("7B"));
        assert_eq!(gen.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(gen.family.as_deref(), Some("qwen2"));
        assert_eq!(gen.size_bytes, Some(4_683_073_280));
        assert_eq!(gen.capability_hint, Some(ModelCapability::Generation));
        assert!(gen.dimensions.is_none());

        assert_eq!(models[1].capability_hint, Some(ModelCapability::Embedding));
        assert_eq!(models[2].capability_hint, Some(ModelCapability::Reranker));
    }

    #[test]
    fn heuristic_table() {
        let cases = [
            ("nomic-embed-text", ModelCapability::Embedding),
            ("qwen3-embedding:0.6b", ModelCapability::Embedding),
            ("bge-reranker-v2-m3", ModelCapability::Reranker),
            ("qwen2.5:7b", ModelCapability::Generation),
            ("llama3.2:3b", ModelCapability::Generation),
        ];
        for (name, expected) in cases {
            assert_eq!(
                heuristic_capability(name),
                expected,
                "heuristic mismatch for {name}"
            );
        }
    }

    #[test]
    fn drops_models_with_whitespace_in_id() {
        let body = r#"{
          "models": [
            { "name": "good-model:latest", "size": 1, "details": null },
            { "name": "bad model", "size": 1, "details": null }
          ]
        }"#;
        let models = parse_tags(body).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "good-model:latest");
    }

    #[test]
    fn surfaces_parse_failure_as_upstream() {
        let err = parse_tags("not json").unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }

    #[test]
    fn handles_empty_models_array() {
        let models = parse_tags(r#"{ "models": [] }"#).unwrap();
        assert!(models.is_empty());
    }
}
