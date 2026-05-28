use std::error::Error as StdError;

use thiserror::Error;

use event_sourcing::error::{CommandError, EsError, ProjectionError};

use crate::server::domain::chunk_set::exceptions::ChunkSetError;
use crate::server::domain::chunk_set::repository::ChunkSetRepositoryError;
use crate::server::domain::comprehension::map::exceptions::DocumentMapError;
use crate::server::domain::comprehension::map::repository::DocumentMapRepositoryError;
use crate::server::domain::configuration::catalog::CatalogError;
use crate::server::domain::configuration::defaults::{
    ConfigurationDefaultsError, ConfigurationDefaultsRepositoryError,
};
use crate::server::domain::configuration::{
    chunking_configuration::ChunkingConfigurationRepositoryError,
    embedding_model::EmbeddingModelRepositoryError,
    generation_model::GenerationModelRepositoryError,
    index_profile::IndexProfileRepositoryError,
    reranker_model::RerankerModelRepositoryError,
    retrieval_profile::RetrievalProfileRepositoryError,
    sweep_template::{SweepTemplateError, SweepTemplateRepositoryError},
    vector_index::VectorIndexRepositoryError,
};
use crate::server::domain::connector::{ConnectorError, ConnectorRepositoryError};
use crate::server::domain::connector_sync::{
    ConnectorDiscoveredItemRepositoryError, ConnectorSyncError, ConnectorSyncRepositoryError,
};
use crate::server::domain::embedding_set::repository::EmbeddingSetRepositoryError;
use crate::server::domain::evaluation::{
    dataset::{exceptions::EvaluationDatasetError, repository::EvaluationDatasetRepositoryError},
    run::{exceptions::EvaluationRunError, repository::EvaluationRunRepositoryError},
};
use crate::server::domain::indexing::{
    exceptions::IndexingError, repository::IndexingRepositoryError,
};
use crate::server::domain::source_document::{
    exceptions::SourceDocumentError, repository::SourceDocumentRepositoryError,
};
use crate::shared::contracts::{ApiError, ApiErrorKind};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<&AppError> for ApiError {
    fn from(err: &AppError) -> Self {
        let (kind, message) = match err {
            AppError::NotFound(m) => (ApiErrorKind::NotFound, m.clone()),
            AppError::Validation(m) => (ApiErrorKind::Validation, m.clone()),
            AppError::Upstream(m) => (ApiErrorKind::Upstream, m.clone()),
            AppError::Io(m) | AppError::Internal(m) => (ApiErrorKind::Internal, m.clone()),
        };
        Self { kind, message }
    }
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        Self::from(&err)
    }
}

impl From<CatalogError> for AppError {
    fn from(value: CatalogError) -> Self {
        match value {
            CatalogError::NotFound => AppError::NotFound(value.to_string()),
            CatalogError::ValidationError(_) | CatalogError::InvalidCommand(_) => {
                AppError::Validation(value.to_string())
            }
        }
    }
}

impl From<ConfigurationDefaultsError> for AppError {
    fn from(value: ConfigurationDefaultsError) -> Self {
        match value {
            ConfigurationDefaultsError::ValidationError(_) => {
                AppError::Validation(value.to_string())
            }
        }
    }
}

