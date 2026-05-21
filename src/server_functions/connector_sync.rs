use leptos::prelude::*;

use crate::shared::contracts::{
    BulkImportResultDto, ConnectorDiscoveredItemViewDto, ConnectorSyncSummaryDto,
};

#[cfg(feature = "ssr")]
use crate::server::application::connector_sync::ItemStatus;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::connector::ConnectorServices;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::ingestion::IngestionServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use crate::shared::contracts::{BulkImportFailureDto, ConnectorItemStatusDto};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(
    name = RunConnectorSync,
    prefix = "/api",
    endpoint = "run_connector_sync"
)]
pub async fn run_connector_sync(connector_id: uuid::Uuid) -> Result<uuid::Uuid, ServerFnError> {
    ctx::<Arc<ConnectorServices>>()?
        .connector_sync_service
        .run_sync(connector_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ListConnectorSyncs,
    prefix = "/api",
    endpoint = "list_connector_syncs"
)]
pub async fn list_connector_syncs(
    connector_id: uuid::Uuid,
    limit: u32,
    offset: u32,
) -> Result<Vec<ConnectorSyncSummaryDto>, ServerFnError> {
    let summaries = ctx::<Arc<ConnectorServices>>()?
        .connector_sync_query_service
        .list_syncs(connector_id, limit, offset)
        .await
        .map_err(|e| map_app_error(&e))?;
    Ok(summaries
        .into_iter()
        .map(|s| ConnectorSyncSummaryDto {
            sync_id: s.sync_id,
            connector_id: s.connector_id,
            status: s.status,
            discovered_count: s.discovered_count,
            started_at: s.started_at,
            completed_at: s.completed_at,
            error: s.error,
        })
        .collect())
}

#[server(
    name = ListConnectorDiscovered,
    prefix = "/api",
    endpoint = "list_connector_discovered"
)]
pub async fn list_connector_discovered(
    connector_id: uuid::Uuid,
) -> Result<Vec<ConnectorDiscoveredItemViewDto>, ServerFnError> {
    let items = ctx::<Arc<ConnectorServices>>()?
        .connector_sync_query_service
        .list_discovered(connector_id)
        .await
        .map_err(|e| map_app_error(&e))?;
    Ok(items
        .into_iter()
        .map(|i| ConnectorDiscoveredItemViewDto {
            connector_id: i.connector_id,
            source_ref_key: i.source_ref_key,
            title: i.title,
            first_seen: i.first_seen,
            last_seen: i.last_seen,
            latest_sync_id: i.latest_sync_id,
            imported_document_id: i.imported_document_id,
            status: match i.status {
                ItemStatus::Discovered => ConnectorItemStatusDto::Discovered,
                ItemStatus::Imported => ConnectorItemStatusDto::Imported,
                ItemStatus::Indexed => ConnectorItemStatusDto::Indexed,
            },
        })
        .collect())
}

#[server(
    name = BulkImportFromConnector,
    prefix = "/api",
    endpoint = "bulk_import_from_connector"
)]
pub async fn bulk_import_from_connector(
    connector_id: uuid::Uuid,
    source_ref_keys: Vec<String>,
    index_after_import: bool,
) -> Result<BulkImportResultDto, ServerFnError> {
    let result = ctx::<Arc<IngestionServices>>()?
        .bulk_import_service
        .bulk_import(connector_id, source_ref_keys, index_after_import)
        .await
        .map_err(|e| map_app_error(&e))?;
    Ok(BulkImportResultDto {
        imported: result.imported,
        indexed: result.indexed,
        failures: result
            .failures
            .into_iter()
            .map(|f| BulkImportFailureDto {
                source_ref_key: f.source_ref_key,
                error: f.error,
            })
            .collect(),
    })
}
