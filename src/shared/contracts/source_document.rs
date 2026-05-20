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
    pub source: SourceDescriptorDto,
    #[serde(default)]
    pub connectors: Vec<DocumentConnectorDto>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentConnectorDto {
    pub connector_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceDescriptorDto {
    Upload,
    Url { host: String, url: String },
}

impl SourceDescriptorDto {
    pub fn filter_key(&self) -> String {
        match self {
            SourceDescriptorDto::Upload => "upload".to_string(),
            SourceDescriptorDto::Url { host, .. } => format!("url:{host}"),
        }
    }

    pub fn label(&self) -> String {
        match self {
            SourceDescriptorDto::Upload => "Upload".to_string(),
            SourceDescriptorDto::Url { host, .. } => host.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceFilterDto {
    Upload,
    UrlHost { host: String },
}

impl SourceFilterDto {
    pub fn from_filter_key(key: &str) -> Option<Self> {
        if key == "upload" {
            return Some(SourceFilterDto::Upload);
        }
        key.strip_prefix("url:")
            .map(|host| SourceFilterDto::UrlHost {
                host: host.to_string(),
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatusFilterDto {
    Registered,
    InProgress,
    Indexed,
    Failed,
}

impl DocumentStatusFilterDto {
    pub fn label(self) -> &'static str {
        match self {
            DocumentStatusFilterDto::Registered => "Registered",
            DocumentStatusFilterDto::InProgress => "In progress",
            DocumentStatusFilterDto::Indexed => "Indexed",
            DocumentStatusFilterDto::Failed => "Failed",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            DocumentStatusFilterDto::Registered => "registered",
            DocumentStatusFilterDto::InProgress => "in_progress",
            DocumentStatusFilterDto::Indexed => "indexed",
            DocumentStatusFilterDto::Failed => "failed",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "registered" => Some(DocumentStatusFilterDto::Registered),
            "in_progress" => Some(DocumentStatusFilterDto::InProgress),
            "indexed" => Some(DocumentStatusFilterDto::Indexed),
            "failed" => Some(DocumentStatusFilterDto::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentListQueryDto {
    #[serde(default)]
    pub cursor: Option<String>,
    pub limit: u32,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub sources: Vec<SourceFilterDto>,
    #[serde(default)]
    pub statuses: Vec<DocumentStatusFilterDto>,
    #[serde(default)]
    pub connectors: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFacetDto {
    pub source: SourceDescriptorDto,
    pub document_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorFacetDto {
    pub connector_id: Uuid,
    pub name: String,
    pub document_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentListPageDto {
    pub items: Vec<DocumentListItemDto>,
    pub next_cursor: Option<String>,
    pub total_matching: u64,
    pub total_all: u64,
    pub sources: Vec<SourceFacetDto>,
    #[serde(default)]
    pub connectors: Vec<ConnectorFacetDto>,
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
