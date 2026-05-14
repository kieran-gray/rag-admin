use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::markdown::{Block, BlockKind};
use crate::server::application::ports::MarkdownParser;
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::indexing::repository::IndexingRepository;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::DocumentMetadata;
use crate::shared::{
    ChunkDto, IndexingDto, MarkdownBlockDto, MarkdownBlockKindDto, SourceDocumentDetailDto,
    SourceDocumentDto, SourceDocumentMarkdownDto,
};

pub struct SourceDocumentQueryService {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    indexing_repository: Arc<dyn IndexingRepository>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    blob_store: Arc<dyn BlobStore>,
    markdown_parser: Arc<dyn MarkdownParser>,
}

impl SourceDocumentQueryService {
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        indexing_repository: Arc<dyn IndexingRepository>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        blob_store: Arc<dyn BlobStore>,
        markdown_parser: Arc<dyn MarkdownParser>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            indexing_repository,
            chunk_set_repository,
            blob_store,
            markdown_parser,
        })
    }

    pub async fn list(&self) -> Result<Vec<SourceDocumentDto>, AppError> {
        let docs = self.source_document_repository.list().await?;
        Ok(docs.into_iter().map(map_doc_to_dto).collect())
    }

    pub async fn get_detail_by_source_ref(
        &self,
        source_ref: &crate::server::domain::source_document::source_ref::SourceRef,
    ) -> Result<Option<SourceDocumentDetailDto>, AppError> {
        let doc = self
            .source_document_repository
            .find_by_source_ref(source_ref)
            .await?;
        match doc {
            None => Ok(None),
            Some(doc) => self.get_detail(doc.document_id).await,
        }
    }

    pub async fn get_detail(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SourceDocumentDetailDto>, AppError> {
        let doc = self.source_document_repository.load(document_id).await?;

        match doc {
            None => Ok(None),
            Some(doc) => {
                let indexings = self
                    .indexing_repository
                    .list_for_document(document_id)
                    .await?;

                Ok(Some(SourceDocumentDetailDto {
                    document: map_doc_to_dto(doc),
                    indexings: indexings
                        .into_iter()
                        .map(|i| IndexingDto {
                            indexing_id: i.indexing_id,
                            pipeline_configuration_id: i.pipeline_configuration_id,
                            document_version: i.document_version,
                            status: format!("{:?}", i.status),
                            attempts: i.attempts,
                            chunk_set_id: i.chunk_set_id,
                            embedding_set_id: i.embedding_set_id,
                            removed: i.removed,
                        })
                        .collect(),
                }))
            }
        }
    }

    pub async fn get_source_markdown(
        &self,
        source_ref: &SourceRef,
    ) -> Result<Option<SourceDocumentMarkdownDto>, AppError> {
        let doc = match self
            .source_document_repository
            .find_by_source_ref(source_ref)
            .await?
        {
            Some(d) => d,
            None => return Ok(None),
        };

        let bytes = self.blob_store.get(&doc.latest_content_hash).await?;
        let source = String::from_utf8(bytes).map_err(|e| {
            AppError::Internal(format!("content for {} is not utf-8: {e}", doc.document_id))
        })?;

        let parsed = self.markdown_parser.parse(&source)?;
        let blocks = parsed.blocks.iter().map(block_to_dto).collect();

        let title = match &doc.latest_metadata {
            DocumentMetadata::BlogPost(m) => m.title.clone(),
        };

        Ok(Some(SourceDocumentMarkdownDto {
            document_id: doc.document_id,
            source_ref_key: doc.source_ref.natural_key().to_string(),
            title,
            version: doc.latest_version_number,
            source,
            blocks,
        }))
    }

    pub async fn get_chunks(&self, chunk_set_id: Uuid) -> Result<Vec<ChunkDto>, AppError> {
        let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
        Ok(chunks
            .into_iter()
            .map(|c| ChunkDto {
                chunk_id: c.chunk_id,
                sequence: c.sequence,
                heading: c.heading,
                text: c.text,
                char_start: c.char_start,
                char_end: c.char_end,
            })
            .collect())
    }
}

fn block_to_dto(block: &Block) -> MarkdownBlockDto {
    let (kind, heading_depth) = match &block.kind {
        BlockKind::Heading(h) => (MarkdownBlockKindDto::Heading, Some(h.depth)),
        BlockKind::Paragraph => (MarkdownBlockKindDto::Paragraph, None),
        BlockKind::List => (MarkdownBlockKindDto::List, None),
        BlockKind::CodeFence => (MarkdownBlockKindDto::CodeFence, None),
        BlockKind::BlockQuote => (MarkdownBlockKindDto::BlockQuote, None),
        BlockKind::Table => (MarkdownBlockKindDto::Table, None),
        BlockKind::Html => (MarkdownBlockKindDto::Html, None),
        BlockKind::ThematicBreak => (MarkdownBlockKindDto::ThematicBreak, None),
        BlockKind::Other => (MarkdownBlockKindDto::Other, None),
    };
    MarkdownBlockDto {
        kind,
        html: markdown::to_html(&block.text),
        char_start: block.span.char_start as u32,
        char_end: block.span.char_end as u32,
        heading_depth,
    }
}

fn map_doc_to_dto(
    doc: crate::server::domain::source_document::read_model::SourceDocumentReadModel,
) -> SourceDocumentDto {
    use crate::server::domain::source_document::version::DocumentMetadata;
    let title = match &doc.latest_metadata {
        DocumentMetadata::BlogPost(m) => m.title.clone(),
    };
    SourceDocumentDto {
        document_id: doc.document_id,
        document_type: format!("{:?}", doc.document_type),
        source_ref_key: doc.source_ref.natural_key().to_string(),
        title,
        latest_version: doc.latest_version_number,
        latest_content_hash: doc.latest_content_hash.as_hex().to_string(),
        deleted: doc.deleted,
    }
}
