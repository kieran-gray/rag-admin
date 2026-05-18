use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ConnectorKind {
    Sitemap,
}

impl ConnectorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectorKind::Sitemap => "sitemap",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            ConnectorKind::Sitemap => "Sitemap",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitemapConfig {
    pub url: String,
    #[serde(default)]
    pub include_patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorConfig {
    Sitemap(SitemapConfig),
}

impl ConnectorConfig {
    pub fn kind(&self) -> ConnectorKind {
        match self {
            ConnectorConfig::Sitemap(_) => ConnectorKind::Sitemap,
        }
    }
}
