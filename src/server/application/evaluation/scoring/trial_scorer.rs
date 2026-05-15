use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::core::{ChunkingConfig, EvaluationMetrics, EvaluationRunOptions, EvaluationScorePolicy};
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::PipelineResolver;
use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::evaluation::ports::{RetrievalQuery, RetrievedChunk, Retriever};
use crate::server::application::llm::ResolvedGenerationModel;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::entity::{Chunk, ChunkSet};
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::embedding_set::entity::{ChunkEmbedding, EmbeddingSet};
use crate::server::domain::embedding_set::repository::EmbeddingSetRepository;
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::question::{EvaluationQuestion, QuestionCategory};
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::scoring::{
    bootstrap_ci, mean, precision_omega, score_question, score_trick_question, std_dev,
};
use crate::server::domain::source_document::repository::SourceDocumentRepository;

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
            questions: indices.iter().map(|&i| &questions[i]).collect(),
            embeddings: indices.iter().map(|&i| embeddings[i].clone()).collect(),
        }
    }
}

pub struct TrialScorer {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    chunker_registry: Arc<ChunkerRegistry>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    embedding_service: Arc<EmbeddingService>,
    embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
    dataset_repository: Arc<dyn EvaluationDatasetRepository>,
    retriever: Arc<dyn Retriever>,
    pipeline_resolver: Arc<PipelineResolver>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl TrialScorer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        blob_store: Arc<dyn BlobStore>,
        chunker_registry: Arc<ChunkerRegistry>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        embedding_service: Arc<EmbeddingService>,
        embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
        dataset_repository: Arc<dyn EvaluationDatasetRepository>,
        retriever: Arc<dyn Retriever>,
        pipeline_resolver: Arc<PipelineResolver>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            blob_store,
            chunker_registry,
            chunk_set_repository,
            embedding_service,
            embedding_set_repository,
            dataset_repository,
            retriever,
            pipeline_resolver,
            clock,
            id_generator,
        })
    }

    pub async fn load_run_context(
        &self,
        dataset_id: Uuid,
        pipeline_configuration_id: Uuid,
    ) -> Result<RunContext, AppError> {
        let dataset = self
            .dataset_repository
            .load(dataset_id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("evaluation dataset {dataset_id}")))?;

        let questions = self
            .dataset_repository
            .load_questions(dataset_id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if questions.is_empty() {
            return Err(AppError::Validation(
                "evaluation dataset has no questions".into(),
            ));
        }

        let doc = self
            .source_document_repository
            .load(dataset.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", dataset.document_id)))?;
        let bytes = self.blob_store.get(&doc.latest_content_hash).await?;
        let plain_text = String::from_utf8(bytes)
            .map_err(|e| AppError::Internal(format!("document content is not valid UTF-8: {e}")))?;

        let pipeline = self
            .pipeline_resolver
            .resolve(pipeline_configuration_id)
            .await?;
        let embedding_model = pipeline.embedding_model.clone();
        let generation_model = pipeline.generation_model.clone();

        let question_texts: Vec<String> = questions.iter().map(|q| q.question.clone()).collect();
        let question_embeddings = self
            .embedding_service
            .embed_with_resolved(&embedding_model, &question_texts)
            .await?;

        Ok(RunContext {
            document_id: dataset.document_id,
            document_version: dataset.document_version,
            plain_text,
            embedding_model,
            generation_model,
            questions,
            question_embeddings,
        })
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

    pub async fn prepare_variant(
        &self,
        ctx: &RunContext,
        label: String,
        config: ChunkingConfig,
    ) -> Result<PreparedVariant, AppError> {
        let (chunk_set_id, chunks) = self
            .find_or_create_chunk_set(
                ctx.document_id,
                ctx.document_version,
                &ctx.plain_text,
                &config,
            )
            .await?;

        let chunk_token_counts = self.count_tokens(&chunks)?;
        let embedding_set_id = self
            .find_or_create_embedding_set(chunk_set_id, &chunks, &ctx.embedding_model)
            .await?;

        Ok(PreparedVariant {
            label,
            config,
            chunk_set_id,
            embedding_set_id,
            chunks,
            chunk_token_counts,
        })
    }

    fn count_tokens(&self, chunks: &[Chunk]) -> Result<HashMap<Uuid, u32>, AppError> {
        let tokenizer = self.chunker_registry.tokenizer();
        let mut map = HashMap::with_capacity(chunks.len());
        for chunk in chunks {
            map.insert(chunk.chunk_id, tokenizer.count(&chunk.text)?);
        }
        Ok(map)
    }

    async fn find_or_create_chunk_set(
        &self,
        document_id: Uuid,
        document_version: u32,
        plain_text: &str,
        config: &ChunkingConfig,
    ) -> Result<(Uuid, Vec<Chunk>), AppError> {
        let existing = self
            .chunk_set_repository
            .list_for_document(document_id)
            .await?;

        if let Some(cs) = existing
            .iter()
            .find(|cs| cs.document_version == document_version && cs.chunking_config == *config)
        {
            let chunks = self
                .chunk_set_repository
                .load_chunks(cs.chunk_set_id)
                .await?;
            return Ok((cs.chunk_set_id, chunks));
        }

        let chunk_outputs = self
            .chunker_registry
            .chunk_markdown(config, plain_text)
            .await
            .map_err(|e| AppError::Internal(format!("chunking failed: {e}")))?;

        let chunk_set_id = self.id_generator.new_uuid();
        let occurred_at = self.clock.now();
        let chunks: Vec<Chunk> = chunk_outputs
            .into_iter()
            .enumerate()
            .map(|(i, co)| Chunk {
                chunk_id: self.id_generator.new_uuid(),
                chunk_set_id,
                sequence: i as u32,
                heading: co.heading,
                text: co.text,
                char_start: co.char_start,
                char_end: co.char_end,
            })
            .collect();

        let chunk_set = ChunkSet {
            chunk_set_id,
            document_id,
            document_version,
            chunking_config: *config,
            created_at: occurred_at.to_string(),
        };
        self.chunk_set_repository
            .save(chunk_set, chunks.clone())
            .await?;

        Ok((chunk_set_id, chunks))
    }

    async fn find_or_create_embedding_set(
        &self,
        chunk_set_id: Uuid,
        chunks: &[Chunk],
        embedding_model: &ResolvedEmbeddingModel,
    ) -> Result<Uuid, AppError> {
        if let Some(existing) = self
            .embedding_set_repository
            .find_by(chunk_set_id, embedding_model.embedding_model_id)
            .await?
        {
            return Ok(existing.embedding_set_id);
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let vectors = self
            .embedding_service
            .embed_with_resolved(embedding_model, &texts)
            .await?;

        let embedding_set_id = self.id_generator.new_uuid();
        let occurred_at = self.clock.now();
        let embedding_set = EmbeddingSet {
            embedding_set_id,
            chunk_set_id,
            embedding_model_id: embedding_model.embedding_model_id,
            embedding_model_snapshot:
                crate::server::domain::configuration::embedding_model::EmbeddingModel {
                    embedding_model_id: embedding_model.embedding_model_id,
                    kind: embedding_model.kind,
                    model: embedding_model.model.clone(),
                    dimensions: embedding_model.dimensions,
                },
            dimensions: embedding_model.dimensions,
            created_at: occurred_at.to_string(),
        };

        let embeddings: Vec<ChunkEmbedding> = chunks
            .iter()
            .zip(vectors.iter())
            .map(|(chunk, vec)| ChunkEmbedding {
                chunk_id: chunk.chunk_id,
                embedding_set_id,
                vector: vec.clone(),
            })
            .collect();

        self.embedding_set_repository
            .save(embedding_set, embeddings)
            .await?;

        Ok(embedding_set_id)
    }

    pub async fn score_variant(
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

        for (q_idx, (question, q_emb)) in subset
            .questions
            .iter()
            .zip(subset.embeddings.iter())
            .enumerate()
        {
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

            let (recall, precision, iou, omega) = if question.category == QuestionCategory::Trick {
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
                question_sequence: q_idx as u32,
                retrieved_chunk_ids,
                scores,
                recall,
                precision,
                iou,
                category: question.category.as_str().to_string(),
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
        let composite_per_question: Vec<f32> = (0..n)
            .map(|i| {
                recall_scores[i] * weights.recall
                    + iou_scores[i] * weights.iou
                    + precision_scores[i] * weights.precision
                    + omega_scores[i] * weights.precision_omega
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
        buf[..chunk.len()].copy_from_slice(chunk);
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
