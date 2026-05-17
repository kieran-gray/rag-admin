use crate::event_sourcing::job_queue::{IdempotencyKey, NewJob};
use crate::event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};
use crate::server::domain::evaluation::question::QuestionCategory;

use super::aggregate::{DatasetGenerationStatus, EvaluationDataset};
use super::effects::{EvaluationDatasetEffect, GenerateParaphraseEffect, GenerateQuestionEffect};
use super::events::EvaluationDatasetEvent;

const ATTEMPT_EFFECT: &str = "attempt_question_generation";
const PARAPHRASE_EFFECT: &str = "generate_paraphrase";

impl HasPolicies<EvaluationDataset, EvaluationDatasetEffect> for EvaluationDatasetEvent {
    fn policies() -> &'static [PolicyFn<Self, EvaluationDataset, EvaluationDatasetEffect>] {
        &[
            attempt_on_generation_requested,
            paraphrase_on_question_accepted,
            attempt_more_on_question_accepted,
            attempt_more_on_question_rejected,
        ]
    }
}

fn attempt_on_generation_requested(
    event: &EvaluationDatasetEvent,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    if !is_generating(ctx.state) {
        return Vec::new();
    }
    match event {
        EvaluationDatasetEvent::DatasetGenerationRequested(_) => vec![attempt_effect(ctx)],
        _ => Vec::new(),
    }
}

fn paraphrase_on_question_accepted(
    event: &EvaluationDatasetEvent,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    if !is_generating(ctx.state) {
        return Vec::new();
    }
    match event {
        EvaluationDatasetEvent::QuestionAccepted(e)
            if e.paraphrase_of.is_none() && ctx.state.grammar_variants_enabled =>
        {
            vec![paraphrase_effect(ctx, e.sequence, e.category)]
        }
        _ => Vec::new(),
    }
}

fn attempt_more_on_question_accepted(
    event: &EvaluationDatasetEvent,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    if !is_generating(ctx.state) {
        return Vec::new();
    }
    match event {
        EvaluationDatasetEvent::QuestionAccepted(_) if should_attempt_more(ctx.state) => {
            vec![attempt_effect(ctx)]
        }
        _ => Vec::new(),
    }
}

fn attempt_more_on_question_rejected(
    event: &EvaluationDatasetEvent,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    if !is_generating(ctx.state) {
        return Vec::new();
    }
    match event {
        EvaluationDatasetEvent::QuestionRejected(_) if should_attempt_more(ctx.state) => {
            vec![attempt_effect(ctx)]
        }
        _ => Vec::new(),
    }
}

fn is_generating(state: &EvaluationDataset) -> bool {
    matches!(state.status, DatasetGenerationStatus::Generating)
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
    category: QuestionCategory,
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
