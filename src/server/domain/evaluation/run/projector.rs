use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::domain::evaluation::run::events::EvaluationRunEvent;
use crate::server::domain::evaluation::run::read_model::{
    EvaluationVariantResultDto, NewRunSummary,
};
use crate::server::domain::evaluation::run::repository::EvaluationRunRepository;
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::projector::Projector;

pub struct EvaluationRunProjector {
    repository: Arc<dyn EvaluationRunRepository>,
}

impl EvaluationRunProjector {
    pub const NAME: &'static str = "evaluation_run_projector";

    pub fn new(repository: Arc<dyn EvaluationRunRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<EvaluationRunEvent> for EvaluationRunProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(&self, events: &[EventEnvelope<EvaluationRunEvent>]) -> Result<(), AppError> {
        for envelope in events {
            match &envelope.event {
                EvaluationRunEvent::RunRequested(e) => {
                    self.repository
                        .insert_summary(NewRunSummary {
                            run_id: e.run_id,
                            dataset_id: e.dataset_id,
                            pipeline_configuration_id: e.pipeline_configuration_id,
                            document_id: e.document_id,
                            document_version: e.document_version,
                            variants: e.variants.clone(),
                            options: e.options.clone(),
                            autotune_request: e.autotune_request.clone(),
                            variants_count: e.variants.len() as u32,
                            scoring_policy: e.scoring_policy,
                            created_at: e.occurred_at.clone(),
                        })
                        .await?;
                }
                EvaluationRunEvent::VariantPrepared(e) => {
                    self.repository.record_variant_prepared(e.run_id).await?;
                }
                EvaluationRunEvent::VariantScored(e) => {
                    self.repository
                        .save_variant_result(EvaluationVariantResultDto {
                            run_id: e.run_id,
                            variant_label: e.variant_label.clone(),
                            variant_config: e.variant_config,
                            options: e.options.clone(),
                            split: e.split,
                            recall_mean: e.metrics.recall_mean,
                            recall_std: e.metrics.recall_std,
                            precision_mean: e.metrics.precision_mean,
                            precision_std: e.metrics.precision_std,
                            iou_mean: e.metrics.iou_mean,
                            iou_std: e.metrics.iou_std,
                            precision_omega_mean: e.metrics.precision_omega_mean,
                            precision_omega_std: e.metrics.precision_omega_std,
                            chunk_set_id: e.chunk_set_id,
                            embedding_set_id: e.embedding_set_id,
                            selected: e.selected,
                            retrieval_traces: e.retrieval_traces.clone(),
                        })
                        .await?;
                }
                EvaluationRunEvent::RunCompleted(e) => {
                    self.repository.mark_completed(e.run_id).await?;
                }
                EvaluationRunEvent::RunFailed(e) => {
                    self.repository
                        .mark_failed(e.run_id, e.reason.clone())
                        .await?;
                }
            }
        }
        Ok(())
    }
}
