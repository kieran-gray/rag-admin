use crate::shared::embedding::EmbedderBackend;

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
