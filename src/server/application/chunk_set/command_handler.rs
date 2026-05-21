use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::chunk_set::repository::{ChunkSetRepository, ChunkSetRepositoryError};

pub struct ChunkSetCommandHandler {
    repository: Arc<dyn ChunkSetRepository>,
}

impl ChunkSetCommandHandler {
    pub fn new(repository: Arc<dyn ChunkSetRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn set_pinned(&self, chunk_set_id: Uuid, pinned: bool) -> Result<(), AppError> {
        self.repository.set_pinned(chunk_set_id, pinned).await?;
        Ok(())
    }

    pub async fn delete(&self, chunk_set_id: Uuid) -> Result<(), AppError> {
        match self.repository.delete(chunk_set_id).await {
            Ok(()) => Ok(()),
            Err(ChunkSetRepositoryError::InUse(_)) => Err(AppError::Validation(format!(
                "chunk set {chunk_set_id} is in use (pinned or referenced by an indexing / evaluation run) — cannot delete",
            ))),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn gc_unused(&self, older_than_seconds: u64) -> Result<u64, AppError> {
        Ok(self.repository.delete_unused(older_than_seconds).await?)
    }
}
