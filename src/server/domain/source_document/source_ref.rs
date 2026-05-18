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
