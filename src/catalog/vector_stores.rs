use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreKind {
    CloudflareVectorize,
    Postgres,
}

impl VectorStoreKind {
    pub fn as_str(self) -> &'static str {
        match self {
            VectorStoreKind::CloudflareVectorize => "cloudflare_vectorize",
            VectorStoreKind::Postgres => "postgres",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            VectorStoreKind::CloudflareVectorize => "Cloudflare Vectorize",
            VectorStoreKind::Postgres => "Postgres (pgvector)",
        }
    }

    pub fn all() -> &'static [VectorStoreKind] {
        &[
            VectorStoreKind::CloudflareVectorize,
            VectorStoreKind::Postgres,
        ]
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cloudflare_vectorize" => Some(VectorStoreKind::CloudflareVectorize),
            "postgres" => Some(VectorStoreKind::Postgres),
            _ => None,
        }
    }
}
