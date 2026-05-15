use crate::server::event_sourcing::effect::{IdempotencyKey, PendingEffect};
use crate::server::event_sourcing::envelope::EventEnvelope;

use super::aggregate::EvaluationRun;
use super::events::EvaluationRunEvent;

use crate::server::application::evaluation::effects::run::{
    EvaluationRunEffect, ExecuteRunEffect, OptimizeRunEffect,
};

pub fn derive_run_effects(
    envelope: &EventEnvelope<EvaluationRunEvent>,
    _state: &EvaluationRun,
) -> Vec<PendingEffect<EvaluationRunEffect>> {
    let log_position = envelope.metadata.log_position;
    match &envelope.event {
        EvaluationRunEvent::RunRequested(event) => match &event.optimization {
            Some(optimization) => vec![PendingEffect {
                stream_id: event.run_id,
                event_log_position: log_position,
                effect_type: "optimize_run",
                idempotency_key: IdempotencyKey::new(event.run_id, log_position, "optimize_run"),
                payload: EvaluationRunEffect::OptimizeRun(OptimizeRunEffect {
                    run_id: event.run_id,
                    dataset_id: event.dataset_id,
                    pipeline_configuration_id: event.pipeline_configuration_id,
                    document_id: event.document_id,
                    document_version: event.document_version,
                    optimization: optimization.clone(),
                    scoring_policy: event.scoring_policy,
                }),
            }],
            None => vec![PendingEffect {
                stream_id: event.run_id,
                event_log_position: log_position,
                effect_type: "execute_run",
                idempotency_key: IdempotencyKey::new(event.run_id, log_position, "execute_run"),
                payload: EvaluationRunEffect::ExecuteRun(ExecuteRunEffect {
                    run_id: event.run_id,
                    dataset_id: event.dataset_id,
                    pipeline_configuration_id: event.pipeline_configuration_id,
                    document_id: event.document_id,
                    document_version: event.document_version,
                    variants: event.variants.clone(),
                    options: event.options.clone(),
                    autotune_request: event.autotune_request.clone(),
                    scoring_policy: event.scoring_policy,
                }),
            }],
        },
        EvaluationRunEvent::VariantPrepared(_)
        | EvaluationRunEvent::VariantScored(_)
        | EvaluationRunEvent::TrialProposed(_)
        | EvaluationRunEvent::RungAdvanced(_)
        | EvaluationRunEvent::ChampionSelected(_)
        | EvaluationRunEvent::RunCompleted(_)
        | EvaluationRunEvent::RunFailed(_) => Vec::new(),
    }
}
