use std::cmp::Ordering;
use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::server::application::embedding::EmbedderRegistry;
use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::shared::contracts::Timings;
use crate::shared::reference_data::AiProviderKind;
use crate::shared::{
    EmbedInputType, EmbedManyCandidate, EmbedManyResult, EmbedMatrixResult, EmbedResult,
};

const SNIPPET_MAX_CHARS: usize = 120;
const MAX_MATRIX_TEXTS: usize = 16;

#[derive(Debug, Clone)]
pub struct ResolvedEmbeddingModel {
    pub embedding_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
    pub dimensions: u32,
}

pub struct EmbeddingService {
    embedders: EmbedderRegistry,
    embedding_models: Arc<dyn EmbeddingModelRepository>,
}

impl EmbeddingService {
    pub fn new(
        embedders: EmbedderRegistry,
        embedding_models: Arc<dyn EmbeddingModelRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            embedders,
            embedding_models,
        })
    }

    pub async fn embed_batch(
        &self,
        embedding_model_id: Uuid,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let resolved = self.resolve(embedding_model_id).await?;
        self.embed_with_resolved(&resolved, texts).await
    }

    pub async fn embed_with_resolved(
        &self,
        model: &ResolvedEmbeddingModel,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let embedder = self.embedders.get(&model.kind).ok_or_else(|| {
            AppError::Internal(format!(
                "no embedder registered for provider kind {}",
                model.kind.as_str()
            ))
        })?;
        let vecs = embedder
            .embed_batch(&model.model, model.dimensions, texts)
            .await?;
        verify_dims(model, &vecs)?;
        Ok(vecs)
    }

    pub async fn resolve(
        &self,
        embedding_model_id: Uuid,
    ) -> Result<ResolvedEmbeddingModel, AppError> {
        let model = self
            .embedding_models
            .find_by_id(embedding_model_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "embedding model {embedding_model_id} not registered"
                ))
            })?;
        Ok(ResolvedEmbeddingModel {
            embedding_model_id: model.embedding_model_id,
            kind: model.kind,
            model: model.model,
            dimensions: model.dimensions,
        })
    }

    pub async fn embed_texts(
        &self,
        embedding_model_id: Uuid,
        text_a: &str,
        text_b: &str,
        type_a: EmbedInputType,
        type_b: EmbedInputType,
    ) -> Result<EmbedResult, AppError> {
        let started = Instant::now();
        let resolved = self.resolve(embedding_model_id).await?;

        let a_prepared = apply_prefix(&resolved.model, text_a, type_a);
        let b_prepared = apply_prefix(&resolved.model, text_b, type_b);

        let embed_start = Instant::now();
        let texts = vec![a_prepared, b_prepared];
        let vecs = self.embed_with_resolved(&resolved, &texts).await?;
        let embed_ms = u32::try_from(embed_start.elapsed().as_millis()).unwrap_or(u32::MAX);

        let (Some(a), Some(b)) = (vecs.first(), vecs.get(1)) else {
            return Err(AppError::Internal(
                "embedder returned unexpected result".into(),
            ));
        };
        if a.is_empty() {
            return Err(AppError::Internal(
                "embedder returned unexpected result".into(),
            ));
        }

        let norm_a = l2_norm(a);
        let norm_b = l2_norm(b);
        let similarity = cosine(a, b, norm_a, norm_b);

        let total_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);

        Ok(EmbedResult {
            dims: a.len(),
            norm_a,
            norm_b,
            similarity,
            timings: Timings {
                embed_ms,
                total_ms,
                ..Timings::default()
            },
        })
    }

    pub async fn embed_many(
        &self,
        embedding_model_id: Uuid,
        query: &str,
        candidates: &[String],
        query_type: EmbedInputType,
        candidate_type: EmbedInputType,
    ) -> Result<EmbedManyResult, AppError> {
        if query.trim().is_empty() {
            return Err(AppError::Validation("query is empty".into()));
        }
        let candidates: Vec<String> = candidates
            .iter()
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_string())
            .collect();
        if candidates.is_empty() {
            return Err(AppError::Validation("no candidate texts provided".into()));
        }

        let started = Instant::now();
        let resolved = self.resolve(embedding_model_id).await?;

        let mut texts: Vec<String> = Vec::with_capacity(candidates.len() + 1);
        texts.push(apply_prefix(&resolved.model, query, query_type));
        for c in &candidates {
            texts.push(apply_prefix(&resolved.model, c, candidate_type));
        }

        let embed_start = Instant::now();
        let vecs = self.embed_with_resolved(&resolved, &texts).await?;
        let embed_ms = u32::try_from(embed_start.elapsed().as_millis()).unwrap_or(u32::MAX);

        let Some(query_vec) = vecs.first() else {
            return Err(AppError::Internal(
                "embedder returned unexpected result".into(),
            ));
        };
        let query_norm = l2_norm(query_vec);

        let mut scored: Vec<EmbedManyCandidate> = Vec::with_capacity(candidates.len());
        for (i, candidate_text) in candidates.iter().enumerate() {
            let Some(candidate_vec) = vecs.get(i + 1) else {
                continue;
            };
            let norm = l2_norm(candidate_vec);
            let similarity = cosine(query_vec, candidate_vec, query_norm, norm);
            scored.push(EmbedManyCandidate {
                text_preview: snippet(candidate_text),
                norm,
                similarity,
            });
        }
        scored.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(Ordering::Equal)
        });

        let total_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);

        Ok(EmbedManyResult {
            dims: query_vec.len(),
            query_norm,
            candidates: scored,
            timings: Timings {
                embed_ms,
                total_ms,
                ..Timings::default()
            },
        })
    }

    pub async fn embed_matrix(
        &self,
        embedding_model_id: Uuid,
        texts: &[String],
        input_type: EmbedInputType,
    ) -> Result<EmbedMatrixResult, AppError> {
        let texts: Vec<String> = texts
            .iter()
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_string())
            .collect();
        if texts.len() < 2 {
            return Err(AppError::Validation(
                "matrix mode needs at least 2 texts".into(),
            ));
        }
        if texts.len() > MAX_MATRIX_TEXTS {
            return Err(AppError::Validation(format!(
                "matrix mode accepts up to {MAX_MATRIX_TEXTS} texts"
            )));
        }

        let started = Instant::now();
        let resolved = self.resolve(embedding_model_id).await?;

        let prepared: Vec<String> = texts
            .iter()
            .map(|t| apply_prefix(&resolved.model, t, input_type))
            .collect();

        let embed_start = Instant::now();
        let vecs = self.embed_with_resolved(&resolved, &prepared).await?;
        let embed_ms = u32::try_from(embed_start.elapsed().as_millis()).unwrap_or(u32::MAX);

        let norms: Vec<f32> = vecs.iter().map(|v| l2_norm(v)).collect();
        let n = vecs.len();
        let mut matrix = vec![vec![0.0_f32; n]; n];
        for i in 0..n {
            for j in 0..n {
                let Some(vi) = vecs.get(i) else { continue };
                let Some(vj) = vecs.get(j) else { continue };
                let Some(&ni) = norms.get(i) else { continue };
                let Some(&nj) = norms.get(j) else { continue };
                let sim = cosine(vi, vj, ni, nj);
                if let Some(row) = matrix.get_mut(i) {
                    if let Some(cell) = row.get_mut(j) {
                        *cell = sim;
                    }
                }
            }
        }

        let total_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let dims = vecs.first().map_or(0, Vec::len);
        let previews: Vec<String> = texts.iter().map(|t| snippet(t)).collect();

        Ok(EmbedMatrixResult {
            dims,
            previews,
            norms,
            matrix,
            timings: Timings {
                embed_ms,
                total_ms,
                ..Timings::default()
            },
        })
    }
}

