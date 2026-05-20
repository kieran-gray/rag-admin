use event_sourcing::job_queue::{IdempotencyKey, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::Indexing;
use super::effects::{
    ExecuteChunkingEffect, ExecuteEmbeddingEffect, ExecuteIndexingEffect, ExecuteRemovalEffect,
    IndexingEffect,
};
use super::events::IndexingEvent;

const CHUNKING: &str = "execute_chunking";
const EMBEDDING: &str = "execute_embedding";
const INDEXING: &str = "execute_indexing";
const REMOVAL: &str = "execute_removal";

impl HasPolicies<Indexing, IndexingEffect> for IndexingEvent {
    fn policies() -> &'static [PolicyFn<Self, Indexing, IndexingEffect>] {
        &[
            chunk_on_ingest_or_requeue,
            embed_on_embedding_requeued,
            index_on_indexing_requeued,
            embed_on_chunking_completed,
            index_on_embedding_completed,
            cleanup_on_indexing_removed,
        ]
    }
}

fn chunk_on_ingest_or_requeue(
    event: &IndexingEvent,
    ctx: &PolicyContext<'_, Indexing, IndexingEvent>,
) -> Vec<NewJob<IndexingEffect>> {
    match event {
        IndexingEvent::IngestRequested(_) | IndexingEvent::ChunkingRequeued(_) => {
            vec![chunking_effect(ctx)]
        }
        _ => Vec::new(),
    }
}

fn embed_on_embedding_requeued(
    event: &IndexingEvent,
    ctx: &PolicyContext<'_, Indexing, IndexingEvent>,
) -> Vec<NewJob<IndexingEffect>> {
    match event {
        IndexingEvent::EmbeddingRequeued(_) => vec![embedding_effect(ctx)],
        _ => Vec::new(),
    }
}

fn index_on_indexing_requeued(
    event: &IndexingEvent,
    ctx: &PolicyContext<'_, Indexing, IndexingEvent>,
) -> Vec<NewJob<IndexingEffect>> {
    match event {
        IndexingEvent::IndexingRequeued(_) => vec![indexing_effect(ctx)],
        _ => Vec::new(),
    }
}

fn embed_on_chunking_completed(
    event: &IndexingEvent,
    ctx: &PolicyContext<'_, Indexing, IndexingEvent>,
) -> Vec<NewJob<IndexingEffect>> {
    match event {
        IndexingEvent::ChunkingCompleted(_) if ctx.state.auto_advance => {
            vec![embedding_effect(ctx)]
        }
        _ => Vec::new(),
    }
}

fn index_on_embedding_completed(
    event: &IndexingEvent,
    ctx: &PolicyContext<'_, Indexing, IndexingEvent>,
) -> Vec<NewJob<IndexingEffect>> {
    match event {
        IndexingEvent::EmbeddingCompleted(_) if ctx.state.auto_advance => {
            vec![indexing_effect(ctx)]
        }
        _ => Vec::new(),
    }
}

fn chunking_effect(ctx: &PolicyContext<'_, Indexing, IndexingEvent>) -> NewJob<IndexingEffect> {
    let indexing_id = ctx.state.indexing_id;
    let log_position = ctx.envelope.metadata.log_position;
    NewJob {
        partition_key: indexing_id,
        job_type: CHUNKING,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, CHUNKING),
        payload: IndexingEffect::ExecuteChunking(ExecuteChunkingEffect { indexing_id }),
    }
}

fn embedding_effect(ctx: &PolicyContext<'_, Indexing, IndexingEvent>) -> NewJob<IndexingEffect> {
    let indexing_id = ctx.state.indexing_id;
    let log_position = ctx.envelope.metadata.log_position;
    NewJob {
        partition_key: indexing_id,
        job_type: EMBEDDING,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, EMBEDDING),
        payload: IndexingEffect::ExecuteEmbedding(ExecuteEmbeddingEffect { indexing_id }),
    }
}

fn indexing_effect(ctx: &PolicyContext<'_, Indexing, IndexingEvent>) -> NewJob<IndexingEffect> {
    let indexing_id = ctx.state.indexing_id;
    let log_position = ctx.envelope.metadata.log_position;
    NewJob {
        partition_key: indexing_id,
        job_type: INDEXING,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, INDEXING),
        payload: IndexingEffect::ExecuteIndexing(ExecuteIndexingEffect { indexing_id }),
    }
}

