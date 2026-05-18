use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use tracing::info;

use crate::server::application::llm::ports::{
    GenerationClient, GenerationRequest, GenerationResponse, GenerationResponseFormat,
};
use crate::server::application::AppError;
use crate::server::infrastructure::clients::{CloudflareApi, CLOUDFLARE_API_BASE};

pub struct WorkersAiGenerationClient {
    api: Arc<CloudflareApi>,
}

impl WorkersAiGenerationClient {
    pub fn new(api: Arc<CloudflareApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

#[derive(Serialize)]
struct WorkersAiChatRequest {
    messages: Vec<WorkersAiChatMessage>,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<WorkersAiResponseFormat>,
}

#[derive(Serialize)]
struct WorkersAiChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct WorkersAiResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Deserialize)]
struct WorkersAiEnvelope {
    success: bool,
    #[serde(default)]
    errors: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
}

fn extract_content(result: &Value) -> Option<String> {
    info!("{:?}", result);
    if let Some(s) = result.as_str() {
        return Some(s.to_string());
    }
    if let Some(s) = result
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
    {
        return Some(s.to_string());
    }
    result
        .get("response")
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[async_trait]
impl GenerationClient for WorkersAiGenerationClient {
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AppError> {
        let url = format!(
            "{}/accounts/{}/ai/run/{}",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
            request.model
        );

        let body = WorkersAiChatRequest {
            messages: vec![
                WorkersAiChatMessage {
                    role: "system",
                    content: request.system,
                },
                WorkersAiChatMessage {
                    role: "user",
                    content: request.user,
                },
            ],
            temperature: request.temperature,
            stream: false,
            response_format: match request.response_format {
                GenerationResponseFormat::Text => None,
                GenerationResponseFormat::Json => Some(WorkersAiResponseFormat {
                    kind: "json_object",
                }),
            },
        };

        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode workers-ai generate body: {e}")))?;

        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "workers-ai generate",
            )
            .await?;

        let envelope: WorkersAiEnvelope = serde_json::from_str(&resp)
            .map_err(|e| AppError::Upstream(format!("parse workers-ai generate: {e}")))?;

        if !envelope.success {
            return Err(AppError::Upstream(format!(
                "workers-ai generate failed: {}",
                envelope
                    .errors
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "<no error body>".into())
            )));
        }

        let content = envelope
            .result
            .as_ref()
            .and_then(extract_content)
            .ok_or_else(|| {
                AppError::Upstream(format!(
                    "workers-ai response had no content; raw result: {}",
                    envelope
                        .result
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "<missing>".into())
                ))
            })?;

        Ok(GenerationResponse { content })
    }
}
