use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::evaluation::ports::{RetrievalQuery, RetrievedChunk, Retriever};
use crate::server::application::AppError;
use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::evaluation::question::CognitiveOperation;
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::scoring::{
    bootstrap_ci, mean, precision_omega, score_question, score_trick_question, std_dev,
};
use crate::shared::{EvaluationMetrics, EvaluationRunOptions, EvaluationScorePolicy};

use super::run_context::{PreparedVariant, QuestionSubset};

pub struct VariantRetrievalScorer {
    retriever: Arc<dyn Retriever>,
}

impl VariantRetrievalScorer {
    pub fn new(retriever: Arc<dyn Retriever>) -> Arc<Self> {
        Arc::new(Self { retriever })
    }

    pub async fn retrieve_passage(
        &self,
        variant: &PreparedVariant,
        query_vector: &[f32],
        options: &EvaluationRunOptions,
    ) -> Result<Vec<(Uuid, f32, String)>, AppError> {
        let retrieved: Vec<RetrievedChunk> = self
            .retriever
            .retrieve(&RetrievalQuery {
                embedding_set_id: variant.embedding_set_id,
                query_vector: query_vector.to_vec(),
                top_k: options.top_k,
                min_score: options.min_score(),
            })
            .await?;
        let chunk_by_id: HashMap<Uuid, &Chunk> =
            variant.chunks.iter().map(|c| (c.chunk_id, c)).collect();
        Ok(retrieved
            .into_iter()
            .filter_map(|r| {
                chunk_by_id
                    .get(&r.chunk_id)
                    .map(|c| (r.chunk_id, r.score, c.text.clone()))
            })
            .collect())
    }

