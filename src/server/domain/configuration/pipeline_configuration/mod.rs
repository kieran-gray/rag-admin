pub mod aggregate;
pub mod commands;
pub mod entity;
pub mod events;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::PipelineConfigurationCatalog;
pub use commands::{
    AddPipelineConfiguration, EmbeddingModelRef, GenerationModelRef,
    PipelineConfigurationCatalogCommand, RemovePipelineConfiguration, UpdatePipelineConfiguration,
    VectorIndexRef,
};
pub use entity::PipelineConfiguration;
pub use events::{
    PipelineConfigurationAdded, PipelineConfigurationCatalogCreated,
    PipelineConfigurationCatalogEvent, PipelineConfigurationRemoved, PipelineConfigurationUpdated,
};
pub use projector::make_pipeline_configuration_projector;
pub use read_model::PipelineConfigurationReadModel;
pub use repository::{PipelineConfigurationRepository, PipelineConfigurationRepositoryError};
