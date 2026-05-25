use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::embedding::ports::Embedder;
use crate::server::application::embedding::EmbedderRegistry;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::LlamaServerApi;
use crate::shared::reference_data::AiProviderKind;

pub struct LlamaServerEmbedder {
    api: Arc<LlamaServerApi>,
}

impl LlamaServerEmbedder {
    pub fn new(api: Arc<LlamaServerApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

pub fn register(registry: &mut EmbedderRegistry, api: Arc<LlamaServerApi>) {
    registry.register(AiProviderKind::LlamaServer, LlamaServerEmbedder::new(api));
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedDatum>,
}

#[derive(Debug, Deserialize)]
struct EmbedDatum {
    embedding: Vec<f32>,
}

fn parse_embed_response(
    body: &str,
    texts: &[String],
    dimensions: u32,
) -> Result<Vec<Vec<f32>>, AppError> {
    let data: EmbedResponse = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse llama-server embedding: {e}")))?;
    if data.data.len() != texts.len() {
        return Err(AppError::Upstream(format!(
            "llama-server returned {} embeddings for {} inputs",
            data.data.len(),
            texts.len()
        )));
    }
    let embeddings: Vec<Vec<f32>> = data.data.into_iter().map(|d| d.embedding).collect();
    let expected = dimensions as usize;
    if let Some(actual) = embeddings.first().map(Vec::len) {
        if expected != 0 && actual != expected {
            return Err(AppError::Upstream(format!(
                "llama-server embedding dimension mismatch: expected {expected}, got {actual}"
            )));
        }
    }
    Ok(embeddings)
}

#[async_trait]
impl Embedder for LlamaServerEmbedder {
    async fn embed_batch(
        &self,
        model: &str,
        dimensions: u32,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let base_url = self.api.base_url.trim().trim_end_matches('/');
        let url = format!("{base_url}/v1/embeddings");
        let request = EmbedRequest {
            model: model.to_string(),
            input: texts.to_vec(),
        };
        let body_bytes = serde_json::to_vec(&request)
            .map_err(|e| AppError::Internal(format!("encode embed body: {e}")))?;
        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "llama-server embedding",
            )
            .await?;
        parse_embed_response(&resp, texts, dimensions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn encodes_embed_request_with_model_and_inputs() {
        let req = EmbedRequest {
            model: "qwen3-embedding-0.6b".into(),
            input: vec!["one".into(), "two".into()],
        };
        let json: Value = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(json["model"], "qwen3-embedding-0.6b");
        assert_eq!(json["input"][0], "one");
        assert_eq!(json["input"][1], "two");
    }

    #[test]
    fn parses_openai_embedding_response() {
        let body = r#"{
            "data": [
                { "index": 0, "embedding": [0.1, 0.2, 0.3] },
                { "index": 1, "embedding": [0.4, 0.5, 0.6] }
            ]
        }"#;
        let texts = vec!["a".into(), "b".into()];
        let parsed = parse_embed_response(body, &texts, 3).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], vec![0.1, 0.2, 0.3]);
        assert_eq!(parsed[1], vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn rejects_count_mismatch() {
        let body = r#"{ "data": [ { "embedding": [0.0, 0.0] } ] }"#;
        let texts = vec!["a".into(), "b".into()];
        let err = parse_embed_response(body, &texts, 2).unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let body = r#"{ "data": [ { "embedding": [0.1, 0.2] } ] }"#;
        let texts = vec!["a".into()];
        let err = parse_embed_response(body, &texts, 4).unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }

    #[test]
    fn accepts_any_dimension_when_expected_zero() {
        let body = r#"{ "data": [ { "embedding": [0.1, 0.2, 0.3, 0.4, 0.5] } ] }"#;
        let texts = vec!["a".into()];
        let parsed = parse_embed_response(body, &texts, 0).unwrap();
        assert_eq!(parsed[0].len(), 5);
    }
}
