pub mod aggregate;
pub mod commands;
pub mod entity;
pub mod events;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::IndexProfileCatalog;
pub use commands::{
    AddIndexProfile, EmbeddingModelRef, IndexProfileCatalogCommand, RemoveIndexProfile,
    UpdateIndexProfile, VectorIndexRef,
};
pub use entity::IndexProfile;
pub use events::{
    IndexProfileAdded, IndexProfileCatalogCreated, IndexProfileCatalogEvent, IndexProfileRemoved,
    IndexProfileUpdated,
};
pub use projector::make_index_profile_projector;
pub use read_model::IndexProfileReadModel;
pub use repository::{IndexProfileRepository, IndexProfileRepositoryError};
