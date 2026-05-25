use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::chunking::ChunkingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSetSummaryDto {
    pub chunk_set_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub chunking_config: ChunkingConfig,
    pub created_at: String,
    pub pinned: bool,
    pub chunk_count: u32,
    pub indexing_refs: u32,
    pub variant_result_refs: u32,
}

impl ChunkSetSummaryDto {
    pub fn in_use(&self) -> bool {
        self.pinned || self.indexing_refs > 0 || self.variant_result_refs > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetChunkSetPinnedRequestDto {
    pub chunk_set_id: Uuid,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteChunkSetRequestDto {
    pub chunk_set_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcChunkSetsRequestDto {
    pub older_than_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcChunkSetsResponseDto {
    pub deleted: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkSetStatusFilterDto {
    Pinned,
    Indexed,
    UsedByEval,
    Unused,
}

impl ChunkSetStatusFilterDto {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pinned => "Pinned",
            Self::Indexed => "Indexed",
            Self::UsedByEval => "Used by eval",
            Self::Unused => "Unused",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Indexed => "indexed",
            Self::UsedByEval => "used-by-eval",
            Self::Unused => "unused",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "pinned" => Some(Self::Pinned),
            "indexed" => Some(Self::Indexed),
            "used-by-eval" => Some(Self::UsedByEval),
            "unused" => Some(Self::Unused),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkSetListQueryDto {
    #[serde(default)]
    pub cursor: Option<String>,
    pub limit: u32,
    #[serde(default)]
    pub statuses: Vec<ChunkSetStatusFilterDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSetStatusFacetDto {
    pub status: ChunkSetStatusFilterDto,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSetListPageDto {
    pub items: Vec<ChunkSetSummaryDto>,
    pub next_cursor: Option<String>,
    pub total_matching: u64,
    pub total_all: u64,
    pub status_counts: Vec<ChunkSetStatusFacetDto>,
}
