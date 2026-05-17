use crate::event_sourcing::job_queue::{IdempotencyKey, NewJob};
use crate::event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::Indexing;
use super::effects::{
    ExecuteChunkingEffect, ExecuteEmbeddingEffect, ExecuteIndexingEffect, IndexingEffect,
};
use super::events::IndexingEvent;

const CHUNKING: &str = "execute_chunking";
const EMBEDDING: &str = "execute_embedding";
const INDEXING: &str = "execute_indexing";

impl HasPolicies<Indexing, IndexingEffect> for IndexingEvent {
    fn policies() -> &'static [PolicyFn<Self, Indexing, IndexingEffect>] {
        &[
            chunk_on_ingest_or_requeue,
            embed_on_embedding_requeued,
            index_on_indexing_requeued,
            embed_on_chunking_completed,
            index_on_embedding_completed,
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
