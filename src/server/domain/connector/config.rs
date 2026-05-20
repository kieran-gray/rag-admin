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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_kind_strings() {
        assert_eq!(ConnectorKind::Sitemap.as_str(), "sitemap");
        assert_eq!(ConnectorKind::Sitemap.display_label(), "Sitemap");
    }

    #[test]
    fn config_kind_matches_inner_variant() {
        let config = ConnectorConfig::Sitemap(SitemapConfig {
            url: "https://example.com/sitemap.xml".into(),
            include_patterns: vec![],
            exclude_patterns: vec![],
        });
        assert_eq!(config.kind(), ConnectorKind::Sitemap);
    }

    #[test]
    fn sitemap_config_defaults_patterns_when_missing() {
        let raw = r#"{"url": "https://example.com/sitemap.xml"}"#;
        let parsed: SitemapConfig = serde_json::from_str(raw).expect("parse");
        assert_eq!(parsed.url, "https://example.com/sitemap.xml");
        assert!(parsed.include_patterns.is_empty());
        assert!(parsed.exclude_patterns.is_empty());
    }

    #[test]
    fn connector_config_serde_uses_tagged_form() {
        let config = ConnectorConfig::Sitemap(SitemapConfig {
            url: "https://example.com/sitemap.xml".into(),
            include_patterns: vec!["/blog/".into()],
            exclude_patterns: vec!["/legacy/".into()],
        });
        let json = serde_json::to_value(&config).expect("serialize");
        assert_eq!(json["type"], "Sitemap");
        assert_eq!(json["data"]["url"], "https://example.com/sitemap.xml");
        let back: ConnectorConfig = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, config);
    }
}
