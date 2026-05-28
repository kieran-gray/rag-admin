use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::server::application::discovery::ModelDiscoveryPort;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::{CloudflareApi, CLOUDFLARE_API_BASE};
use crate::shared::contracts::DiscoveredModelDto;
use crate::shared::reference_data::{AiProviderKind, ModelCapability};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const PROVIDER: AiProviderKind = AiProviderKind::Cloudflare;

pub struct CloudflareDiscoveryAdapter {
    api: Arc<CloudflareApi>,
}

impl CloudflareDiscoveryAdapter {
    pub fn new(api: Arc<CloudflareApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

#[derive(Debug, Deserialize)]
struct ModelsEnvelope {
    success: bool,
    #[serde(default)]
    errors: Option<Value>,
    #[serde(default)]
    result: Option<Vec<ModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    properties: Option<Vec<ModelProperty>>,
}

#[derive(Debug, Deserialize)]
struct ModelProperty {
    property_id: String,
    value: Value,
}

#[async_trait]
impl ModelDiscoveryPort for CloudflareDiscoveryAdapter {
    async fn discover(
        &self,
        capability: ModelCapability,
    ) -> Result<Vec<DiscoveredModelDto>, AppError> {
        let task = cloudflare_task(capability);
        let url = format!(
            "{}/accounts/{}/ai/models/search?task={}",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
            urlencoding::encode(task),
        );
        let body = self
            .api
            .request_with_timeout(
                Method::GET,
                &url,
                Vec::new(),
                "application/json",
                "cf models",
                DISCOVERY_TIMEOUT,
            )
            .await?;
        parse_models(&body, capability)
    }
}

pub(crate) fn parse_models(
    body: &str,
    capability: ModelCapability,
) -> Result<Vec<DiscoveredModelDto>, AppError> {
    let envelope: ModelsEnvelope = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse cf models: {e}")))?;
    if !envelope.success {
        let detail = envelope
            .errors
            .map(|e| e.to_string())
            .unwrap_or_else(|| "<no error body>".into());
        return Err(AppError::Upstream(format!(
            "cf models success=false: {detail}"
        )));
    }
    let entries = envelope.result.unwrap_or_default();
    let mut models = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(model_id) = entry.name else {
            warn!(
                provider = PROVIDER.as_str(),
                "discovery dropping model with missing name"
            );
            continue;
        };
        if !PROVIDER.model_id_well_formed(&model_id) {
            warn!(
                provider = PROVIDER.as_str(),
                id = %model_id,
                "discovery dropping model with malformed id"
            );
            continue;
        }

        let (dimensions, context_length) = extract_properties(&entry.properties);
        models.push(DiscoveredModelDto {
            id: model_id.clone(),
            display_name: Some(model_id),
            description: entry.description,
            dimensions,
            context_length,
            parameter_size: None,
            quantization: None,
            family: None,
            size_bytes: None,
            capability_hint: Some(capability),
        });
    }
    Ok(models)
}

pub(crate) fn cloudflare_task(capability: ModelCapability) -> &'static str {
    match capability {
        ModelCapability::Embedding => "Text Embeddings",
        ModelCapability::Generation => "Text Generation",
        ModelCapability::Reranker => "Text Classification",
    }
}

fn extract_properties(props: &Option<Vec<ModelProperty>>) -> (Option<u32>, Option<u32>) {
    let mut dimensions = None;
    let mut context_length = None;
    let Some(list) = props else {
        return (dimensions, context_length);
    };
    for prop in list {
        match prop.property_id.as_str() {
            "output_dimensions" => dimensions = parse_u32(&prop.value),
            "context_window" => context_length = parse_u32(&prop.value),
            _ => {}
        }
    }
    (dimensions, context_length)
}

