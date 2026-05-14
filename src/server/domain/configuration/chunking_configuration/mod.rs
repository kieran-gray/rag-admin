pub mod read_model;
pub mod repository;

pub use read_model::ChunkingConfigurationReadModel;
pub use repository::{
    ChunkingConfigurationRepository, ChunkingConfigurationRepositoryError,
    ChunkingConfigurationUpdate, NewChunkingConfiguration,
};
