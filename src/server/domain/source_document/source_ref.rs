use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SourceRef {
    UpstreamSlug { slug: String },
}

impl SourceRef {
    pub fn natural_key(&self) -> &str {
        match self {
            SourceRef::UpstreamSlug { slug } => slug,
        }
    }
}
