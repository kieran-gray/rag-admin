use serde::{Deserialize, Serialize};

use crate::shared::reference_data::{AiProviderKind, ModelCapability, VectorStoreKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredModelDto {
    pub id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub dimensions: Option<u32>,
    pub context_length: Option<u32>,
    pub parameter_size: Option<String>,
    pub quantization: Option<String>,
    pub family: Option<String>,
    pub size_bytes: Option<u64>,
    pub capability_hint: Option<ModelCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredIndexDto {
    pub name: String,
    pub dimensions: u32,
    pub metric: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResponseDto {
    pub provider: AiProviderKind,
    pub capability: ModelCapability,
    pub outcome: DiscoveryOutcomeDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiscoveryOutcomeDto {
    Success { models: Vec<DiscoveredModelDto> },
    Failure { message: String },
    NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexDiscoveryResponseDto {
    pub kind: VectorStoreKind,
    pub outcome: IndexDiscoveryOutcomeDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexDiscoveryOutcomeDto {
    Success { indexes: Vec<DiscoveredIndexDto> },
    Failure { message: String },
    NotImplemented,
}
