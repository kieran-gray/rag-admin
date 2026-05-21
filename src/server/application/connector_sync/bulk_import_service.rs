use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::application::configuration::{
    ChunkingConfigurationQueryService, IndexProfileResolver,
};
use crate::server::application::source_document::SourceDocumentIngestService;
use crate::server::application::AppError;
use crate::server::domain::source_document::source_ref::SourceRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportFailure {
    pub source_ref_key: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub imported: u32,
    pub indexed: u32,
    pub failures: Vec<BulkImportFailure>,
}

pub struct BulkImportService {
    ingest_service: Arc<SourceDocumentIngestService>,
    index_profile_resolver: Arc<IndexProfileResolver>,
    chunking_query_service: Arc<ChunkingConfigurationQueryService>,
}

impl BulkImportService {
    pub fn new(
        ingest_service: Arc<SourceDocumentIngestService>,
        index_profile_resolver: Arc<IndexProfileResolver>,
        chunking_query_service: Arc<ChunkingConfigurationQueryService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            ingest_service,
            index_profile_resolver,
            chunking_query_service,
        })
    }

    pub async fn bulk_import(
        &self,
        connector_id: Uuid,
        source_ref_keys: Vec<String>,
        index_after_import: bool,
    ) -> Result<BulkImportResult, AppError> {
        let resolved_index_profile = if index_after_import {
            Some(
                self.index_profile_resolver
                    .resolve_for_connector(connector_id)
                    .await?,
            )
        } else {
            None
        };
        let chunking_config = if index_after_import {
            Some(
                self.chunking_query_service
                    .resolve_for_connector(connector_id)
                    .await?,
            )
        } else {
            None
        };

        let mut imported = 0u32;
        let mut indexed = 0u32;
        let mut failures: Vec<BulkImportFailure> = Vec::new();

        for key in source_ref_keys {
            let Some(source_ref) = SourceRef::parse_route_key(&key) else {
                failures.push(BulkImportFailure {
                    source_ref_key: key,
                    error: "invalid source ref key".into(),
                });
                continue;
            };

            let dto = match self
                .ingest_service
                .import_from_connector(connector_id, source_ref.clone())
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    failures.push(BulkImportFailure {
                        source_ref_key: key,
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            imported += 1;

            if let (Some(profile), Some(chunking)) = (&resolved_index_profile, &chunking_config) {
                match self
                    .ingest_service
                    .request_indexing(source_ref, profile.index_profile_id, *chunking, true)
                    .await
                {
                    Ok(_) => indexed += 1,
                    Err(e) => failures.push(BulkImportFailure {
                        source_ref_key: dto.source_ref_key,
                        error: format!("indexing: {e}"),
                    }),
                }
            }
        }

        Ok(BulkImportResult {
            imported,
            indexed,
            failures,
        })
    }
}
