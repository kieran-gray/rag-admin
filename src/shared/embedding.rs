use serde::{Deserialize, Serialize};

use crate::shared::contracts::Timings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResult {
    pub dims: usize,
    pub norm_a: f32,
    pub norm_b: f32,
    pub similarity: f32,
    #[serde(default)]
    pub timings: Timings,
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
