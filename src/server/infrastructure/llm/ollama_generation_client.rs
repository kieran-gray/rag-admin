use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::ports::{
    GenerationClient, GenerationRequest, GenerationResponse, GenerationResponseFormat,
};
use crate::server::application::AppError;
use crate::server::infrastructure::http_client::ReqwestHttpClient;

pub struct OllamaGenerationClient {
    http: Arc<ReqwestHttpClient>,
    base_url: String,
    num_ctx: u32,
}

impl OllamaGenerationClient {
    pub fn new(http: Arc<ReqwestHttpClient>, base_url: String, num_ctx: u32) -> Arc<Self> {
        Arc::new(Self {
            http,
            base_url,
            num_ctx,
        })
    }
}

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'static str>,
    options: OllamaGenerateOptions,
}

#[derive(Serialize)]
struct OllamaGenerateOptions {
    temperature: f32,
    num_ctx: u32,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: Option<String>,
}

#[async_trait]
impl GenerationClient for OllamaGenerationClient {
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AppError> {
        let base_url = self.base_url.trim().trim_end_matches('/');

        let request = OllamaGenerateRequest {
            model: request.model,
            prompt: request.user,
            system: request.system,
            stream: false,
            format: match request.response_format {
                GenerationResponseFormat::Text => None,
                GenerationResponseFormat::Json => Some("json"),
            },
            options: OllamaGenerateOptions {
                temperature: request.temperature,
                num_ctx: self.num_ctx,
            },
        };

        let body = serde_json::to_vec(&request)
            .map_err(|e| AppError::Internal(format!("encode Ollama generate request: {e}")))?;

        let (status, body_text) = self
            .http
            .request_text(
                Method::POST,
                &format!("{}/api/generate", base_url),
                json_headers(),
                Some(body),
            )
            .await?;

        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!(
                "ollama generate: {status} - {}",
                truncate(&body_text, 500)
            )));
        }

        let response: OllamaGenerateResponse = serde_json::from_str(&body_text)
            .map_err(|e| AppError::Upstream(format!("parse Ollama generate response: {e}")))?;
        let content = response
            .response
            .ok_or_else(|| AppError::Upstream("Ollama generate missing response".into()))?;

        Ok(GenerationResponse { content })
    }
}

fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "..."
    }
}
