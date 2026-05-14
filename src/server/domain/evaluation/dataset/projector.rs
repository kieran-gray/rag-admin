use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::domain::evaluation::dataset::events::EvaluationDatasetEvent;
use crate::server::domain::evaluation::dataset::read_model::NewDatasetSummary;
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::question::EvaluationQuestion;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

pub struct EvaluationDatasetProjector {
    repository: Arc<dyn EvaluationDatasetRepository>,
}

impl EvaluationDatasetProjector {
    pub const NAME: &'static str = "evaluation_dataset_projector";

    pub fn new(repository: Arc<dyn EvaluationDatasetRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<EvaluationDatasetEvent> for EvaluationDatasetProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<EvaluationDatasetEvent>],
    ) -> Result<(), AppError> {
        for envelope in events {
            match &envelope.event {
                EvaluationDatasetEvent::DatasetGenerationRequested(e) => {
                    self.repository
                        .insert_summary(NewDatasetSummary {
                            dataset_id: e.dataset_id,
                            document_id: e.document_id,
                            document_version: e.document_version,
                            content_hash: e.content_hash.clone(),
                            label: e.label.clone(),
                            target_question_count: e.target_question_count,
                            generation_model_id: e.generation_model_id,
                            generation_model: e.generation_model.clone(),
                            excerpt_similarity_threshold_milli: e
                                .excerpt_similarity_threshold_milli,
                            duplicate_similarity_threshold_milli: e
                                .duplicate_similarity_threshold_milli,
                            embedding_model_id: e.embedding_model_id,
                            created_at: e.occurred_at.clone(),
                        })
                        .await?;
                }
                EvaluationDatasetEvent::QuestionAccepted(e) => {
                    self.repository
                        .save_question(
                            e.dataset_id,
                            EvaluationQuestion {
                                sequence: e.sequence,
                                question: e.question.clone(),
                                references: e.references.clone(),
                                embedding: e.embedding.clone(),
                            },
                        )
                        .await?;
                }
                EvaluationDatasetEvent::QuestionRejected(e) => {
                    self.repository
                        .increment_rejection_count(e.dataset_id)
                        .await?;
                }
                EvaluationDatasetEvent::DatasetGenerationCompleted(e) => {
                    self.repository.mark_completed(e.dataset_id).await?;
                }
                EvaluationDatasetEvent::DatasetGenerationFailed(e) => {
                    self.repository
                        .mark_failed(e.dataset_id, e.reason.clone())
                        .await?;
                }
                EvaluationDatasetEvent::DatasetRenamed(e) => {
                    self.repository
                        .rename(e.dataset_id, e.label.clone())
                        .await?;
                }
                EvaluationDatasetEvent::DatasetDeleted(e) => {
                    self.repository.mark_deleted(e.dataset_id).await?;
                }
            }
        }
        Ok(())
    }
}
