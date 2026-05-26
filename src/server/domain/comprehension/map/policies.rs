use event_sourcing::job_queue::{IdempotencyKey, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::{DocumentMap, MapStatus};
use super::effects::{
    DocumentMapEffect, ExtractObservationsEffect, SuggestRolesEffect, SynthesizeInsightsEffect,
    SynthesizeThreadsEffect,
};
use super::events::DocumentMapEvent;

const SUGGEST_ROLES: &str = "suggest_roles";
const EXTRACT_OBSERVATIONS: &str = "extract_observations";
const SYNTHESIZE_THREADS: &str = "synthesize_threads";
const SYNTHESIZE_INSIGHTS: &str = "synthesize_insights";

impl HasPolicies<DocumentMap, DocumentMapEffect> for DocumentMapEvent {
    fn policies() -> &'static [PolicyFn<Self, DocumentMap, DocumentMapEffect>] {
        &[
            suggest_roles_on_request,
            extract_observations_on_roles,
            advance_after_observations,
            advance_after_threads,
        ]
    }
}

fn suggest_roles_on_request(
    event: &DocumentMapEvent,
    ctx: &PolicyContext<'_, DocumentMap, DocumentMapEvent>,
) -> Vec<NewJob<DocumentMapEffect>> {
    match event {
        DocumentMapEvent::MapBuildRequested(_) => {
            let map_id = ctx.state.map_id;
            let log_position = ctx.envelope.metadata.log_position;
            vec![NewJob {
                partition_key: map_id,
                job_type: SUGGEST_ROLES,
                idempotency_key: IdempotencyKey::new(map_id, log_position, SUGGEST_ROLES),
                payload: DocumentMapEffect::SuggestRoles(SuggestRolesEffect { map_id }),
            }]
        }
        _ => Vec::new(),
    }
}

fn extract_observations_on_roles(
    event: &DocumentMapEvent,
    ctx: &PolicyContext<'_, DocumentMap, DocumentMapEvent>,
) -> Vec<NewJob<DocumentMapEffect>> {
    match event {
        DocumentMapEvent::RolesSuggested(_) => {
            let map_id = ctx.state.map_id;
            let log_position = ctx.envelope.metadata.log_position;
            if ctx.state.chunk_count == 0 {
                return vec![synthesize_insights_job(map_id, log_position)];
            }
            (0..ctx.state.chunk_count)
                .map(|chunk_sequence| {
                    let token = format!("{EXTRACT_OBSERVATIONS}:{chunk_sequence}");
                    NewJob {
                        partition_key: map_id,
                        job_type: EXTRACT_OBSERVATIONS,
                        idempotency_key: IdempotencyKey::new(map_id, log_position, &token),
                        payload: DocumentMapEffect::ExtractObservations(
                            ExtractObservationsEffect {
                                map_id,
                                chunk_sequence,
                            },
                        ),
                    }
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn advance_after_observations(
    event: &DocumentMapEvent,
    ctx: &PolicyContext<'_, DocumentMap, DocumentMapEvent>,
) -> Vec<NewJob<DocumentMapEffect>> {
    if !matches!(
        event,
        DocumentMapEvent::ObservationsExtracted(_) | DocumentMapEvent::ChunkExtractionFailed(_)
    ) {
        return Vec::new();
    }
    let map_id = ctx.state.map_id;
    let log_position = ctx.envelope.metadata.log_position;
    match &ctx.state.status {
        MapStatus::SynthesizingThreads { .. } => {
            let sections = ctx.state.section_count();
            (0..sections)
                .map(|section_sequence| {
                    let token = format!("{SYNTHESIZE_THREADS}:{section_sequence}");
                    NewJob {
                        partition_key: map_id,
                        job_type: SYNTHESIZE_THREADS,
                        idempotency_key: IdempotencyKey::new(map_id, log_position, &token),
                        payload: DocumentMapEffect::SynthesizeThreads(SynthesizeThreadsEffect {
                            map_id,
                            section_sequence,
                        }),
                    }
                })
                .collect()
        }
        MapStatus::SynthesizingInsights => vec![synthesize_insights_job(map_id, log_position)],
        _ => Vec::new(),
    }
}

fn advance_after_threads(
    event: &DocumentMapEvent,
    ctx: &PolicyContext<'_, DocumentMap, DocumentMapEvent>,
) -> Vec<NewJob<DocumentMapEffect>> {
    if !matches!(
        event,
        DocumentMapEvent::ThreadsSynthesized(_) | DocumentMapEvent::SectionSynthesisFailed(_)
    ) {
        return Vec::new();
    }
    let map_id = ctx.state.map_id;
    let log_position = ctx.envelope.metadata.log_position;
    if matches!(ctx.state.status, MapStatus::SynthesizingInsights) {
        return vec![synthesize_insights_job(map_id, log_position)];
    }
    Vec::new()
}

fn synthesize_insights_job(map_id: uuid::Uuid, log_position: i64) -> NewJob<DocumentMapEffect> {
    NewJob {
        partition_key: map_id,
        job_type: SYNTHESIZE_INSIGHTS,
        idempotency_key: IdempotencyKey::new(map_id, log_position, SYNTHESIZE_INSIGHTS),
        payload: DocumentMapEffect::SynthesizeInsights(SynthesizeInsightsEffect { map_id }),
    }
}
