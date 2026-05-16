use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SourceRef {
    UpstreamSlug { slug: String },
    Upload { upload_id: Uuid },
    Url { url: String },
}

impl SourceRef {
    pub fn natural_key(&self) -> String {
        match self {
            SourceRef::UpstreamSlug { slug } => slug.clone(),
            SourceRef::Upload { upload_id } => format!("upload:{upload_id}"),
            SourceRef::Url { url } => format!("url:{url}"),
        }
    }

    pub fn parse_route_key(value: &str) -> Self {
        if let Some(rest) = value.strip_prefix("upload:") {
            if let Ok(id) = Uuid::parse_str(rest) {
                return SourceRef::Upload { upload_id: id };
            }
        }
        if let Some(rest) = value.strip_prefix("url:") {
            return SourceRef::Url {
                url: rest.to_string(),
            };
        }
        SourceRef::UpstreamSlug {
            slug: value.to_string(),
        }
    }
}
