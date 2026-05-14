pub mod aggregate;
pub mod commands;
pub mod entity;
pub mod events;
pub mod exceptions;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::EmbeddingModelCatalog;
pub use commands::{
    AddEmbeddingModel, EmbeddingModelCatalogCommand, RemoveEmbeddingModel, UpdateEmbeddingModel,
};
pub use entity::EmbeddingModel;
pub use events::{
    EmbeddingModelAdded, EmbeddingModelCatalogCreated, EmbeddingModelCatalogEvent,
    EmbeddingModelRemoved, EmbeddingModelUpdated,
};
pub use exceptions::EmbeddingModelCatalogError;
pub use projector::EmbeddingModelProjector;
pub use read_model::EmbeddingModelReadModel;
pub use repository::{EmbeddingModelRepository, EmbeddingModelRepositoryError};
