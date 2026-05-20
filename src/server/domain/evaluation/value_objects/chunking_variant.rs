use serde::{Deserialize, Serialize};

use crate::server::domain::shared::value_objects::ChunkingConfig;
use crate::shared::ChunkingVariant as ChunkingVariantDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingVariant {
    pub label: String,
    pub config: ChunkingConfig,
}

impl From<ChunkingVariantDto> for ChunkingVariant {
    fn from(v: ChunkingVariantDto) -> Self {
        Self {
            label: v.label,
            config: v.config.into(),
        }
    }
}

impl From<ChunkingVariant> for ChunkingVariantDto {
    fn from(v: ChunkingVariant) -> Self {
        Self {
            label: v.label,
            config: v.config.into(),
        }
    }
}
