use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResult {
    pub dims: usize,
    pub norm_a: f32,
    pub norm_b: f32,
    pub similarity: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EmbedderBackend {
    #[default]
    Cloudflare,
    Ollama,
}

impl EmbedderBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            EmbedderBackend::Cloudflare => "cloudflare",
            EmbedderBackend::Ollama => "ollama",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddingModel {
    pub backend: EmbedderBackend,
    pub id: String,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub dims: u32,
}

impl Default for EmbeddingModel {
    fn default() -> Self {
        Self {
            backend: EmbedderBackend::Cloudflare,
            id: "@cf/qwen/qwen3-embedding-0.6b".into(),
            dims: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub dims: u32,
}

pub const CLOUDFLARE_EMBEDDING_MODELS: &[CatalogEntry] = &[
    CatalogEntry {
        id: "@cf/baai/bge-small-en-v1.5",
        dims: 384,
    },
    CatalogEntry {
        id: "@cf/baai/bge-base-en-v1.5",
        dims: 768,
    },
    CatalogEntry {
        id: "@cf/baai/bge-large-en-v1.5",
        dims: 1024,
    },
    CatalogEntry {
        id: "@cf/baai/bge-m3",
        dims: 1024,
    },
    CatalogEntry {
        id: "@cf/qwen/qwen3-embedding-0.6b",
        dims: 1024,
    },
];

pub const OLLAMA_EMBEDDING_MODELS: &[CatalogEntry] = &[
    CatalogEntry {
        id: "qwen3-embedding:0.6b",
        dims: 1024,
    },
    CatalogEntry {
        id: "nomic-embed-text",
        dims: 768,
    },
    CatalogEntry {
        id: "mxbai-embed-large",
        dims: 1024,
    },
    CatalogEntry {
        id: "all-minilm",
        dims: 384,
    },
];

pub fn catalog_for_backend(backend: EmbedderBackend) -> &'static [CatalogEntry] {
    match backend {
        EmbedderBackend::Cloudflare => CLOUDFLARE_EMBEDDING_MODELS,
        EmbedderBackend::Ollama => OLLAMA_EMBEDDING_MODELS,
    }
}
