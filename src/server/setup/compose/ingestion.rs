use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Notify;

use crate::server::application::configuration::{
    ChunkingConfigurationQueryService, IndexProfileResolver,
};
use crate::server::application::connector::{ConnectorQueryService, ConnectorRegistry};
use crate::server::application::connector_sync::BulkImportService;
use crate::server::application::indexing::IndexingCommandHandler;
use crate::server::application::ports::{Clock, HttpClient, IdGenerator, MarkdownParser};
use crate::server::application::source_document::ports::{HtmlToMarkdown, PdfToMarkdown};
use crate::server::application::source_document::{
    DocumentNormalizerRegistry, SourceDocumentCommandHandler, SourceDocumentIngestService,
    SourceDocumentIngestServiceDeps, SourceDocumentQueryService,
};
use crate::server::domain::source_document::aggregate::SourceDocument;
use crate::server::domain::source_document::projector::SourceDocumentProjector;
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::infrastructure::source_document::normalizers::html::HtmdConverter;
use crate::server::infrastructure::source_document::normalizers::pdf::PdfExtractConverter;
use crate::server::infrastructure::source_document::{
    HtmlNormalizer, MarkdownNormalizer, PdfNormalizer, PlainTextNormalizer,
};
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;

pub struct IngestionServices {
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub source_document_query_service: Arc<SourceDocumentQueryService>,
    pub source_document_ingest_service: Arc<SourceDocumentIngestService>,
    pub bulk_import_service: Arc<BulkImportService>,
}

pub struct IngestionDeps<'a> {
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub http: Arc<ReqwestHttpClient>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
    pub event_bus: Arc<EventBus>,
    pub markdown_parser: Arc<dyn MarkdownParser>,
    pub index_profile_resolver: Arc<IndexProfileResolver>,
    pub chunking_configuration_query_service: Arc<ChunkingConfigurationQueryService>,
    pub connector_registry: Arc<ConnectorRegistry>,
    pub connector_query_service: Arc<ConnectorQueryService>,
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub indexing_command_handler: Arc<IndexingCommandHandler>,
    pub wakeups: &'a mut HashMap<String, Arc<Notify>>,
}

impl IngestionServices {
    pub fn build(deps: IngestionDeps<'_>) -> Result<Self, SetupError> {
        let IngestionDeps {
            clock,
            id_generator,
            http,
            repos,
            wirings,
            event_bus,
            markdown_parser,
            index_profile_resolver,
            chunking_configuration_query_service,
            connector_registry,
            connector_query_service,
            source_document_command_handler,
            indexing_command_handler,
            wakeups,
        } = deps;

        let source_document_query_service = SourceDocumentQueryService::new(
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.indexing),
            Arc::clone(&repos.chunk_set),
            Arc::clone(&repos.blob_store),
            markdown_parser,
            Arc::clone(&connector_query_service),
        );

        let html_to_markdown: Arc<dyn HtmlToMarkdown> = HtmdConverter::new();
        let pdf_to_markdown: Arc<dyn PdfToMarkdown> = PdfExtractConverter::new();
        let http_port: Arc<dyn HttpClient> = Arc::clone(&http) as Arc<dyn HttpClient>;

        let mut normalizer_registry = DocumentNormalizerRegistry::new();
        normalizer_registry.register(MarkdownNormalizer::new());
        normalizer_registry.register(PlainTextNormalizer::new());
        normalizer_registry.register(HtmlNormalizer::new(html_to_markdown, Arc::clone(&clock)));
        normalizer_registry.register(PdfNormalizer::new(pdf_to_markdown));
        let normalizer_registry = Arc::new(normalizer_registry);

        let source_document_ingest_service =
            SourceDocumentIngestService::new(SourceDocumentIngestServiceDeps {
                source_document_command_handler: Arc::clone(&source_document_command_handler),
                indexing_command_handler,
                source_document_repository: Arc::clone(&repos.source_document),
                blob_store: Arc::clone(&repos.blob_store),
                connector_registry,
                connector_query_service,
                index_profile_resolver: Arc::clone(&index_profile_resolver),
                http_client: http_port,
                normalizer_registry,
                clock,
                id_generator,
            });

        let bulk_import_service = BulkImportService::new(
            Arc::clone(&source_document_ingest_service),
            index_profile_resolver,
            chunking_configuration_query_service,
        );

        spawn_driver::<SourceDocument, ()>(
            Arc::clone(&wirings.source_document.event_store),
            vec![Arc::new(SourceDocumentProjector::new(Arc::clone(
                &repos.source_document,
            )))],
            None,
            Arc::clone(&repos.checkpoint),
            event_bus,
            wakeups,
        );

        Ok(Self {
            source_document_command_handler,
            source_document_query_service,
            source_document_ingest_service,
            bulk_import_service,
        })
    }
}
