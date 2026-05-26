use std::sync::Arc;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::chunk_set::read_model::ChunkSetReadModel;
use crate::server::domain::chunk_set::repository::{
    ChunkSetListCursor, ChunkSetListQuery, ChunkSetRepository, ChunkSetStatusFilter,
};
use crate::shared::contracts::{
    ChunkSetListPageDto, ChunkSetListQueryDto, ChunkSetStatusFacetDto, ChunkSetStatusFilterDto,
    ChunkSetSummaryDto,
};

pub struct ChunkSetQueryService {
    repository: Arc<dyn ChunkSetRepository>,
}

impl ChunkSetQueryService {
    pub fn new(repository: Arc<dyn ChunkSetRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSetSummaryDto>, AppError> {
        let rows = self.repository.list_for_document(document_id).await?;
        Ok(rows.into_iter().map(read_model_to_dto).collect())
    }

    pub async fn list_page(
        &self,
        query: ChunkSetListQueryDto,
    ) -> Result<ChunkSetListPageDto, AppError> {
        let cursor = match query.cursor.as_deref() {
            Some(raw) => Some(parse_cursor(raw)?),
            None => None,
        };
        let statuses = query
            .statuses
            .into_iter()
            .map(filter_dto_to_domain)
            .collect();
        let domain_query = ChunkSetListQuery {
            cursor,
            limit: query.limit,
            statuses,
        };
        let page = self.repository.list_page(&domain_query).await?;

        Ok(ChunkSetListPageDto {
            items: page.items.into_iter().map(read_model_to_dto).collect(),
            next_cursor: page.next_cursor.map(|c| encode_cursor(&c)),
            total_matching: page.total_matching,
            total_all: page.total_all,
            status_counts: page
                .status_counts
                .into_iter()
                .map(|(status, count)| ChunkSetStatusFacetDto {
                    status: filter_domain_to_dto(status),
                    count,
                })
                .collect(),
        })
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

fn filter_dto_to_domain(status: ChunkSetStatusFilterDto) -> ChunkSetStatusFilter {
    match status {
        ChunkSetStatusFilterDto::Pinned => ChunkSetStatusFilter::Pinned,
        ChunkSetStatusFilterDto::Indexed => ChunkSetStatusFilter::Indexed,
        ChunkSetStatusFilterDto::UsedByEval => ChunkSetStatusFilter::UsedByEval,
        ChunkSetStatusFilterDto::Unused => ChunkSetStatusFilter::Unused,
    }
}

fn filter_domain_to_dto(status: ChunkSetStatusFilter) -> ChunkSetStatusFilterDto {
    match status {
        ChunkSetStatusFilter::Pinned => ChunkSetStatusFilterDto::Pinned,
        ChunkSetStatusFilter::Indexed => ChunkSetStatusFilterDto::Indexed,
        ChunkSetStatusFilter::UsedByEval => ChunkSetStatusFilterDto::UsedByEval,
        ChunkSetStatusFilter::Unused => ChunkSetStatusFilterDto::Unused,
    }
}

fn encode_cursor(cursor: &ChunkSetListCursor) -> String {
    let created_at = cursor
        .created_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::new());
    format!("{}|{}", created_at, cursor.chunk_set_id)
}

fn parse_cursor(raw: &str) -> Result<ChunkSetListCursor, AppError> {
    let (created_at, id) = raw
        .split_once('|')
        .ok_or_else(|| AppError::Validation(format!("invalid chunk-set cursor: {raw}")))?;
    let chunk_set_id = Uuid::parse_str(id)
        .map_err(|e| AppError::Validation(format!("invalid chunk-set cursor uuid: {e}")))?;
    let created_at = OffsetDateTime::parse(created_at, &Rfc3339)
        .map_err(|e| AppError::Validation(format!("invalid chunk-set cursor timestamp: {e}")))?;
    Ok(ChunkSetListCursor {
        created_at,
        chunk_set_id,
    })
}
