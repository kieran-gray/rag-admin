pub mod aggregate;
pub mod commands;
pub mod entity;
pub mod events;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::RetrievalProfileCatalog;
pub use commands::{
    AddRetrievalProfile, GenerationModelRef, IndexProfileRef, RemoveRetrievalProfile,
    RetrievalProfileCatalogCommand, UpdateRetrievalProfile,
};
pub use entity::RetrievalProfile;
pub use events::{
    RetrievalProfileAdded, RetrievalProfileCatalogCreated, RetrievalProfileCatalogEvent,
    RetrievalProfileRemoved, RetrievalProfileUpdated,
};
pub use projector::make_retrieval_profile_projector;
pub use read_model::RetrievalProfileReadModel;
pub use repository::{RetrievalProfileRepository, RetrievalProfileRepositoryError};
