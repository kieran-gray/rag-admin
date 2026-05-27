use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

use super::super::insight::Insight;
use super::super::observation::Observation;
use super::super::role::SuggestedRole;
use super::super::thread::Thread;
use super::events::MapBuildRequested;
use super::read_model::{DocumentMapDetail, DocumentMapReadModel};

#[derive(Debug, Error)]
pub enum DocumentMapRepositoryError {
    #[error("document map repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait DocumentMapRepository: Send + Sync {
    async fn load_summary(
        &self,
        map_id: Uuid,
    ) -> Result<Option<DocumentMapReadModel>, DocumentMapRepositoryError>;

    async fn find_for_document(
        &self,
        document_id: Uuid,
        document_version: u32,
    ) -> Result<Option<DocumentMapReadModel>, DocumentMapRepositoryError>;

    async fn load_detail(
        &self,
        map_id: Uuid,
    ) -> Result<Option<DocumentMapDetail>, DocumentMapRepositoryError>;

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<DocumentMapReadModel>, DocumentMapRepositoryError>;

    async fn list_all(&self) -> Result<Vec<DocumentMapReadModel>, DocumentMapRepositoryError>;

    async fn load_observations_for_section(
        &self,
        map_id: Uuid,
        from_chunk_sequence: u32,
        to_chunk_sequence_inclusive: u32,
    ) -> Result<Vec<Observation>, DocumentMapRepositoryError>;

    async fn carried_summary(&self, map_id: Uuid) -> Result<String, DocumentMapRepositoryError>;

    async fn project_requested(
        &self,
        event: &MapBuildRequested,
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_roles(
        &self,
        map_id: Uuid,
        roles: &[SuggestedRole],
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_observations(
        &self,
        map_id: Uuid,
        chunk_sequence: u32,
        observations: &[Observation],
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_chunk_extraction_failure(
        &self,
        map_id: Uuid,
        chunk_sequence: u32,
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_threads(
        &self,
        map_id: Uuid,
        section_sequence: u32,
        carried_summary: &str,
        threads: &[Thread],
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_section_synthesis_failure(
        &self,
        map_id: Uuid,
        section_sequence: u32,
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_insights(
        &self,
        map_id: Uuid,
        insights: &[Insight],
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn project_failure(
        &self,
        map_id: Uuid,
        reason: &str,
    ) -> Result<(), DocumentMapRepositoryError>;

    async fn delete(&self, map_id: Uuid) -> Result<(), DocumentMapRepositoryError>;

    async fn delete_event_stream(&self, map_id: Uuid) -> Result<(), DocumentMapRepositoryError>;
}

impl From<DocumentMapRepositoryError> for ProjectionError {
    fn from(value: DocumentMapRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
