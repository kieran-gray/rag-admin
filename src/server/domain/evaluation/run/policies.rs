use crate::server::domain::evaluation::value_objects::{ChunkingVariant, EvaluationResultSplit};
use event_sourcing::job_queue::{IdempotencyKey, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::EvaluationRun;
use super::effects::{
    EvaluationRunEffect, ExecuteVariantEffect, FinalizeRunEffect, OptimizeRunEffect,
};
use super::events::EvaluationRunEvent;

impl HasPolicies<EvaluationRun, EvaluationRunEffect> for EvaluationRunEvent {
    fn policies() -> &'static [PolicyFn<Self, EvaluationRun, EvaluationRunEffect>] {
        &[fanout_on_run_requested, finalize_when_primary_scoring_done]
    }
}

fn fanout_on_run_requested(
    event: &EvaluationRunEvent,
    ctx: &PolicyContext<'_, EvaluationRun, EvaluationRunEvent>,
) -> Vec<NewJob<EvaluationRunEffect>> {
    let log_position = ctx.envelope.metadata.log_position;
    let EvaluationRunEvent::RunRequested(event) = event else {
        return Vec::new();
    };

    if let Some(optimization) = &event.optimization {
        return vec![NewJob {
            partition_key: event.run_id,
            job_type: "optimize_run",
            idempotency_key: IdempotencyKey::new(event.run_id, log_position, "optimize_run"),
            payload: EvaluationRunEffect::OptimizeRun(OptimizeRunEffect {
                run_id: event.run_id,
                dataset_id: event.dataset_id,
                index_profile_id: event.index_profile_id,
                document_id: event.document_id,
                document_version: event.document_version,
                optimization: optimization.clone(),
                scoring_policy: event.scoring_policy,
            }),
        }];
    }

    let autotune = event.autotune_request.clone();

    event
        .variants
        .iter()
        .cloned()
        .map(|variant: ChunkingVariant| {
            let discriminator = format!("execute_variant:{}", variant.label);
            NewJob {
                partition_key: event.run_id,
                job_type: "execute_variant",
                idempotency_key: IdempotencyKey::new(event.run_id, log_position, &discriminator),
                payload: EvaluationRunEffect::ExecuteVariant(ExecuteVariantEffect {
                    run_id: event.run_id,
                    dataset_id: event.dataset_id,
                    index_profile_id: event.index_profile_id,
                    document_id: event.document_id,
                    document_version: event.document_version,
                    variant_label: variant.label,
                    variant_config: variant.config,
                    options: event.options.clone(),
                    autotune_request: autotune.clone(),
                    scoring_policy: event.scoring_policy,
                }),
            }
        })
        .collect()
}

fn finalize_when_primary_scoring_done(
    event: &EvaluationRunEvent,
    ctx: &PolicyContext<'_, EvaluationRun, EvaluationRunEvent>,
) -> Vec<NewJob<EvaluationRunEffect>> {
    let log_position = ctx.envelope.metadata.log_position;
    let EvaluationRunEvent::VariantScored(_) = event else {
        return Vec::new();
    };

    let run = ctx.state;

    if run.optimization.is_some() {
        return Vec::new();
    }

    let primary_split = if run.autotune_request.is_some() {
        EvaluationResultSplit::Tuning
    } else {
        EvaluationResultSplit::Full
    };

    let primary_count = run
        .scored_keys
        .iter()
        .filter(|k| k.split == primary_split)
        .count();
    let expected = run.variants.len() * run.options.len();
    if primary_count != expected {
        return Vec::new();
    }

    vec![NewJob {
        partition_key: run.run_id,
        job_type: "finalize_run",
        idempotency_key: IdempotencyKey::new(run.run_id, log_position, "finalize_run"),
        payload: EvaluationRunEffect::FinalizeRun(FinalizeRunEffect {
            run_id: run.run_id,
            dataset_id: run.dataset_id,
            index_profile_id: run.index_profile_id,
            autotune_request: run.autotune_request.clone(),
            scoring_policy: run.scoring_policy,
        }),
    }]
}
