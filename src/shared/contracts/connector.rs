use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorKindDto {
    Sitemap,
}

impl ConnectorKindDto {
    pub fn display_label(&self) -> &'static str {
        match self {
            ConnectorKindDto::Sitemap => "Sitemap",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitemapConfigDto {
    pub url: String,
    #[serde(default)]
    pub include_patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorConfigDto {
    Sitemap(SitemapConfigDto),
}

impl ConnectorConfigDto {
    pub fn kind(&self) -> ConnectorKindDto {
        match self {
            ConnectorConfigDto::Sitemap(_) => ConnectorKindDto::Sitemap,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDto {
    pub connector_id: Uuid,
    pub name: String,
    pub config: ConnectorConfigDto,
    #[serde(default)]
    pub default_index_profile_id: Option<Uuid>,
    #[serde(default)]
    pub default_chunking_configuration_id: Option<Uuid>,
    #[serde(default)]
    pub default_retrieval_profile_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterConnectorDto {
    pub name: String,
    pub config: ConnectorConfigDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameConnectorDto {
    pub connector_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConnectorConfigDto {
    pub connector_id: Uuid,
    pub config: ConnectorConfigDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnregisterConnectorDto {
    pub connector_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetConnectorDefaultsDto {
    pub connector_id: Uuid,
    pub index_profile_id: Option<Uuid>,
    pub chunking_configuration_id: Option<Uuid>,
    pub retrieval_profile_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorCommandDto {
    RegisterConnector(RegisterConnectorDto),
    RenameConnector(RenameConnectorDto),
    UpdateConnectorConfig(UpdateConnectorConfigDto),
    UnregisterConnector(UnregisterConnectorDto),
    SetConnectorDefaults(SetConnectorDefaultsDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorSyncSummaryDto {
    pub sync_id: Uuid,
    pub connector_id: Uuid,
    pub status: String,
    pub discovered_count: u32,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorItemStatusDto {
    Discovered,
    Imported,
    Indexed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDiscoveredItemViewDto {
    pub connector_id: Uuid,
    pub source_ref_key: String,
    pub title: String,
    pub first_seen: String,
    pub last_seen: String,
    pub latest_sync_id: Uuid,
    pub imported_document_id: Option<Uuid>,
    pub status: ConnectorItemStatusDto,
    pub available: bool,
}