    pub async fn score(
        &self,
        run_id: Uuid,
        variant: &PreparedVariant,
        subset: &QuestionSubset<'_>,
        options: &EvaluationRunOptions,
    ) -> Result<(EvaluationMetrics, Vec<RetrievalTraceEntry>), AppError> {
        let chunk_by_id: HashMap<Uuid, &Chunk> =
            variant.chunks.iter().map(|c| (c.chunk_id, c)).collect();

        let n = subset.questions.len();
        let mut recall_scores = Vec::with_capacity(n);
        let mut precision_scores = Vec::with_capacity(n);
        let mut iou_scores = Vec::with_capacity(n);
        let mut omega_scores = Vec::with_capacity(n);
        let mut traces = Vec::with_capacity(n);

        for (question, q_emb) in subset.questions.iter().zip(subset.embeddings.iter()) {
            let question = *question;
            let retrieved = self
                .retriever
                .retrieve(&RetrievalQuery {
                    embedding_set_id: variant.embedding_set_id,
                    query_vector: q_emb.clone(),
                    top_k: options.top_k,
                    min_score: options.min_score(),
                })
                .await?;

            let mut retrieved_refs = Vec::with_capacity(retrieved.len());
            let mut retrieved_chunk_ids = Vec::with_capacity(retrieved.len());
            let mut scores = Vec::with_capacity(retrieved.len());
            for r in &retrieved {
                if let Some(&chunk) = chunk_by_id.get(&r.chunk_id) {
                    retrieved_refs.push(chunk);
                    retrieved_chunk_ids.push(r.chunk_id);
                    scores.push(r.score);
                }
            }

            let (recall, precision, iou, omega) =
                if question.dimensions.operation == CognitiveOperation::Adversarial {
                    score_trick_question(retrieved_refs.len())
                } else {
                    let (r, p, i) = score_question(question, &retrieved_refs);
                    let o = precision_omega(question, &variant.chunks);
                    (r, p, i, o)
                };

            recall_scores.push(recall);
            precision_scores.push(precision);
            iou_scores.push(iou);
            omega_scores.push(omega);

            traces.push(RetrievalTraceEntry {
                question_sequence: question.sequence,
                retrieved_chunk_ids,
                scores,
                recall,
                precision,
                iou,
                operation: question.dimensions.operation.as_str().to_string(),
                evidence_kind: question.dimensions.evidence.as_str().to_string(),
            });
        }

        let average_chunk_tokens = if variant.chunk_token_counts.is_empty() {
            0
        } else {
            let total: u64 = variant.chunk_token_counts.values().map(|n| *n as u64).sum();
            (total / variant.chunk_token_counts.len() as u64) as u32
        };

        let average_retrieved_tokens = if traces.is_empty() {
            0
        } else {
            let mut question_totals: u64 = 0;
            for trace in &traces {
                let mut q_total: u64 = 0;
                for id in &trace.retrieved_chunk_ids {
                    if let Some(&n) = variant.chunk_token_counts.get(id) {
                        q_total += n as u64;
                    }
                }
                question_totals += q_total;
            }
            (question_totals / traces.len() as u64) as u32
        };

        let weights = EvaluationScorePolicy::default().weights();
        let composite_per_question: Vec<f32> = recall_scores
            .iter()
            .zip(iou_scores.iter())
            .zip(precision_scores.iter())
            .zip(omega_scores.iter())
            .map(|(((r, i), p), o)| {
                r * weights.recall
                    + i * weights.iou
                    + p * weights.precision
                    + o * weights.precision_omega
            })
            .collect();

        let seed = ci_seed(run_id, &variant.label, options);
        const BOOTSTRAP_SAMPLES: usize = 500;
        const BOOTSTRAP_ALPHA: f32 = 0.05;
        let (recall_ci_low, recall_ci_high) = bootstrap_ci(
            &recall_scores,
            seed ^ 0x1,
            BOOTSTRAP_SAMPLES,
            BOOTSTRAP_ALPHA,
        );
        let (precision_ci_low, precision_ci_high) = bootstrap_ci(
            &precision_scores,
            seed ^ 0x2,
            BOOTSTRAP_SAMPLES,
            BOOTSTRAP_ALPHA,
        );
        let (iou_ci_low, iou_ci_high) =
            bootstrap_ci(&iou_scores, seed ^ 0x3, BOOTSTRAP_SAMPLES, BOOTSTRAP_ALPHA);
        let (precision_omega_ci_low, precision_omega_ci_high) = bootstrap_ci(
            &omega_scores,
            seed ^ 0x4,
            BOOTSTRAP_SAMPLES,
            BOOTSTRAP_ALPHA,
        );
        let (composite_ci_low, composite_ci_high) = bootstrap_ci(
            &composite_per_question,
            seed ^ 0x5,
            BOOTSTRAP_SAMPLES,
            BOOTSTRAP_ALPHA,
        );

        let metrics = EvaluationMetrics {
            recall_mean: mean(&recall_scores),
            recall_std: std_dev(&recall_scores),
            precision_mean: mean(&precision_scores),
            precision_std: std_dev(&precision_scores),
            iou_mean: mean(&iou_scores),
            iou_std: std_dev(&iou_scores),
            precision_omega_mean: mean(&omega_scores),
            precision_omega_std: std_dev(&omega_scores),
            chunk_count: variant.chunks.len() as u32,
            average_chunk_tokens,
            average_retrieved_tokens,
            recall_ci_low,
            recall_ci_high,
            precision_ci_low,
            precision_ci_high,
            iou_ci_low,
            iou_ci_high,
            precision_omega_ci_low,
            precision_omega_ci_high,
            composite_ci_low,
            composite_ci_high,
            judge_score: None,
        };

        Ok((metrics, traces))
    }
}

fn ci_seed(run_id: Uuid, variant_label: &str, options: &EvaluationRunOptions) -> u64 {
    let bytes = run_id.as_bytes();
    let mut acc: u64 = 0xCBF2_9CE4_8422_2325;
    for chunk in bytes.chunks(8) {
        let mut buf = [0u8; 8];
        for (slot, src) in buf.iter_mut().zip(chunk.iter()) {
            *slot = *src;
        }
        acc ^= u64::from_le_bytes(buf);
        acc = acc.wrapping_mul(0x100000001B3);
    }
    for b in variant_label.as_bytes() {
        acc ^= *b as u64;
        acc = acc.wrapping_mul(0x100000001B3);
    }
    acc ^= options.top_k as u64;
    acc = acc.wrapping_mul(0x100000001B3);
    acc ^= options.min_score_milli as u64;
    acc = acc.wrapping_mul(0x100000001B3);
    acc
}
