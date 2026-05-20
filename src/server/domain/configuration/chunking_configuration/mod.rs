pub mod aggregate;
pub mod commands;
pub mod entity;
pub mod events;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::ChunkingConfigurationCatalog;
pub use commands::{
    AddChunkingConfiguration, ChunkingConfigurationCatalogCommand, RemoveChunkingConfiguration,
    UpdateChunkingConfiguration,
};
pub use entity::ChunkingConfiguration;
pub use events::{
    ChunkingConfigurationAdded, ChunkingConfigurationCatalogCreated,
    ChunkingConfigurationCatalogEvent, ChunkingConfigurationRemoved, ChunkingConfigurationUpdated,
};
pub use projector::make_chunking_configuration_projector;
pub use read_model::ChunkingConfigurationReadModel;
pub use repository::{ChunkingConfigurationRepository, ChunkingConfigurationRepositoryError};
