use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKind {
    Cloudflare,
    Ollama,
}

impl AiProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AiProviderKind::Cloudflare => "cloudflare",
            AiProviderKind::Ollama => "ollama",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            AiProviderKind::Cloudflare => "Cloudflare",
            AiProviderKind::Ollama => "Ollama",
        }
    }

    pub fn model_id_well_formed(self, id: &str) -> bool {
        match self {
            AiProviderKind::Cloudflare => id.starts_with("@cf/"),
            AiProviderKind::Ollama => !id.is_empty() && !id.contains(char::is_whitespace),
        }
    }

    pub fn all() -> &'static [AiProviderKind] {
        &[AiProviderKind::Cloudflare, AiProviderKind::Ollama]
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cloudflare" => Some(AiProviderKind::Cloudflare),
            "ollama" => Some(AiProviderKind::Ollama),
            _ => None,
        }
    }
}

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
