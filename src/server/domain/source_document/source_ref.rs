use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SourceRef {
    Upload { upload_id: Uuid },
    Url { url: String },
}

impl SourceRef {
    pub fn natural_key(&self) -> String {
        match self {
            SourceRef::Upload { upload_id } => format!("upload:{upload_id}"),
            SourceRef::Url { url } => format!("url:{url}"),
        }
    }

    pub fn parse_route_key(value: &str) -> Option<Self> {
        if let Some(rest) = value.strip_prefix("upload:") {
            return Uuid::parse_str(rest)
                .ok()
                .map(|id| SourceRef::Upload { upload_id: id });
        }
        if let Some(rest) = value.strip_prefix("url:") {
            return Some(SourceRef::Url {
                url: rest.to_string(),
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_natural_key_includes_uuid() {
        let id = Uuid::nil();
        let key = SourceRef::Upload { upload_id: id }.natural_key();
        assert_eq!(key, format!("upload:{id}"));
    }

    #[test]
    fn url_natural_key_includes_url() {
        let key = SourceRef::Url {
            url: "https://example.com/x".into(),
        }
        .natural_key();
        assert_eq!(key, "url:https://example.com/x");
    }

    #[test]
    fn parse_route_key_roundtrip_upload() {
        let original = SourceRef::Upload {
            upload_id: Uuid::nil(),
        };
        let parsed = SourceRef::parse_route_key(&original.natural_key()).expect("parsed");
        assert_eq!(parsed, original);
    }

    #[test]
    fn parse_route_key_roundtrip_url_with_embedded_colon() {
        let original = SourceRef::Url {
            url: "https://example.com/a:b".into(),
        };
        let parsed = SourceRef::parse_route_key(&original.natural_key()).expect("parsed");
        assert_eq!(parsed, original);
    }

    #[test]
    fn parse_route_key_returns_none_for_unknown_prefix() {
        assert_eq!(SourceRef::parse_route_key("garbage:value"), None);
        assert_eq!(SourceRef::parse_route_key(""), None);
    }

    #[test]
    fn parse_route_key_returns_none_for_invalid_upload_uuid() {
        assert_eq!(SourceRef::parse_route_key("upload:not-a-uuid"), None);
    }
}