fn cleanup_on_indexing_removed(
    event: &IndexingEvent,
    ctx: &PolicyContext<'_, Indexing, IndexingEvent>,
) -> Vec<NewJob<IndexingEffect>> {
    match event {
        IndexingEvent::IndexingRemoved(_) => vec![removal_effect(ctx)],
        _ => Vec::new(),
    }
}

fn removal_effect(ctx: &PolicyContext<'_, Indexing, IndexingEvent>) -> NewJob<IndexingEffect> {
    let indexing_id = ctx.state.indexing_id;
    let log_position = ctx.envelope.metadata.log_position;
    NewJob {
        partition_key: indexing_id,
        job_type: REMOVAL,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, REMOVAL),
        payload: IndexingEffect::ExecuteRemoval(ExecuteRemovalEffect { indexing_id }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use event_sourcing::envelope::{EventEnvelope, EventMetadata};
    use uuid::Uuid;

    use crate::server::domain::indexing::events::{
        ChunkingCompleted, ChunkingRequeued, EmbeddingCompleted, EmbeddingRequeued,
        IndexingCompleted, IndexingRemoved, IndexingRequeued, IngestRequested, IngestionFailed,
        IngestionRetried,
    };
    use crate::server::domain::indexing::status::{IndexingStatus, IngestStage};
    use crate::server::domain::shared::event_payloads::{
        ChunkingConfigPayload, SectionChunkingConfigPayload,
    };

    fn indexing(auto_advance: bool) -> Indexing {
        Indexing {
            indexing_id: Uuid::new_v4(),
            chunk_set_id: None,
            embedding_set_id: None,
            status: IndexingStatus::Pending,
            last_request_id: None,
            removed: false,
            auto_advance,
        }
    }

    fn envelope(stream_id: Uuid, log_position: i64) -> EventEnvelope<IndexingEvent> {
        EventEnvelope {
            event: IndexingEvent::ChunkingRequeued(ChunkingRequeued {
                occurred_at: "2024-01-01T00:00:00Z".into(),
            }),
            metadata: EventMetadata {
                stream_id,
                aggregate_type: "indexing".into(),
                sequence: 1,
                log_position,
                event_type: "ChunkingRequeued".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    fn ingest_requested_event() -> IndexingEvent {
        IndexingEvent::IngestRequested(IngestRequested {
            document_id: Uuid::new_v4(),
            pipeline_configuration_id: Uuid::new_v4(),
            document_version: 1,
            chunking_config: ChunkingConfigPayload::Section(SectionChunkingConfigPayload {
                max_section_tokens: 512,
            }),
            request_id: Uuid::new_v4(),
            auto_advance: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        })
    }

    #[test]
    fn ingest_requested_enqueues_chunking_effect() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 7);
        let ctx = PolicyContext::new(&env, &state);

        let jobs = chunk_on_ingest_or_requeue(&ingest_requested_event(), &ctx);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, "execute_chunking");
        assert_eq!(jobs[0].partition_key, state.indexing_id);
        assert!(matches!(
            jobs[0].payload,
            IndexingEffect::ExecuteChunking(_)
        ));
    }

    #[test]
    fn chunking_requeued_also_enqueues_chunking_effect() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 10);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::ChunkingRequeued(ChunkingRequeued {
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = chunk_on_ingest_or_requeue(&event, &ctx);
        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn chunk_policy_ignores_other_events() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 10);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::IndexingRemoved(IndexingRemoved {
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        assert!(chunk_on_ingest_or_requeue(&event, &ctx).is_empty());
    }

    #[test]
    fn embedding_requeued_enqueues_embedding_effect() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 11);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::EmbeddingRequeued(EmbeddingRequeued {
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = embed_on_embedding_requeued(&event, &ctx);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, "execute_embedding");
        assert!(matches!(
            jobs[0].payload,
            IndexingEffect::ExecuteEmbedding(_)
        ));
    }

    #[test]
    fn indexing_requeued_enqueues_indexing_effect() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 12);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::IndexingRequeued(IndexingRequeued {
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = index_on_indexing_requeued(&event, &ctx);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, "execute_indexing");
    }

    #[test]
    fn chunking_completed_advances_when_auto_advance_on() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 13);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id: Uuid::new_v4(),
            chunk_count: 5,
            auto_advance: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = embed_on_chunking_completed(&event, &ctx);
        assert_eq!(jobs.len(), 1);
        assert!(matches!(
            jobs[0].payload,
            IndexingEffect::ExecuteEmbedding(_)
        ));
    }

    #[test]
    fn chunking_completed_does_not_advance_when_auto_advance_off() {
        let state = indexing(false);
        let env = envelope(state.indexing_id, 14);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::ChunkingCompleted(ChunkingCompleted {
            chunk_set_id: Uuid::new_v4(),
            chunk_count: 5,
            auto_advance: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        assert!(embed_on_chunking_completed(&event, &ctx).is_empty());
    }

    #[test]
    fn embedding_completed_advances_when_auto_advance_on() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 15);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::EmbeddingCompleted(EmbeddingCompleted {
            embedding_set_id: Uuid::new_v4(),
            auto_advance: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = index_on_embedding_completed(&event, &ctx);
        assert_eq!(jobs.len(), 1);
        assert!(matches!(
            jobs[0].payload,
            IndexingEffect::ExecuteIndexing(_)
        ));
    }

    #[test]
    fn embedding_completed_does_not_advance_when_auto_advance_off() {
        let state = indexing(false);
        let env = envelope(state.indexing_id, 16);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::EmbeddingCompleted(EmbeddingCompleted {
            embedding_set_id: Uuid::new_v4(),
            auto_advance: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        assert!(index_on_embedding_completed(&event, &ctx).is_empty());
    }

    #[test]
    fn indexing_removed_enqueues_removal_effect() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 17);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::IndexingRemoved(IndexingRemoved {
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = cleanup_on_indexing_removed(&event, &ctx);
        assert_eq!(jobs.len(), 1);
        assert!(matches!(jobs[0].payload, IndexingEffect::ExecuteRemoval(_)));
    }

    #[test]
    fn unrelated_event_emits_nothing_from_any_policy() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 18);
        let ctx = PolicyContext::new(&env, &state);

        let event = IndexingEvent::IngestionFailed(IngestionFailed {
            stage: IngestStage::Embedding,
            reason: "x".into(),
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });

        assert!(chunk_on_ingest_or_requeue(&event, &ctx).is_empty());
        assert!(embed_on_embedding_requeued(&event, &ctx).is_empty());
        assert!(index_on_indexing_requeued(&event, &ctx).is_empty());
        assert!(embed_on_chunking_completed(&event, &ctx).is_empty());
        assert!(index_on_embedding_completed(&event, &ctx).is_empty());
        assert!(cleanup_on_indexing_removed(&event, &ctx).is_empty());
    }

    #[test]
    fn idempotency_key_uses_log_position_for_uniqueness() {
        let state = indexing(true);
        let env_a = envelope(state.indexing_id, 1);
        let env_b = envelope(state.indexing_id, 2);
        let ctx_a = PolicyContext::new(&env_a, &state);
        let ctx_b = PolicyContext::new(&env_b, &state);

        let jobs_a = chunk_on_ingest_or_requeue(&ingest_requested_event(), &ctx_a);
        let jobs_b = chunk_on_ingest_or_requeue(&ingest_requested_event(), &ctx_b);

        assert_ne!(
            jobs_a[0].idempotency_key.as_str(),
            jobs_b[0].idempotency_key.as_str()
        );
    }

    #[test]
    fn full_policy_chain_runs_all_six_when_invoked_via_has_policies() {
        let state = indexing(true);
        let env = envelope(state.indexing_id, 99);
        let ctx = PolicyContext::new(&env, &state);

        let removed = IndexingEvent::IndexingRemoved(IndexingRemoved {
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = removed.apply_policies(&ctx);

        assert_eq!(jobs.len(), 1);
        assert!(matches!(jobs[0].payload, IndexingEffect::ExecuteRemoval(_)));

        let retried = IndexingEvent::IngestionRetried(IngestionRetried {
            request_id: Uuid::new_v4(),
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        assert!(retried.apply_policies(&ctx).is_empty());

        let completed = IndexingEvent::IndexingCompleted(IndexingCompleted {
            vector_count: 10,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        assert!(completed.apply_policies(&ctx).is_empty());
    }
}
