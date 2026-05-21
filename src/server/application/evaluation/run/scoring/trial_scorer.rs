use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::IndexProfileResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::ports::Retriever;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::embedding_set::repository::EmbeddingSetRepository;
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::shared::{ChunkingConfig, EvaluationMetrics, EvaluationRunOptions};

use super::run_context::{PreparedVariant, QuestionSubset, RunContext};
use super::variant_indexer::VariantIndexer;
use super::variant_retrieval_scorer::VariantRetrievalScorer;

pub struct TrialScorer {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    dataset_repository: Arc<dyn EvaluationDatasetRepository>,
    index_profile_resolver: Arc<IndexProfileResolver>,
    generation_service: Arc<GenerationService>,
    embedding_service: Arc<EmbeddingService>,
    indexer: Arc<VariantIndexer>,
    scorer: Arc<VariantRetrievalScorer>,
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
        index_profile_resolver: Arc<IndexProfileResolver>,
        generation_service: Arc<GenerationService>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        let indexer = VariantIndexer::new(
            chunker_registry,
            chunk_set_repository,
            Arc::clone(&embedding_service),
            embedding_set_repository,
            clock,
            id_generator,
        );
        let scorer = VariantRetrievalScorer::new(retriever);
        Arc::new(Self {
            source_document_repository,
            blob_store,
            dataset_repository,
            index_profile_resolver,
            generation_service,
            embedding_service,
            indexer,
            scorer,
        })
    }

    pub async fn load_run_context(
        &self,
        dataset_id: Uuid,
        index_profile_id: Uuid,
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

        let index_profile = self
            .index_profile_resolver
            .resolve(index_profile_id)
            .await?;
        let embedding_model = index_profile.embedding_model.clone();
        let generation_model = self
            .generation_service
            .resolve(dataset.generation_model_id)
            .await?;

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

    pub async fn prepare_variant(
        &self,
        ctx: &RunContext,
        label: String,
        config: ChunkingConfig,
    ) -> Result<PreparedVariant, AppError> {
        self.indexer.prepare(ctx, label, config).await
    }

    pub async fn score_variant(
        &self,
        run_id: Uuid,
        variant: &PreparedVariant,
        subset: &QuestionSubset<'_>,
        options: &EvaluationRunOptions,
    ) -> Result<(EvaluationMetrics, Vec<RetrievalTraceEntry>), AppError> {
        self.scorer.score(run_id, variant, subset, options).await
    }

    pub async fn retrieve_passage(
        &self,
        variant: &PreparedVariant,
        query_vector: &[f32],
        options: &EvaluationRunOptions,
    ) -> Result<Vec<(Uuid, f32, String)>, AppError> {
        self.scorer
            .retrieve_passage(variant, query_vector, options)
            .await
    }
}