fn verify_dims(model: &ResolvedEmbeddingModel, vecs: &[Vec<f32>]) -> Result<(), AppError> {
    if let Some(first) = vecs.first() {
        if first.len() as u32 != model.dimensions {
            return Err(AppError::Validation(format!(
                "embedder returned dims={} but model '{}' declares dims={}",
                first.len(),
                model.model,
                model.dimensions
            )));
        }
    }
    Ok(())
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine(a: &[f32], b: &[f32], norm_a: f32, norm_b: f32) -> f32 {
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    dot / (norm_a * norm_b)
}

fn apply_prefix(model: &str, text: &str, input_type: EmbedInputType) -> String {
    let family = model_family(model);
    match (family, input_type) {
        (PrefixFamily::E5, EmbedInputType::Query) => format!("query: {text}"),
        (PrefixFamily::E5, EmbedInputType::Passage) => format!("passage: {text}"),
        (PrefixFamily::Bge | PrefixFamily::Snowflake, EmbedInputType::Query) => {
            format!("Represent this sentence for searching relevant passages: {text}")
        }
        _ => text.to_string(),
    }
}

#[derive(Clone, Copy)]
enum PrefixFamily {
    None,
    E5,
    Bge,
    Snowflake,
}

fn model_family(model: &str) -> PrefixFamily {
    let m = model.to_ascii_lowercase();
    if m.contains("e5-") || m.contains("e5_") || m.ends_with("/e5") {
        return PrefixFamily::E5;
    }
    if m.contains("bge") {
        return PrefixFamily::Bge;
    }
    if m.contains("snowflake") {
        return PrefixFamily::Snowflake;
    }
    PrefixFamily::None
}

fn snippet(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= SNIPPET_MAX_CHARS {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(SNIPPET_MAX_CHARS).collect();
    out.push('…');
    out
}
