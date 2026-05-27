use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::server::application::embedding::EmbedderRegistry;
use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::shared::contracts::Timings;
use crate::shared::reference_data::AiProviderKind;
use crate::shared::EmbedResult;

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
    ) -> Result<EmbedResult, AppError> {
        let started = Instant::now();
        let embed_start = Instant::now();
        let texts = vec![text_a.to_string(), text_b.to_string()];
        let vecs = self.embed_batch(embedding_model_id, &texts).await?;
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
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let similarity = if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        };

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
