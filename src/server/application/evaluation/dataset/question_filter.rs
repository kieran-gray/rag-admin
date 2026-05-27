use std::slice;

use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::AppError;
use crate::shared::contracts::EvaluationQuestionDto;
use crate::shared::ordered_f32_vec;
use crate::shared::OrderedF32;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuestionFilterStats {
    pub duplicate: u32,
}

pub enum QuestionFilterDecision {
    Accepted { embedding: Vec<f32> },
    RejectedDuplicate { similarity: f32 },
}

pub struct GeneratedQuestionGate<'a> {
    embedding_service: &'a EmbeddingService,
    embedding_model: &'a ResolvedEmbeddingModel,
    duplicate_similarity_threshold: f32,
    question_embeddings: Vec<Vec<f32>>,
}

impl<'a> GeneratedQuestionGate<'a> {
    pub fn with_existing(
        embedding_service: &'a EmbeddingService,
        embedding_model: &'a ResolvedEmbeddingModel,
        duplicate_similarity_threshold: f32,
        existing: Vec<EvaluationQuestionDto>,
    ) -> Self {
        let question_embeddings: Vec<Vec<f32>> = existing
            .into_iter()
            .filter_map(|q| q.embedding.map(|e| e.into_iter().map(f32::from).collect()))
            .collect();
        Self {
            embedding_service,
            embedding_model,
            duplicate_similarity_threshold,
            question_embeddings,
        }
    }

    pub async fn try_accept(&mut self, question: &str) -> Result<QuestionFilterDecision, AppError> {
        let owned = question.to_string();
        let mut embeddings = self
            .embedding_service
            .embed_with_resolved(self.embedding_model, slice::from_ref(&owned))
            .await?;
        let embedding = embeddings
            .pop()
            .ok_or_else(|| AppError::Internal("embedding service returned empty result".into()))?;

        let mut max_sim = 0.0f32;
        for existing in &self.question_embeddings {
            let sim = cosine_similarity(&embedding, existing);
            if sim > max_sim {
                max_sim = sim;
            }
        }
        if max_sim >= self.duplicate_similarity_threshold {
            return Ok(QuestionFilterDecision::RejectedDuplicate {
                similarity: max_sim,
            });
        }

        self.question_embeddings.push(embedding.clone());
        Ok(QuestionFilterDecision::Accepted { embedding })
    }
}

pub fn embedding_to_ordered(e: Vec<f32>) -> Vec<OrderedF32> {
    ordered_f32_vec(e)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (av, bv) in a.iter().zip(b.iter()).take(n) {
        dot += av * bv;
        na += av * av;
        nb += bv * bv;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom <= 1e-9 {
        0.0
    } else {
        dot / denom
    }
}
