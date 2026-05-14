use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::embedding::ports::Embedder;
use crate::server::application::AppError;
use crate::server::infrastructure::clients::OllamaApi;

pub struct OllamaEmbedder {
    api: Arc<OllamaApi>,
}

impl OllamaEmbedder {
    pub fn new(api: Arc<OllamaApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
    dimensions: u32,
}

#[derive(Debug, Deserialize)]
struct EmbedResult {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    async fn embed_batch(
        &self,
        model: &str,
        dimensions: u32,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let url = format!("{}/api/embed", self.api.base_url);
        let request = EmbedRequest {
            model: model.to_string(),
            input: texts.to_vec(),
            dimensions,
        };
        let body_bytes = serde_json::to_vec(&request)
            .map_err(|e| AppError::Internal(format!("encode embed body: {e}")))?;
        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "embedding",
            )
            .await?;
        let data: EmbedResult = serde_json::from_str(&resp)
            .map_err(|e| AppError::Upstream(format!("parse embedding: {e}")))?;
        if data.embeddings.len() != texts.len() {
            return Err(AppError::Upstream(format!(
                "ollama returned {} embeddings for {} inputs",
                data.embeddings.len(),
                texts.len()
            )));
        }
        Ok(data.embeddings)
    }
}
