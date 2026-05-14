use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::ports::{
    ChatClient, ChatRequest as AppChatRequest, ChatResponse as AppChatResponse, ChatResponseFormat,
};
use crate::server::application::AppError;
use crate::server::infrastructure::http_client::ReqwestHttpClient;

pub struct OllamaChatClient {
    http: Arc<ReqwestHttpClient>,
    base_url: String,
}

impl OllamaChatClient {
    pub fn new(http: Arc<ReqwestHttpClient>, base_url: String) -> Arc<Self> {
        Arc::new(Self { http, base_url })
    }
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'static str>,
    options: OllamaChatOptions,
}

#[derive(Serialize)]
struct OllamaChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct OllamaChatOptions {
    temperature: f32,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaChatResponseMessage>,
}

#[derive(Deserialize)]
struct OllamaChatResponseMessage {
    content: String,
}

#[async_trait]
impl ChatClient for OllamaChatClient {
    async fn chat(&self, request: AppChatRequest) -> Result<AppChatResponse, AppError> {
        let base_url = self.base_url.trim().trim_end_matches('/');

        let request = OllamaChatRequest {
            model: request.model,
            messages: vec![
                OllamaChatMessage {
                    role: "system",
                    content: request.system,
                },
                OllamaChatMessage {
                    role: "user",
                    content: request.user,
                },
            ],
            stream: false,
            format: match request.response_format {
                ChatResponseFormat::Text => None,
                ChatResponseFormat::Json => Some("json"),
            },
            options: OllamaChatOptions {
                temperature: request.temperature,
            },
        };

        let body = serde_json::to_vec(&request)
            .map_err(|e| AppError::Internal(format!("encode Ollama chat request: {e}")))?;

        let (status, body_text) = self
            .http
            .request_text(
                Method::POST,
                &format!("{}/api/chat", base_url),
                json_headers(),
                Some(body),
            )
            .await?;

        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!(
                "ollama chat: {status} - {}",
                truncate(&body_text, 500)
            )));
        }

        let response: OllamaChatResponse = serde_json::from_str(&body_text)
            .map_err(|e| AppError::Upstream(format!("parse Ollama chat response: {e}")))?;
        let content = response
            .message
            .map(|m| m.content)
            .ok_or_else(|| AppError::Upstream("Ollama chat missing message.content".into()))?;

        Ok(AppChatResponse { content })
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
