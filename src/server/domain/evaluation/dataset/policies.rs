use crate::server::event_sourcing::effect::{IdempotencyKey, PendingEffect};
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::EvaluationDataset;
use super::events::{DatasetGenerationRequested, EvaluationDatasetEvent};

use crate::server::application::evaluation::effects::dataset::{
    EvaluationDatasetEffect, GenerateDatasetEffect,
};

fn for_dataset_generation_requested(
    event: &DatasetGenerationRequested,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<PendingEffect<EvaluationDatasetEffect>> {
    vec![PendingEffect {
        stream_id: event.dataset_id,
        event_log_position: ctx.envelope.metadata.log_position,
        effect_type: "generate_dataset",
        idempotency_key: IdempotencyKey::new(
            event.dataset_id,
            ctx.envelope.metadata.log_position,
            "generate_dataset",
        ),
        payload: EvaluationDatasetEffect::GenerateDataset(GenerateDatasetEffect {
            dataset_id: event.dataset_id,
            document_id: event.document_id,
            target_question_count: event.target_question_count,
            generation_model_id: event.generation_model_id,
            embedding_model_id: event.embedding_model_id,
            excerpt_similarity_threshold_milli: event.excerpt_similarity_threshold_milli,
            duplicate_similarity_threshold_milli: event.duplicate_similarity_threshold_milli,
        }),
    }]
}

impl HasPolicies<EvaluationDataset, EvaluationDatasetEvent, EvaluationDatasetEffect>
    for DatasetGenerationRequested
{
    fn policies() -> &'static [PolicyFn<
        Self,
        EvaluationDataset,
        EvaluationDatasetEvent,
        EvaluationDatasetEffect,
    >] {
        &[for_dataset_generation_requested]
    }
}

pub fn derive_dataset_effects(
    envelope: &EventEnvelope<EvaluationDatasetEvent>,
    state: &EvaluationDataset,
) -> Vec<PendingEffect<EvaluationDatasetEffect>> {
    let ctx = PolicyContext::new(envelope, state);
    match &envelope.event {
        EvaluationDatasetEvent::DatasetGenerationRequested(e) => e.apply_policies(&ctx),
        EvaluationDatasetEvent::QuestionAccepted(_)
        | EvaluationDatasetEvent::QuestionRejected(_)
        | EvaluationDatasetEvent::DatasetGenerationCompleted(_)
        | EvaluationDatasetEvent::DatasetGenerationFailed(_)
        | EvaluationDatasetEvent::DatasetRenamed(_)
        | EvaluationDatasetEvent::DatasetDeleted(_) => Vec::new(),
    }
}
