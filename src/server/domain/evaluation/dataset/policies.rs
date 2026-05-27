use event_sourcing::job_queue::{IdempotencyKey, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::{DatasetGenerationStatus, EvaluationDataset, MapDependencyStatus};
use super::effects::{
    EnsureMapBuildEffect, EvaluationDatasetEffect, GenerateQuestionEffect, PollMapStatusEffect,
};
use super::events::EvaluationDatasetEvent;

const ATTEMPT_EFFECT: &str = "attempt_question_generation";
const ENSURE_MAP_EFFECT: &str = "ensure_map_build";
const POLL_MAP_EFFECT: &str = "poll_map_status";

impl HasPolicies<EvaluationDataset, EvaluationDatasetEffect> for EvaluationDatasetEvent {
    fn policies() -> &'static [PolicyFn<Self, EvaluationDataset, EvaluationDatasetEffect>] {
        &[
            ensure_map_on_request,
            attempt_after_map_resolved,
            attempt_more_on_question_accepted,
            attempt_more_on_question_rejected,
        ]
    }
}

fn ensure_map_on_request(
    event: &EvaluationDatasetEvent,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    match event {
        EvaluationDatasetEvent::DatasetGenerationRequested(_) => {
            let stream_id = ctx.envelope.metadata.stream_id;
            let log_position = ctx.envelope.metadata.log_position;
            vec![NewJob {
                partition_key: stream_id,
                job_type: ENSURE_MAP_EFFECT,
                idempotency_key: IdempotencyKey::new(stream_id, log_position, ENSURE_MAP_EFFECT),
                payload: EvaluationDatasetEffect::EnsureMapBuild(EnsureMapBuildEffect {
                    dataset_id: stream_id,
                }),
            }]
        }
        _ => Vec::new(),
    }
}

fn attempt_after_map_resolved(
    event: &EvaluationDatasetEvent,
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    match event {
        EvaluationDatasetEvent::MapDependencyResolved(e) if e.ready => {
            if !matches!(ctx.state.status, DatasetGenerationStatus::Generating) {
                return Vec::new();
            }
            vec![attempt_effect(ctx)]
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
        && matches!(state.map_status, MapDependencyStatus::Ready)
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

#[allow(dead_code)]
fn poll_map_effect(
    ctx: &PolicyContext<'_, EvaluationDataset, EvaluationDatasetEvent>,
) -> NewJob<EvaluationDatasetEffect> {
    let stream_id = ctx.envelope.metadata.stream_id;
    let log_position = ctx.envelope.metadata.log_position;
    NewJob {
        partition_key: stream_id,
        job_type: POLL_MAP_EFFECT,
        idempotency_key: IdempotencyKey::new(stream_id, log_position, POLL_MAP_EFFECT),
        payload: EvaluationDatasetEffect::PollMapStatus(PollMapStatusEffect {
            dataset_id: stream_id,
        }),
    }
}
