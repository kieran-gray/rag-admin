use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::dataset::generator::GenerationPlan;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::SourceDocumentQueryService;
use crate::server::application::AppError;
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::dataset::commands::{
    CancelDatasetGeneration, DeleteDataset, EvaluationDatasetCommand, RenameDataset,
    RequestDatasetGeneration,
};
use crate::server::domain::evaluation::dataset::events::AxisWeight;
use crate::shared::contracts::EvaluationJobInfo;
use event_sourcing::CommandProcessor;

pub struct StartDatasetGenerationRequest {
    pub document_id: Uuid,
    pub generation_model_id: Uuid,
    pub embedding_model_id: Uuid,
    pub label: String,
    pub question_count: u32,
    pub duplicate_similarity_threshold_milli: u32,
    pub weight_matrix: Vec<AxisWeight>,
}

pub struct EvaluationDatasetCommandHandler {
    processor: Arc<CommandProcessor<EvaluationDataset>>,
    embedding_service: Arc<EmbeddingService>,
    generation_service: Arc<GenerationService>,
    documents: Arc<SourceDocumentQueryService>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl EvaluationDatasetCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<EvaluationDataset>>,
        embedding_service: Arc<EmbeddingService>,
        generation_service: Arc<GenerationService>,
        documents: Arc<SourceDocumentQueryService>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            embedding_service,
            generation_service,
            documents,
            clock,
            id_generator,
        })
    }

    pub async fn start_generation(
        &self,
        request: StartDatasetGenerationRequest,
    ) -> Result<EvaluationJobInfo, AppError> {
        let generation_model = self
            .generation_service
            .resolve(request.generation_model_id)
            .await?;
        let embedding_model = self
            .embedding_service
            .resolve(request.embedding_model_id)
            .await?;

        let detail = self
            .documents
            .get_detail(request.document_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("document {} not found", request.document_id))
            })?;
        let document = detail.document;

        let dataset_id = self.id_generator.new_uuid();
        let occurred_at = self.clock.now();

        let target = request.question_count.max(1);
        let max_attempts = target.saturating_mul(12).max(target.saturating_add(12));

        let weight_matrix = if request.weight_matrix.is_empty() {
            GenerationPlan::default_weight_matrix()
        } else {
            request.weight_matrix
        };

        self.processor
            .handle(
                dataset_id,
                EvaluationDatasetCommand::RequestDatasetGeneration(RequestDatasetGeneration {
                    dataset_id,
                    document_id: request.document_id,
                    document_version: document.latest_version,
                    content_hash: document.latest_content_hash,
                    label: request.label,
                    target_question_count: target,
                    generation_model_id: generation_model.generation_model_id,
                    generation_model: generation_model.model.clone(),
                    duplicate_similarity_threshold_milli: request
                        .duplicate_similarity_threshold_milli
                        .min(1000),
                    embedding_model_id: embedding_model.embedding_model_id,
                    max_attempts,
                    weight_matrix,
                    occurred_at,
                }),
            )
            .await?;

        Ok(EvaluationJobInfo {
            job_id: dataset_id.to_string(),
            stream_url: format!("/api/events/ws?stream_id={dataset_id}"),
        })
    }

    pub async fn rename(&self, dataset_id: Uuid, label: String) -> Result<(), AppError> {
        self.processor
            .handle(
                dataset_id,
                EvaluationDatasetCommand::RenameDataset(RenameDataset {
                    dataset_id,
                    label,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn delete(&self, dataset_id: Uuid) -> Result<(), AppError> {
        self.processor
            .handle(
                dataset_id,
                EvaluationDatasetCommand::DeleteDataset(DeleteDataset {
                    dataset_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn cancel_generation(&self, dataset_id: Uuid) -> Result<(), AppError> {
        self.processor
            .handle(
                dataset_id,
                EvaluationDatasetCommand::CancelDatasetGeneration(CancelDatasetGeneration {
                    dataset_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }
}
