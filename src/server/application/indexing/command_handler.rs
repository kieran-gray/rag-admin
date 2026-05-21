use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::indexing::{
    aggregate::Indexing,
    commands::{
        IndexingCommand, RequestIngest, RequeueChunking, RequeueEmbedding, RequeueIndexing,
    },
};
use crate::server::domain::shared::value_objects::ChunkingConfig as DomainChunkingConfig;
use crate::shared::ChunkingConfig;
use event_sourcing::CommandProcessor;

pub struct RequestIngestInput {
    pub document_id: Uuid,
    pub index_profile_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub auto_advance: bool,
}

pub struct IndexingCommandHandler {
    processor: Arc<CommandProcessor<Indexing>>,
    id_generator: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
}

impl IndexingCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<Indexing>>,
        id_generator: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            id_generator,
            clock,
        })
    }

    pub async fn request_ingest(&self, input: RequestIngestInput) -> Result<Uuid, AppError> {
        let chunking_domain: DomainChunkingConfig = input.chunking_config.into();
        let stream_id =
            Indexing::compute_id(input.document_id, input.index_profile_id, &chunking_domain);
        let command = IndexingCommand::RequestIngest(RequestIngest {
            document_id: input.document_id,
            index_profile_id: input.index_profile_id,
            document_version: input.document_version,
            chunking_config: chunking_domain,
            request_id: self.id_generator.new_uuid(),
            auto_advance: input.auto_advance,
            occurred_at: self.clock.now(),
        });
        self.processor.handle(stream_id, command).await?;
        Ok(stream_id)
    }

    pub async fn requeue_chunking(&self, indexing_id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            indexing_id,
            IndexingCommand::RequeueChunking(RequeueChunking {
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn requeue_embedding(&self, indexing_id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            indexing_id,
            IndexingCommand::RequeueEmbedding(RequeueEmbedding {
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn requeue_indexing(&self, indexing_id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            indexing_id,
            IndexingCommand::RequeueIndexing(RequeueIndexing {
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    async fn dispatch(&self, indexing_id: Uuid, command: IndexingCommand) -> Result<(), AppError> {
        self.processor.handle(indexing_id, command).await?;
        Ok(())
    }
}
