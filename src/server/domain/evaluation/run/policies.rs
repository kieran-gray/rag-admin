use crate::event_sourcing::job_queue::{IdempotencyKey, NewJob};
use crate::event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::EvaluationRun;
use super::effects::{EvaluationRunEffect, ExecuteRunEffect, OptimizeRunEffect};
use super::events::EvaluationRunEvent;

impl HasPolicies<EvaluationRun, EvaluationRunEffect> for EvaluationRunEvent {
    fn policies() -> &'static [PolicyFn<Self, EvaluationRun, EvaluationRunEffect>] {
        &[dispatch_on_run_requested]
    }
}

fn dispatch_on_run_requested(
    event: &EvaluationRunEvent,
    ctx: &PolicyContext<'_, EvaluationRun, EvaluationRunEvent>,
) -> Vec<NewJob<EvaluationRunEffect>> {
    let log_position = ctx.envelope.metadata.log_position;
    let EvaluationRunEvent::RunRequested(event) = event else {
        return Vec::new();
    };
    match &event.optimization {
        Some(optimization) => vec![NewJob {
            partition_key: event.run_id,
            job_type: "optimize_run",
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
        None => vec![NewJob {
            partition_key: event.run_id,
            job_type: "execute_run",
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
    }
}
