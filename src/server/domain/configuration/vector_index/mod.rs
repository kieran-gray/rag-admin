pub mod aggregate;
pub mod commands;
pub mod entity;
pub mod events;
pub mod exceptions;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::VectorIndexCatalog;
pub use commands::{
    AddVectorIndex, RemoveVectorIndex, UpdateVectorIndex, VectorIndexCatalogCommand,
};
pub use entity::VectorIndex;
pub use events::{
    VectorIndexAdded, VectorIndexCatalogCreated, VectorIndexCatalogEvent, VectorIndexRemoved,
    VectorIndexUpdated,
};
pub use exceptions::VectorIndexCatalogError;
pub use projector::VectorIndexProjector;
pub use read_model::VectorIndexReadModel;
pub use repository::{VectorIndexRepository, VectorIndexRepositoryError};
