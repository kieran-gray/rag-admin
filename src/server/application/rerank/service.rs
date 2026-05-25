use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::indexing::ports::vector_index::VectorMatch;
use crate::server::application::rerank::ports::{RerankRequest, RerankScore};
use crate::server::application::rerank::RerankerRegistry;
use crate::server::application::AppError;
use crate::server::domain::configuration::reranker_model::RerankerModelRepository;
use crate::shared::reference_data::AiProviderKind;

pub const RERANK_CANDIDATE_MULTIPLIER: u32 = 4;
pub const RERANK_MAX_CANDIDATES: u32 = 200;

#[derive(Debug, Clone)]
pub struct ResolvedRerankerModel {
    pub reranker_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

pub struct RerankService {
    rerankers: Arc<RerankerRegistry>,
    reranker_models: Arc<dyn RerankerModelRepository>,
}

impl RerankService {
    pub fn new(
        rerankers: Arc<RerankerRegistry>,
        reranker_models: Arc<dyn RerankerModelRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            rerankers,
            reranker_models,
        })
    }

    pub async fn rerank(
        &self,
        reranker_model_id: Uuid,
        query: String,
        documents: Vec<String>,
    ) -> Result<Vec<RerankScore>, AppError> {
        let resolved = self.resolve(reranker_model_id).await?;
        let reranker = self.rerankers.get(&resolved.kind).ok_or_else(|| {
            AppError::Internal(format!(
                "no reranker registered for provider kind {}",
                resolved.kind.as_str()
            ))
        })?;
        reranker
            .rerank(RerankRequest {
                model: resolved.model,
                query,
                documents,
            })
            .await
    }

    pub async fn rerank_matches(
        &self,
        reranker_model_id: Uuid,
        query: &str,
        matches: Vec<VectorMatch>,
    ) -> Result<Vec<VectorMatch>, AppError> {
        if matches.is_empty() {
            return Ok(matches);
        }
        let documents: Vec<String> = matches
            .iter()
            .map(|m| {
                m.metadata
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        let scores = self
            .rerank(reranker_model_id, query.to_string(), documents)
            .await?;
        Ok(reorder_matches_by_scores(matches, scores))
    }

    pub async fn resolve(
        &self,
        reranker_model_id: Uuid,
    ) -> Result<ResolvedRerankerModel, AppError> {
        let model = self
            .reranker_models
            .find_by_id(reranker_model_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("reranker model {reranker_model_id} not registered"))
            })?;
        Ok(ResolvedRerankerModel {
            reranker_model_id: model.reranker_model_id,
            kind: model.kind,
            model: model.model,
        })
    }
}

fn reorder_matches_by_scores(
    mut matches: Vec<VectorMatch>,
    scores: Vec<RerankScore>,
) -> Vec<VectorMatch> {
    let mut score_by_index: Vec<f32> = vec![0.0; matches.len()];
    for s in &scores {
        if s.index < score_by_index.len() {
            score_by_index[s.index] = s.score;
        }
    }
    for (i, m) in matches.iter_mut().enumerate() {
        m.score = score_by_index[i];
    }
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches
}

pub fn rerank_fetch_k(top_k: u32) -> u32 {
    top_k
        .saturating_mul(RERANK_CANDIDATE_MULTIPLIER)
        .clamp(1, RERANK_MAX_CANDIDATES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vm(id: &str, score: f32, text: &str) -> VectorMatch {
        VectorMatch {
            id: id.into(),
            score,
            metadata: json!({"text": text}),
        }
    }

    #[test]
    fn reorder_replaces_score_and_sorts_descending() {
        let matches = vec![
            vm("a", 0.9, "alpha"),
            vm("b", 0.8, "beta"),
            vm("c", 0.7, "gamma"),
        ];
        let scores = vec![
            RerankScore { index: 0, score: 0.1 },
            RerankScore { index: 1, score: 0.95 },
            RerankScore { index: 2, score: 0.55 },
        ];

        let reordered = reorder_matches_by_scores(matches, scores);

        assert_eq!(reordered[0].id, "b");
        assert!((reordered[0].score - 0.95).abs() < 1e-5);
        assert_eq!(reordered[1].id, "c");
        assert_eq!(reordered[2].id, "a");
    }

    #[test]
    fn fetch_k_multiplies_and_clamps() {
        assert_eq!(rerank_fetch_k(0), 1);
        assert_eq!(rerank_fetch_k(5), 20);
        assert_eq!(rerank_fetch_k(100), 200);
    }
}
