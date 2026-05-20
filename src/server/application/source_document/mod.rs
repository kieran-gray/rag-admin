pub mod command_handler;
pub mod ingest_service;
pub mod normalizer_registry;
pub mod ports;
pub mod query_service;

pub use command_handler::SourceDocumentCommandHandler;
pub use ingest_service::{SourceDocumentIngestService, SourceDocumentIngestServiceDeps};
pub use normalizer_registry::DocumentNormalizerRegistry;
pub use query_service::SourceDocumentQueryService;
