use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::chunk_set::read_model::ChunkSetReadModel;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::shared::contracts::ChunkSetSummaryDto;

pub struct ChunkSetQueryService {
    repository: Arc<dyn ChunkSetRepository>,
}

impl ChunkSetQueryService {
    pub fn new(repository: Arc<dyn ChunkSetRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn list_all(&self) -> Result<Vec<ChunkSetSummaryDto>, AppError> {
        let rows = self.repository.list_all_with_referrers().await?;
        Ok(rows.into_iter().map(read_model_to_dto).collect())
    }

    pub async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSetSummaryDto>, AppError> {
        let rows = self
            .repository
            .list_for_document_with_referrers(document_id)
            .await?;
        Ok(rows.into_iter().map(read_model_to_dto).collect())
    }
}

fn read_model_to_dto(rm: ChunkSetReadModel) -> ChunkSetSummaryDto {
    ChunkSetSummaryDto {
        chunk_set_id: rm.chunk_set_id,
        document_id: rm.document_id,
        document_version: rm.document_version,
        chunking_config: rm.chunking_config.into(),
        created_at: rm.created_at,
        pinned: rm.pinned,
        chunk_count: rm.chunk_count,
        indexing_refs: rm.indexing_refs,
        variant_result_refs: rm.variant_result_refs,
    }
}
