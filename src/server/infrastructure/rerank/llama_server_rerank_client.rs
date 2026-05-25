use std::cmp::Ordering;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::rerank::ports::{RerankRequest, RerankScore, Reranker};
use crate::server::application::rerank::RerankerRegistry;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::LlamaServerApi;
use crate::shared::reference_data::AiProviderKind;

pub struct LlamaServerRerankClient {
    api: Arc<LlamaServerApi>,
}

impl LlamaServerRerankClient {
    pub fn new(api: Arc<LlamaServerApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

pub fn register(registry: &mut RerankerRegistry, api: Arc<LlamaServerApi>) {
    registry.register(
        AiProviderKind::LlamaServer,
        LlamaServerRerankClient::new(api),
    );
}

#[derive(Serialize)]
struct RerankBody {
    model: String,
    query: String,
    documents: Vec<String>,
}

#[derive(Deserialize)]
struct RerankResponse {
    #[serde(default)]
    results: Vec<RerankResult>,
}

#[derive(Deserialize)]
struct RerankResult {
    index: usize,
    relevance_score: f32,
}

#[async_trait]
impl Reranker for LlamaServerRerankClient {
    async fn rerank(&self, request: RerankRequest) -> Result<Vec<RerankScore>, AppError> {
        if request.documents.is_empty() {
            return Ok(Vec::new());
        }

        let base_url = self.api.base_url.trim().trim_end_matches('/');
        let url = format!("{base_url}/v1/rerank");
        let body = RerankBody {
            model: request.model,
            query: request.query,
            documents: request.documents,
        };
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode llama-server rerank body: {e}")))?;
        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "llama-server rerank",
            )
            .await?;
        parse_rerank_response(&resp)
    }
}

fn parse_rerank_response(body: &str) -> Result<Vec<RerankScore>, AppError> {
    let parsed: RerankResponse = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse llama-server rerank: {e}")))?;
    let mut scores: Vec<RerankScore> = parsed
        .results
        .into_iter()
        .map(|r| RerankScore {
            index: r.index,
            score: sigmoid(r.relevance_score),
        })
        .collect();
    scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    Ok(scores)
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_results_and_sorts_descending() {
        let body = r#"{
            "model": "qwen3-reranker-4b",
            "results": [
                { "index": 0, "relevance_score": -2.0 },
                { "index": 1, "relevance_score": 3.0 },
                { "index": 2, "relevance_score": 0.5 }
            ]
        }"#;
        let scores = parse_rerank_response(body).unwrap();
        assert_eq!(scores.len(), 3);
        assert_eq!(scores[0].index, 1);
        assert_eq!(scores[1].index, 2);
        assert_eq!(scores[2].index, 0);
        assert!(scores[0].score > scores[1].score);
        assert!(scores[1].score > scores[2].score);
    }

    #[test]
    fn sigmoid_maps_to_unit_interval() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.999);
        assert!(sigmoid(-10.0) < 0.001);
    }

    #[test]
    fn empty_results_returns_empty_vec() {
        let body = r#"{ "results": [] }"#;
        let scores = parse_rerank_response(body).unwrap();
        assert!(scores.is_empty());
    }

    #[test]
    fn missing_results_field_returns_empty_vec() {
        let body = r#"{ "model": "x" }"#;
        let scores = parse_rerank_response(body).unwrap();
        assert!(scores.is_empty());
    }

    #[test]
    fn surfaces_parse_error_on_malformed_body() {
        let err = parse_rerank_response("not json").unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }
}
