use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::server::application::embedding::ports::Embedder;
use crate::server::application::AppError;
use crate::server::infrastructure::clients::{CloudflareApi, CLOUDFLARE_API_BASE};

pub struct WorkersAiEmbedder {
    api: Arc<CloudflareApi>,
}

impl WorkersAiEmbedder {
    pub fn new(api: Arc<CloudflareApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

#[derive(Debug, Deserialize)]
struct AiEnvelope {
    success: bool,
    #[serde(default)]
    errors: Option<Value>,
    #[serde(default)]
    result: Option<AiResult>,
}

#[derive(Debug, Deserialize)]
struct AiResult {
    #[serde(default)]
    data: Option<Vec<Vec<f32>>>,
}

#[async_trait]
impl Embedder for WorkersAiEmbedder {
    async fn embed_batch(
        &self,
        model: &str,
        _dimensions: u32,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let url = format!(
            "{}/accounts/{}/ai/run/{}",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
            model
        );
        let body = json!({ "text": texts });
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode embed body: {e}")))?;
        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "workers-ai",
            )
            .await?;
        let envelope: AiEnvelope = serde_json::from_str(&resp)
            .map_err(|e| AppError::Upstream(format!("parse workers-ai: {e}")))?;
        if !envelope.success {
            return Err(AppError::Upstream(format!(
                "workers-ai failed: {}",
                envelope
                    .errors
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "<no error body>".into())
            )));
        }
        let data = envelope
            .result
            .and_then(|r| r.data)
            .ok_or_else(|| AppError::Upstream("workers-ai missing result.data".into()))?;
        if data.len() != texts.len() {
            return Err(AppError::Upstream(format!(
                "workers-ai returned {} embeddings for {} inputs",
                data.len(),
                texts.len()
            )));
        }
        Ok(data)
    }
}
