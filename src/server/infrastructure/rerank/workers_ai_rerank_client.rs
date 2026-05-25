use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::server::application::rerank::ports::{Reranker, RerankRequest, RerankScore};
use crate::server::application::rerank::RerankerRegistry;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::{CloudflareApi, CLOUDFLARE_API_BASE};
use crate::shared::reference_data::AiProviderKind;

pub struct WorkersAiRerankClient {
    api: Arc<CloudflareApi>,
}

impl WorkersAiRerankClient {
    pub fn new(api: Arc<CloudflareApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

pub fn register(registry: &mut RerankerRegistry, api: Arc<CloudflareApi>) {
    registry.register(AiProviderKind::Cloudflare, WorkersAiRerankClient::new(api));
}

#[derive(Serialize)]
struct RerankBody {
    query: String,
    contexts: Vec<RerankContext>,
}

#[derive(Serialize)]
struct RerankContext {
    text: String,
}

#[derive(Deserialize)]
struct AiEnvelope {
    success: bool,
    #[serde(default)]
    errors: Option<Value>,
    #[serde(default)]
    result: Option<AiResult>,
}

#[derive(Deserialize)]
struct AiResult {
    #[serde(default)]
    response: Vec<RerankItem>,
}

#[derive(Deserialize)]
struct RerankItem {
    id: usize,
    score: f32,
}

#[async_trait]
impl Reranker for WorkersAiRerankClient {
    async fn rerank(&self, request: RerankRequest) -> Result<Vec<RerankScore>, AppError> {
        if request.documents.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!(
            "{}/accounts/{}/ai/run/{}",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
            request.model
        );
        let body = RerankBody {
            query: request.query,
            contexts: request
                .documents
                .into_iter()
                .map(|text| RerankContext { text })
                .collect(),
        };
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode workers-ai rerank body: {e}")))?;
        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "workers-ai rerank",
            )
            .await?;
        parse_rerank_response(&resp)
    }
}

fn parse_rerank_response(body: &str) -> Result<Vec<RerankScore>, AppError> {
    let envelope: AiEnvelope = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse workers-ai rerank: {e}")))?;
    if !envelope.success {
        return Err(AppError::Upstream(format!(
            "workers-ai rerank failed: {}",
            envelope
                .errors
                .map(|e| e.to_string())
                .unwrap_or_else(|| "<no error body>".into())
        )));
    }
    let response = envelope
        .result
        .map(|r| r.response)
        .unwrap_or_default();
    Ok(response
        .into_iter()
        .map(|item| RerankScore {
            index: item.id,
            score: item.score,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_envelope_with_scored_response() {
        let body = r#"{
            "success": true,
            "result": {
                "response": [
                    { "id": 2, "score": 0.91 },
                    { "id": 0, "score": 0.43 },
                    { "id": 1, "score": 0.12 }
                ]
            }
        }"#;
        let scores = parse_rerank_response(body).unwrap();
        assert_eq!(scores.len(), 3);
        assert_eq!(scores[0].index, 2);
        assert!((scores[0].score - 0.91).abs() < 1e-5);
    }

    #[test]
    fn surfaces_upstream_failure() {
        let body = r#"{ "success": false, "errors": ["nope"] }"#;
        let err = parse_rerank_response(body).unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }

    #[test]
    fn returns_empty_when_response_array_missing() {
        let body = r#"{ "success": true, "result": {} }"#;
        let scores = parse_rerank_response(body).unwrap();
        assert!(scores.is_empty());
    }
}
