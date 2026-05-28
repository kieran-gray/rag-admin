use event_sourcing::job_queue::{IdempotencyKey, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::effects::{EvaluationDatasetEffect, NotifyMapReadyEffect};
use crate::server::domain::comprehension::map::aggregate::DocumentMap;
use crate::server::domain::comprehension::map::events::DocumentMapEvent;

const NOTIFY_MAP_READY: &str = "notify_map_ready";

impl HasPolicies<DocumentMap, EvaluationDatasetEffect> for DocumentMapEvent {
    fn policies() -> &'static [PolicyFn<Self, DocumentMap, EvaluationDatasetEffect>] {
        &[notify_awaiting_datasets]
    }
}

fn notify_awaiting_datasets(
    event: &DocumentMapEvent,
    ctx: &PolicyContext<'_, DocumentMap, DocumentMapEvent>,
) -> Vec<NewJob<EvaluationDatasetEffect>> {
    if !matches!(event, DocumentMapEvent::InsightsSynthesized(_)) {
        return Vec::new();
    }
    let map_id = ctx.state.map_id;
    let log_position = ctx.envelope.metadata.log_position;
    vec![NewJob {
        partition_key: map_id,
        job_type: NOTIFY_MAP_READY,
        idempotency_key: IdempotencyKey::new(map_id, log_position, NOTIFY_MAP_READY),
        payload: EvaluationDatasetEffect::NotifyMapReady(NotifyMapReadyEffect { map_id }),
    }]
}
