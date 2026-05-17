use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::job_queue::{IdempotencyKey, NewJob};
use crate::server::event_sourcing::policy::PolicyContext;

use super::aggregate::{DatasetGenerationStatus, EvaluationDataset};
use super::effects::{EvaluationDatasetEffect, GenerateParaphraseEffect, GenerateQuestionEffect};
use super::events::EvaluationDatasetEvent;

const ATTEMPT_EFFECT: &str = "attempt_question_generation";
const PARAPHRASE_EFFECT: &str = "generate_paraphrase";

pub fn derive_dataset_effects(
    envelope: &EventEnvelope<EvaluationDatasetEvent>,
    state: &EvaluationDataset,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    let ctx = PolicyContext::new(envelope, state);

    if !matches!(state.status, DatasetGenerationStatus::Generating) {
        return Vec::new();
    }

    match &envelope.event {
        EvaluationDatasetEvent::DatasetGenerationRequested(_) => {
            vec![attempt_effect(&ctx)]
        }
        EvaluationDatasetEvent::QuestionAccepted(e) => {
            let mut out = Vec::new();
            if e.paraphrase_of.is_none() && state.grammar_variants_enabled {
                out.push(paraphrase_effect(&ctx, e.sequence, e.category));
            }
            if should_attempt_more(state) {
                out.push(attempt_effect(&ctx));
            }
            out
        }
        EvaluationDatasetEvent::QuestionRejected(_) => {
            if should_attempt_more(state) {
                vec![attempt_effect(&ctx)]
            } else {
                Vec::new()
            }
        }
        EvaluationDatasetEvent::DatasetGenerationCompleted(_)
        | EvaluationDatasetEvent::DatasetGenerationFailed(_)
        | EvaluationDatasetEvent::DatasetRenamed(_)
        | EvaluationDatasetEvent::DatasetDeleted(_)
        | EvaluationDatasetEvent::DatasetGenerationCancelled(_) => Vec::new(),
    }
}

fn should_attempt_more(state: &EvaluationDataset) -> bool {
    let clean = state.clean_accepted_count();
    let target = state.target_question_count;
    let attempt_cap = state.max_attempts;
    clean < target && state.attempt_count < attempt_cap
}

fn attempt_effect(
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> NewJob<EvaluationDatasetEffect> {
    let stream_id = ctx.envelope.metadata.stream_id;
    let log_position = ctx.envelope.metadata.log_position;
    NewJob {
        partition_key: stream_id,
        job_type: ATTEMPT_EFFECT,
        idempotency_key: IdempotencyKey::new(stream_id, log_position, ATTEMPT_EFFECT),
        payload: EvaluationDatasetEffect::AttemptQuestionGeneration(GenerateQuestionEffect {
            dataset_id: stream_id,
        }),
    }
}

fn paraphrase_effect(
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
    clean_sequence: u32,
    category: crate::server::domain::evaluation::question::QuestionCategory,
) -> NewJob<EvaluationDatasetEffect> {
    let stream_id = ctx.envelope.metadata.stream_id;
    let log_position = ctx.envelope.metadata.log_position;
    let discriminator = format!("paraphrase:{clean_sequence}");
    NewJob {
        partition_key: stream_id,
        job_type: PARAPHRASE_EFFECT,
        idempotency_key: IdempotencyKey::new(stream_id, log_position, &discriminator),
        payload: EvaluationDatasetEffect::GenerateParaphrase(GenerateParaphraseEffect {
            dataset_id: stream_id,
            clean_sequence,
            category,
        }),
    }
}
