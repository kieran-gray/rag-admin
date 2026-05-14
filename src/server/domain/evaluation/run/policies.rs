use crate::server::event_sourcing::effect::{IdempotencyKey, PendingEffect};
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::EvaluationRun;
use super::events::{EvaluationRunEvent, RunRequested};

use crate::server::application::evaluation::effects::run::{EvaluationRunEffect, ExecuteRunEffect};

fn for_run_requested(
    event: &RunRequested,
    ctx: &PolicyContext<'_, EvaluationRun, EvaluationRunEvent>,
) -> Vec<PendingEffect<EvaluationRunEffect>> {
    vec![PendingEffect {
        stream_id: event.run_id,
        event_log_position: ctx.envelope.metadata.log_position,
        effect_type: "execute_run",
        idempotency_key: IdempotencyKey::new(
            event.run_id,
            ctx.envelope.metadata.log_position,
            "execute_run",
        ),
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
    }]
}

impl HasPolicies<EvaluationRun, EvaluationRunEvent, EvaluationRunEffect> for RunRequested {
    fn policies(
    ) -> &'static [PolicyFn<Self, EvaluationRun, EvaluationRunEvent, EvaluationRunEffect>] {
        &[for_run_requested]
    }
}

pub fn derive_run_effects(
    envelope: &EventEnvelope<EvaluationRunEvent>,
    state: &EvaluationRun,
) -> Vec<PendingEffect<EvaluationRunEffect>> {
    let ctx = PolicyContext::new(envelope, state);
    match &envelope.event {
        EvaluationRunEvent::RunRequested(e) => e.apply_policies(&ctx),
        EvaluationRunEvent::VariantPrepared(_)
        | EvaluationRunEvent::VariantScored(_)
        | EvaluationRunEvent::RunCompleted(_)
        | EvaluationRunEvent::RunFailed(_) => Vec::new(),
    }
}
