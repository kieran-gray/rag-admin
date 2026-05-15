use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentListItemDto {
    pub source_ref_key: String,
    pub document_type: String,
    pub title: String,
    pub document_id: Option<Uuid>,
    pub latest_version: Option<u32>,
    pub latest_content_hash: Option<String>,
    pub indexings: Vec<IndexingDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocumentDto {
    pub document_id: Uuid,
    pub document_type: String,
    pub source_ref_key: String,
    pub title: String,
    pub latest_version: u32,
    pub latest_content_hash: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingDto {
    pub indexing_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_version: u32,
    pub status: String,
    pub attempts: u32,
    pub chunk_set_id: Option<Uuid>,
    pub embedding_set_id: Option<Uuid>,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocumentDetailDto {
    pub document: SourceDocumentDto,
    pub indexings: Vec<IndexingDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDto {
    pub chunk_id: Uuid,
    pub sequence: u32,
    pub heading: String,
    pub text: String,
    pub char_start: u32,
    pub char_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocumentMarkdownDto {
    pub document_id: Uuid,
    pub source_ref_key: String,
    pub title: String,
    pub version: u32,
    pub source: String,
    pub blocks: Vec<MarkdownBlockDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownBlockKindDto {
    Heading,
    Paragraph,
    List,
    CodeFence,
    BlockQuote,
    Table,
    Html,
    ThematicBreak,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownBlockDto {
    pub kind: MarkdownBlockKindDto,
    pub html: String,
    pub char_start: u32,
    pub char_end: u32,
    pub heading_depth: Option<u8>,
}
