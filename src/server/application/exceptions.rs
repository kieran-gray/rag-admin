use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::server::domain::chunk_set::repository::ChunkSetRepositoryError;
use crate::server::domain::configuration::{
    chunking_configuration::ChunkingConfigurationRepositoryError,
    embedding_model::{EmbeddingModelCatalogError, EmbeddingModelRepositoryError},
    generation_model::{GenerationModelCatalogError, GenerationModelRepositoryError},
    pipeline_configuration::PipelineConfigurationRepositoryError,
    sweep_template::{SweepTemplateError, SweepTemplateRepositoryError},
    vector_index::{VectorIndexCatalogError, VectorIndexRepositoryError},
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

#[derive(Debug, Error, Serialize, Deserialize)]
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

impl From<EmbeddingModelCatalogError> for AppError {
    fn from(value: EmbeddingModelCatalogError) -> Self {
        match value {
            EmbeddingModelCatalogError::NotFound => AppError::NotFound(value.to_string()),
            EmbeddingModelCatalogError::ValidationError(_)
            | EmbeddingModelCatalogError::InvalidCommand(_) => {
                AppError::Validation(value.to_string())
            }
        }
    }
}

impl From<GenerationModelCatalogError> for AppError {
    fn from(value: GenerationModelCatalogError) -> Self {
        match value {
            GenerationModelCatalogError::NotFound => AppError::NotFound(value.to_string()),
            GenerationModelCatalogError::ValidationError(_)
            | GenerationModelCatalogError::InvalidCommand(_) => {
                AppError::Validation(value.to_string())
            }
        }
    }
}

impl From<VectorIndexCatalogError> for AppError {
    fn from(value: VectorIndexCatalogError) -> Self {
        match value {
            VectorIndexCatalogError::NotFound => AppError::NotFound(value.to_string()),
            VectorIndexCatalogError::ValidationError(_)
            | VectorIndexCatalogError::InvalidCommand(_) => AppError::Validation(value.to_string()),
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

impl From<VectorIndexRepositoryError> for AppError {
    fn from(value: VectorIndexRepositoryError) -> Self {
        AppError::Internal(value.to_string())
    }
}

impl From<PipelineConfigurationRepositoryError> for AppError {
    fn from(value: PipelineConfigurationRepositoryError) -> Self {
        match value {
            PipelineConfigurationRepositoryError::NotFound(_) => {
                AppError::NotFound(value.to_string())
            }
            PipelineConfigurationRepositoryError::NameConflict
            | PipelineConfigurationRepositoryError::ReferenceViolation(_) => {
                AppError::Validation(value.to_string())
            }
            PipelineConfigurationRepositoryError::Internal(_) => {
                AppError::Internal(value.to_string())
            }
        }
    }
}

impl From<ChunkingConfigurationRepositoryError> for AppError {
    fn from(value: ChunkingConfigurationRepositoryError) -> Self {
        match value {
            ChunkingConfigurationRepositoryError::NotFound(_) => {
                AppError::NotFound(value.to_string())
            }
            ChunkingConfigurationRepositoryError::NameConflict
            | ChunkingConfigurationRepositoryError::ReferenceViolation(_) => {
                AppError::Validation(value.to_string())
            }
            ChunkingConfigurationRepositoryError::Internal(_) => {
                AppError::Internal(value.to_string())
            }
        }
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

impl From<SourceDocumentError> for AppError {
    fn from(value: SourceDocumentError) -> Self {
        match value {
            SourceDocumentError::NotFound => AppError::NotFound(value.to_string()),
            SourceDocumentError::AlreadyExists => AppError::Validation(value.to_string()),
            SourceDocumentError::AlreadyDeleted => AppError::Validation(value.to_string()),
            SourceDocumentError::ValidationError(_) => AppError::Validation(value.to_string()),
            SourceDocumentError::InvalidCommand(_) => AppError::Validation(value.to_string()),
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
            IndexingError::Removed => AppError::Validation(value.to_string()),
            IndexingError::NotFailed => AppError::Validation(value.to_string()),
            IndexingError::ValidationError(_) => AppError::Validation(value.to_string()),
            IndexingError::InvalidCommand(_) => AppError::Validation(value.to_string()),
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
            EvaluationDatasetError::AlreadyExists => AppError::Validation(value.to_string()),
            EvaluationDatasetError::NotFound => AppError::NotFound(value.to_string()),
            EvaluationDatasetError::GenerationNotInProgress
            | EvaluationDatasetError::AlreadyCompleted
            | EvaluationDatasetError::AlreadyFailed
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
            EvaluationRunError::AlreadyExists => AppError::Validation(value.to_string()),
            EvaluationRunError::NotFound => AppError::NotFound(value.to_string()),
            EvaluationRunError::AlreadyCompleted
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
