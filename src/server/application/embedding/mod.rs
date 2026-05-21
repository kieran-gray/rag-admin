pub mod ports;
pub mod registry;
pub mod service;

pub use registry::EmbedderRegistry;
pub use service::{EmbeddingService, ResolvedEmbeddingModel};
