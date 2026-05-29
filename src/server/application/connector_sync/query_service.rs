use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::connector_sync::{
    ConnectorDiscoveredItemRepository, ConnectorSyncRepository, ConnectorSyncSummary,
};
use crate::server::domain::indexing::repository::IndexingRepository;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::source_document::source_ref::SourceRef;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Discovered,
    Imported,
    Indexed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDiscoveredItemView {
    pub connector_id: Uuid,
    pub source_ref_key: String,
    pub title: String,
    pub first_seen: String,
    pub last_seen: String,
    pub latest_sync_id: Uuid,
    pub imported_document_id: Option<Uuid>,
    pub status: ItemStatus,
    pub available: bool,
}

pub struct ConnectorSyncQueryService {
    sync_repository: Arc<dyn ConnectorSyncRepository>,
    discovered_repository: Arc<dyn ConnectorDiscoveredItemRepository>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    indexing_repository: Arc<dyn IndexingRepository>,
}

impl ConnectorSyncQueryService {
    pub fn new(
        sync_repository: Arc<dyn ConnectorSyncRepository>,
        discovered_repository: Arc<dyn ConnectorDiscoveredItemRepository>,
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        indexing_repository: Arc<dyn IndexingRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            sync_repository,
            discovered_repository,
            source_document_repository,
            indexing_repository,
        })
    }

    pub async fn list_syncs(
        &self,
        connector_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ConnectorSyncSummary>, AppError> {
        Ok(self
            .sync_repository
            .list_for_connector(connector_id, limit, offset)
            .await?)
    }

    pub async fn latest_sync(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorSyncSummary>, AppError> {
        Ok(self
            .sync_repository
            .latest_for_connector(connector_id)
            .await?)
    }

    pub async fn list_discovered(
        &self,
        connector_id: Uuid,
    ) -> Result<Vec<ConnectorDiscoveredItemView>, AppError> {
        let items = self
            .discovered_repository
            .list_for_connector(connector_id)
            .await?;

        let current_sync_id = self
            .sync_repository
            .latest_completed_for_connector(connector_id)
            .await?
            .map(|s| s.sync_id);

        let mut document_by_key: HashMap<String, Uuid> = HashMap::new();
        for item in &items {
            let Some(source_ref) = SourceRef::parse_route_key(&item.source_ref_key) else {
                continue;
            };
            if let Some(doc) = self
                .source_document_repository
                .find_by_source_ref(&source_ref)
                .await?
                .filter(|d| !d.deleted)
            {
                document_by_key.insert(item.source_ref_key.clone(), doc.document_id);
            }
        }

        let document_ids: Vec<Uuid> = document_by_key.values().copied().collect();
        let indexings = self
            .indexing_repository
            .list_for_documents(&document_ids)
            .await?;
        let mut indexed_docs: HashSet<Uuid> = HashSet::new();
        for ix in indexings {
            if ix.removed {
                continue;
            }
            if ix.status.is_indexed() {
                indexed_docs.insert(ix.document_id);
            }
        }

        Ok(items
            .into_iter()
            .map(|item| {
                let document_id = document_by_key.get(&item.source_ref_key).copied();
                let status = match document_id {
                    None => ItemStatus::Discovered,
                    Some(id) if indexed_docs.contains(&id) => ItemStatus::Indexed,
                    Some(_) => ItemStatus::Imported,
                };
                let available =
                    current_sync_id.is_none_or(|current| item.latest_sync_id == current);
                ConnectorDiscoveredItemView {
                    connector_id: item.connector_id,
                    source_ref_key: item.source_ref_key,
                    title: item.title,
                    first_seen: item.first_seen,
                    last_seen: item.last_seen,
                    latest_sync_id: item.latest_sync_id,
                    imported_document_id: document_id,
                    status,
                    available,
                }
            })
            .collect())
    }
}
