use crate::server::application::indexing::effects::{
    ExecuteChunkingEffect, ExecuteEmbeddingEffect, ExecuteIndexingEffect, IndexingEffect,
};
use crate::server::event_sourcing::envelope::EventEnvelope;
use crate::server::event_sourcing::job_queue::{IdempotencyKey, NewJob};

use super::aggregate::Indexing;
use super::events::IndexingEvent;

const CHUNKING: &str = "execute_chunking";
const EMBEDDING: &str = "execute_embedding";
const INDEXING: &str = "execute_indexing";

pub fn derive_indexing_effects(
    envelope: &EventEnvelope<IndexingEvent>,
    state: &Indexing,
) -> Vec<NewJob<IndexingEffect>> {
    let indexing_id = state.indexing_id;
    let log_position = envelope.metadata.log_position;
    match &envelope.event {
        IndexingEvent::IngestRequested(_) | IndexingEvent::ChunkingRequeued(_) => {
            vec![chunking_effect(indexing_id, log_position)]
        }
        IndexingEvent::EmbeddingRequeued(_) => {
            vec![embedding_effect(indexing_id, log_position)]
        }
        IndexingEvent::IndexingRequeued(_) => {
            vec![indexing_effect(indexing_id, log_position)]
        }
        IndexingEvent::ChunkingCompleted(_) if state.auto_advance => {
            vec![embedding_effect(indexing_id, log_position)]
        }
        IndexingEvent::EmbeddingCompleted(_) if state.auto_advance => {
            vec![indexing_effect(indexing_id, log_position)]
        }
        IndexingEvent::ChunkingCompleted(_)
        | IndexingEvent::EmbeddingCompleted(_)
        | IndexingEvent::IndexingCompleted(_)
        | IndexingEvent::IngestionFailed(_)
        | IndexingEvent::IngestionRetried(_)
        | IndexingEvent::IndexingRemoved(_) => Vec::new(),
    }
}

fn chunking_effect(indexing_id: uuid::Uuid, log_position: i64) -> NewJob<IndexingEffect> {
    NewJob {
        partition_key: indexing_id,
        job_type: CHUNKING,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, CHUNKING),
        payload: IndexingEffect::ExecuteChunking(ExecuteChunkingEffect { indexing_id }),
    }
}

fn embedding_effect(indexing_id: uuid::Uuid, log_position: i64) -> NewJob<IndexingEffect> {
    NewJob {
        partition_key: indexing_id,
        job_type: EMBEDDING,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, EMBEDDING),
        payload: IndexingEffect::ExecuteEmbedding(ExecuteEmbeddingEffect { indexing_id }),
    }
}

fn indexing_effect(indexing_id: uuid::Uuid, log_position: i64) -> NewJob<IndexingEffect> {
    NewJob {
        partition_key: indexing_id,
        job_type: INDEXING,
        idempotency_key: IdempotencyKey::new(indexing_id, log_position, INDEXING),
        payload: IndexingEffect::ExecuteIndexing(ExecuteIndexingEffect { indexing_id }),
    }
}
