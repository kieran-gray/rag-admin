use std::collections::HashMap;

use uuid::Uuid;

use crate::server::application::embedding::ResolvedEmbeddingModel;
use crate::server::application::llm::ResolvedGenerationModel;
use crate::server::domain::chunk_set::entity::Chunk;
use crate::server::domain::evaluation::question::EvaluationQuestion;
use crate::shared::ChunkingConfig;

pub struct RunContext {
    pub document_id: Uuid,
    pub document_version: u32,
    pub plain_text: String,
    pub embedding_model: ResolvedEmbeddingModel,
    pub generation_model: ResolvedGenerationModel,
    pub questions: Vec<EvaluationQuestion>,
    pub question_embeddings: Vec<Vec<f32>>,
}

pub struct PreparedVariant {
    pub label: String,
    pub config: ChunkingConfig,
    pub chunk_set_id: Uuid,
    pub embedding_set_id: Uuid,
    pub chunks: Vec<Chunk>,
    pub chunk_token_counts: HashMap<Uuid, u32>,
}

pub struct QuestionSubset<'a> {
    pub questions: Vec<&'a EvaluationQuestion>,
    pub embeddings: Vec<Vec<f32>>,
}

impl<'a> QuestionSubset<'a> {
    pub fn from_indices(
        questions: &'a [EvaluationQuestion],
        embeddings: &[Vec<f32>],
        indices: &[usize],
    ) -> Self {
        Self {
            questions: indices.iter().filter_map(|&i| questions.get(i)).collect(),
            embeddings: indices
                .iter()
                .filter_map(|&i| embeddings.get(i).cloned())
                .collect(),
        }
    }
}
