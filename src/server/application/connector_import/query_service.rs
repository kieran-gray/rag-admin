use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::connector_import::{
    ConnectorImportReadModel, ConnectorImportRepository,
};

pub struct ConnectorImportQueryService {
    repository: Arc<dyn ConnectorImportRepository>,
}

impl ConnectorImportQueryService {
    pub fn new(repository: Arc<dyn ConnectorImportRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn list_for_documents(
        &self,
        document_ids: &[Uuid],
    ) -> Result<Vec<ConnectorImportReadModel>, AppError> {
        Ok(self.repository.list_for_documents(document_ids).await?)
    }

    pub async fn list_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Vec<ConnectorImportReadModel>, AppError> {
        Ok(self.repository.list_for_connector(connector_id).await?)
    }

    pub async fn document_ids_for_connectors(
        &self,
        connector_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, AppError> {
        Ok(self
            .repository
            .document_ids_for_connectors(connector_ids)
            .await?)
    }

    pub async fn facets(&self) -> Result<Vec<(Uuid, u64)>, AppError> {
        Ok(self.repository.facets().await?)
    }
}