fn parse_u32(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|u| u32::try_from(u).ok()),
        Value::String(s) => s.parse::<u32>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMBEDDING_FIXTURE: &str = r#"{
      "success": true,
      "errors": [],
      "result": [
        {
          "id": "429b9e8b-d99e-44de-91ad-706cf8183658",
          "name": "@cf/baai/bge-base-en-v1.5",
          "description": "Base BGE embedding model.",
          "task": { "id": "task-emb", "name": "Text Embeddings" },
          "properties": [
            { "property_id": "output_dimensions", "value": "768" },
            { "property_id": "context_window", "value": 512 },
            { "property_id": "max_input_tokens", "value": "512" }
          ]
        },
        {
          "id": "c1a7e9d6-3b25-4d2f-8e1c-2c9f0c5a7f04",
          "name": "@cf/baai/bge-m3",
          "description": "Multilingual BGE-M3.",
          "task": { "id": "task-emb", "name": "Text Embeddings" },
          "properties": [
            { "property_id": "output_dimensions", "value": 1024 },
            { "property_id": "context_window", "value": 8192 }
          ]
        }
      ]
    }"#;

    const GENERATION_FIXTURE: &str = r#"{
      "success": true,
      "result": [
        {
          "id": "9d3b81f3-4a17-4c6d-bcb6-9c8f1d3f7c44",
          "name": "@cf/meta/llama-3.1-8b-instruct",
          "description": "Meta Llama 3.1 8B Instruct.",
          "properties": [
            { "property_id": "context_window", "value": 131072 }
          ]
        }
      ]
    }"#;

    #[test]
    fn parses_embedding_fixture() {
        let models = parse_models(EMBEDDING_FIXTURE, ModelCapability::Embedding).unwrap();
        assert_eq!(models.len(), 2);

        let first = &models[0];
        assert_eq!(first.id, "@cf/baai/bge-base-en-v1.5");
        assert_eq!(first.dimensions, Some(768));
        assert_eq!(first.context_length, Some(512));
        assert_eq!(
            first.display_name.as_deref(),
            Some("@cf/baai/bge-base-en-v1.5")
        );
        assert_eq!(first.capability_hint, Some(ModelCapability::Embedding));

        assert_eq!(models[1].dimensions, Some(1024));
        assert_eq!(models[1].context_length, Some(8192));
    }

    #[test]
    fn parses_generation_fixture() {
        let models = parse_models(GENERATION_FIXTURE, ModelCapability::Generation).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].dimensions, None);
        assert_eq!(models[0].context_length, Some(131_072));
        assert_eq!(models[0].capability_hint, Some(ModelCapability::Generation));
    }

    #[test]
    fn task_mapping() {
        assert_eq!(
            cloudflare_task(ModelCapability::Embedding),
            "Text Embeddings"
        );
        assert_eq!(
            cloudflare_task(ModelCapability::Generation),
            "Text Generation"
        );
        assert_eq!(
            cloudflare_task(ModelCapability::Reranker),
            "Text Classification"
        );
    }

    #[test]
    fn surfaces_success_false_as_upstream() {
        let body = r#"{ "success": false, "errors": ["nope"] }"#;
        let err = parse_models(body, ModelCapability::Embedding).unwrap_err();
        match err {
            AppError::Upstream(message) => assert!(message.contains("success=false")),
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    #[test]
    fn drops_models_without_at_cf_prefix() {
        let body = r#"{
          "success": true,
          "result": [
            { "name": "@cf/good/model", "properties": [] },
            { "name": "missing-prefix", "properties": [] }
          ]
        }"#;
        let models = parse_models(body, ModelCapability::Embedding).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "@cf/good/model");
    }

    #[test]
    fn drops_models_with_missing_name() {
        let body = r#"{
          "success": true,
          "result": [
            { "name": "@cf/with/name", "properties": [] },
            { "properties": [] }
          ]
        }"#;
        let models = parse_models(body, ModelCapability::Embedding).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "@cf/with/name");
    }

    #[test]
    fn returns_empty_when_result_missing() {
        let body = r#"{ "success": true }"#;
        let models = parse_models(body, ModelCapability::Embedding).unwrap();
        assert!(models.is_empty());
    }

    #[test]
    fn dimensions_accept_number_or_string() {
        let body = r#"{
          "success": true,
          "result": [
            {
              "name": "@cf/a",
              "properties": [{ "property_id": "output_dimensions", "value": 256 }]
            },
            {
              "name": "@cf/b",
              "properties": [{ "property_id": "output_dimensions", "value": "512" }]
            }
          ]
        }"#;
        let models = parse_models(body, ModelCapability::Embedding).unwrap();
        assert_eq!(models[0].dimensions, Some(256));
        assert_eq!(models[1].dimensions, Some(512));
    }
}
