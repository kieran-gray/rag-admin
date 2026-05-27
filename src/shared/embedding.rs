use serde::{Deserialize, Serialize};

use crate::shared::contracts::Timings;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EmbedInputType {
    #[default]
    Plain,
    Query,
    Passage,
}

impl EmbedInputType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Query => "query",
            Self::Passage => "passage",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Plain => "Plain",
            Self::Query => "Query",
            Self::Passage => "Passage",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResult {
    pub dims: usize,
    pub norm_a: f32,
    pub norm_b: f32,
    pub similarity: f32,
    #[serde(default)]
    pub timings: Timings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedManyCandidate {
    pub text_preview: String,
    pub norm: f32,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedManyResult {
    pub dims: usize,
    pub query_norm: f32,
    pub candidates: Vec<EmbedManyCandidate>,
    #[serde(default)]
    pub timings: Timings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedMatrixResult {
    pub dims: usize,
    pub previews: Vec<String>,
    pub norms: Vec<f32>,
    pub matrix: Vec<Vec<f32>>,
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
