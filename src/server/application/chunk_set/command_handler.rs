use std::sync::Arc;

use uuid::Uuid;

use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::error::CommandError;

use crate::server::application::ports::Clock;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::aggregate::ChunkSet;
use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::chunk_set::commands::{
    ChunkSetCommand, CreateChunkSet, DeleteChunkSet, PinChunkSet, UnpinChunkSet,
};
use crate::server::domain::chunk_set::events::ChunkSetEvent;
use crate::server::domain::chunk_set::exceptions::ChunkSetError;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::shared::value_objects::ChunkingConfig;

pub struct ChunkSetCommandHandler {
    processor: Arc<CommandProcessor<ChunkSet>>,
    repository: Arc<dyn ChunkSetRepository>,
    clock: Arc<dyn Clock>,
}

impl ChunkSetCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<ChunkSet>>,
        repository: Arc<dyn ChunkSetRepository>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            repository,
            clock,
        })
    }

    pub async fn create(
        &self,
        chunk_set_id: Uuid,
        document_id: Uuid,
        document_version: u32,
        chunking_config: ChunkingConfig,
        chunks: Vec<Chunk>,
    ) -> Result<(), AppError> {
        let cmd = ChunkSetCommand::CreateChunkSet(CreateChunkSet {
            chunk_set_id,
            document_id,
            document_version,
            chunking_config,
            chunks,
            occurred_at: self.clock.now(),
        });
        let appended = match self.processor.handle(chunk_set_id, cmd).await {
            Ok(envelopes) => envelopes,
            Err(CommandError::Aggregate(ChunkSetError::AlreadyExists)) => return Ok(()),
            Err(e) => return Err(map_command_error(e)),
        };
        for env in &appended {
            if let ChunkSetEvent::ChunkSetCreated(event) = &env.event {
                self.repository.project_created(event).await?;
            }
        }
        Ok(())
    }

    pub async fn set_pinned(&self, chunk_set_id: Uuid, pinned: bool) -> Result<(), AppError> {
        let occurred_at = self.clock.now();
        let cmd = if pinned {
            ChunkSetCommand::PinChunkSet(PinChunkSet {
                chunk_set_id,
                occurred_at,
            })
        } else {
            ChunkSetCommand::UnpinChunkSet(UnpinChunkSet {
                chunk_set_id,
                occurred_at,
            })
        };
        self.processor
            .handle(chunk_set_id, cmd)
            .await
            .map_err(map_command_error)?;
        Ok(())
    }

    pub async fn delete(&self, chunk_set_id: Uuid) -> Result<(), AppError> {
        let summary = self
            .repository
            .load_summary(chunk_set_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("chunk set {chunk_set_id}")))?;
        if summary.indexing_refs > 0 || summary.variant_result_refs > 0 {
            return Err(AppError::Validation(format!(
                "chunk set {chunk_set_id} is in use (referenced by an indexing or evaluation run) — cannot delete",
            )));
        }

        let cmd = ChunkSetCommand::DeleteChunkSet(DeleteChunkSet {
            chunk_set_id,
            occurred_at: self.clock.now(),
        });
        self.processor
            .handle(chunk_set_id, cmd)
            .await
            .map_err(map_command_error)?;
        Ok(())
    }

    pub async fn delete_unused(&self, older_than_seconds: u64) -> Result<u64, AppError> {
        let candidates = self
            .repository
            .list_unused_older_than(older_than_seconds)
            .await?;
        let mut deleted = 0u64;
        for id in candidates {
            if self.delete(id).await.is_ok() {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    pub async fn reconcile_referrer_counts(&self) -> Result<(), AppError> {
        self.repository.reconcile_referrer_counts().await?;
        Ok(())
    }
}

fn map_command_error(e: CommandError<ChunkSetError>) -> AppError {
    match e {
        CommandError::Aggregate(err) => err.into(),
        CommandError::EventStore(err) => err.into(),
    }
}