impl From<EmbeddingModelRepositoryError> for AppError {
    fn from(value: EmbeddingModelRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<GenerationModelRepositoryError> for AppError {
    fn from(value: GenerationModelRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<RerankerModelRepositoryError> for AppError {
    fn from(value: RerankerModelRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<VectorIndexRepositoryError> for AppError {
    fn from(value: VectorIndexRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<IndexProfileRepositoryError> for AppError {
    fn from(value: IndexProfileRepositoryError) -> Self {
        match value {
            IndexProfileRepositoryError::Internal(_) => AppError::Internal(value.to_string()),
        }
    }
}

impl From<RetrievalProfileRepositoryError> for AppError {
    fn from(value: RetrievalProfileRepositoryError) -> Self {
        match value {
            RetrievalProfileRepositoryError::Internal(_) => AppError::Internal(value.to_string()),
        }
    }
}

impl From<ChunkingConfigurationRepositoryError> for AppError {
    fn from(value: ChunkingConfigurationRepositoryError) -> Self {
        match value {
            ChunkingConfigurationRepositoryError::Internal(_) => {
                AppError::Internal(value.to_string())
            }
        }
    }
}

impl From<ConfigurationDefaultsRepositoryError> for AppError {
    fn from(value: ConfigurationDefaultsRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<SweepTemplateRepositoryError> for AppError {
    fn from(value: SweepTemplateRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<SweepTemplateError> for AppError {
    fn from(value: SweepTemplateError) -> Self {
        match value {
            SweepTemplateError::NotFound => AppError::NotFound(value.to_string()),
            SweepTemplateError::AlreadyExists
            | SweepTemplateError::AlreadyDeleted
            | SweepTemplateError::ValidationError(_)
            | SweepTemplateError::InvalidCommand(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<ConnectorError> for AppError {
    fn from(value: ConnectorError) -> Self {
        match value {
            ConnectorError::NotFound => AppError::NotFound(value.to_string()),
            ConnectorError::AlreadyExists
            | ConnectorError::AlreadyDeleted
            | ConnectorError::ValidationError(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<ConnectorRepositoryError> for AppError {
    fn from(value: ConnectorRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<ConnectorSyncError> for AppError {
    fn from(value: ConnectorSyncError) -> Self {
        match value {
            ConnectorSyncError::NotFound => AppError::NotFound(value.to_string()),
            ConnectorSyncError::AlreadyStarted
            | ConnectorSyncError::AlreadyTerminal
            | ConnectorSyncError::InvalidCommand(_) => AppError::Validation(value.to_string()),
            ConnectorSyncError::Internal(_) => AppError::Internal(value.to_string()),
        }
    }
}

impl From<ConnectorSyncRepositoryError> for AppError {
    fn from(value: ConnectorSyncRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<ConnectorDiscoveredItemRepositoryError> for AppError {
    fn from(value: ConnectorDiscoveredItemRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<SourceDocumentError> for AppError {
    fn from(value: SourceDocumentError) -> Self {
        match value {
            SourceDocumentError::NotFound => AppError::NotFound(value.to_string()),
            SourceDocumentError::AlreadyExists
            | SourceDocumentError::AlreadyDeleted
            | SourceDocumentError::ValidationError(_)
            | SourceDocumentError::InvalidCommand(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<SourceDocumentRepositoryError> for AppError {
    fn from(value: SourceDocumentRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<IndexingError> for AppError {
    fn from(value: IndexingError) -> Self {
        match value {
            IndexingError::NotFound => AppError::NotFound(value.to_string()),
            IndexingError::Removed
            | IndexingError::NotFailed
            | IndexingError::ValidationError(_)
            | IndexingError::InvalidCommand(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<IndexingRepositoryError> for AppError {
    fn from(value: IndexingRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<ChunkSetRepositoryError> for AppError {
    fn from(value: ChunkSetRepositoryError) -> Self {
        match value {
            ChunkSetRepositoryError::InUse(_) => AppError::Validation(value.to_string()),
            ChunkSetRepositoryError::Internal(_) => AppError::Internal(value.to_string()),
        }
    }
}

impl From<ChunkSetError> for AppError {
    fn from(value: ChunkSetError) -> Self {
        match value {
            ChunkSetError::NotFound => AppError::NotFound(value.to_string()),
            ChunkSetError::AlreadyExists
            | ChunkSetError::AlreadyDeleted
            | ChunkSetError::PinnedCannotDelete
            | ChunkSetError::ValidationError(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<DocumentMapError> for AppError {
    fn from(value: DocumentMapError) -> Self {
        match value {
            DocumentMapError::NotFound => AppError::NotFound(value.to_string()),
            DocumentMapError::AlreadyExists
            | DocumentMapError::AlreadyReady
            | DocumentMapError::InvalidTransition(_)
            | DocumentMapError::ValidationError(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<DocumentMapRepositoryError> for AppError {
    fn from(value: DocumentMapRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<EmbeddingSetRepositoryError> for AppError {
    fn from(value: EmbeddingSetRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<EvaluationDatasetError> for AppError {
    fn from(value: EvaluationDatasetError) -> Self {
        match value {
            EvaluationDatasetError::NotFound => AppError::NotFound(value.to_string()),
            EvaluationDatasetError::AlreadyExists
            | EvaluationDatasetError::GenerationNotInProgress
            | EvaluationDatasetError::AlreadyCompleted
            | EvaluationDatasetError::AlreadyFailed
            | EvaluationDatasetError::AlreadyCancelled
            | EvaluationDatasetError::NoQuestionsAccepted
            | EvaluationDatasetError::Deleted
            | EvaluationDatasetError::EmptyLabel
            | EvaluationDatasetError::InvalidCommand(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<EvaluationDatasetRepositoryError> for AppError {
    fn from(value: EvaluationDatasetRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<EvaluationRunError> for AppError {
    fn from(value: EvaluationRunError) -> Self {
        match value {
            EvaluationRunError::NotFound => AppError::NotFound(value.to_string()),
            EvaluationRunError::AlreadyExists
            | EvaluationRunError::AlreadyCompleted
            | EvaluationRunError::AlreadyFailed
            | EvaluationRunError::NotAllVariantsScored
            | EvaluationRunError::InvalidCommand(_) => AppError::Validation(value.to_string()),
        }
    }
}

impl From<EvaluationRunRepositoryError> for AppError {
    fn from(value: EvaluationRunRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<EsError> for AppError {
    fn from(value: EsError) -> Self {
        match value {
            EsError::ConcurrencyConflict { .. } => AppError::Validation(value.to_string()),
            EsError::Storage(_) | EsError::Serialization(_) => {
                AppError::Internal(value.to_string())
            }
        }
    }
}

impl From<ProjectionError> for AppError {
    fn from(value: ProjectionError) -> Self {
        match value {
            ProjectionError::Storage(_) | ProjectionError::InvalidState(_) => {
                AppError::Internal(value.to_string())
            }
        }
    }
}

impl<E> From<CommandError<E>> for AppError
where
    E: StdError + 'static,
    AppError: From<E>,
{
    fn from(value: CommandError<E>) -> Self {
        match value {
            CommandError::Aggregate(e) => AppError::from(e),
            CommandError::EventStore(e) => <AppError as From<EsError>>::from(e),
        }
    }
}
