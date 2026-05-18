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
#[serde(tag = "type", content = "data")]
pub enum ConnectorCommandDto {
    RegisterConnector(RegisterConnectorDto),
    RenameConnector(RenameConnectorDto),
    UpdateConnectorConfig(UpdateConnectorConfigDto),
    UnregisterConnector(UnregisterConnectorDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDiscoveredItemDto {
    pub source_ref_key: String,
    pub title: String,
    pub already_imported: bool,
}
