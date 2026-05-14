use std::sync::Arc;

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;

use crate::server::application::AppError;
use crate::server::infrastructure::http_client::ReqwestHttpClient;

pub struct OllamaApi {
    http: Arc<ReqwestHttpClient>,
    pub base_url: String,
}

impl OllamaApi {
    pub fn new(http: Arc<ReqwestHttpClient>, base_url: String) -> Self {
        Self { http, base_url }
    }

    fn headers(content_type: &str) -> Result<HeaderMap, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_str(content_type)
                .map_err(|e| AppError::Internal(format!("content-type header: {e}")))?,
        );
        Ok(headers)
    }

    pub async fn request(
        &self,
        method: Method,
        url: &str,
        body: Vec<u8>,
        content_type: &str,
        label: &str,
    ) -> Result<String, AppError> {
        let headers = Self::headers(content_type)?;
        let body_opt = if body.is_empty() { None } else { Some(body) };
        let (status, body_text) = self
            .http
            .request_text(method, url, headers, body_opt)
            .await?;
        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!(
                "{label}: {status} — {}",
                truncate(&body_text, 500)
            )));
        }
        Ok(body_text)
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}
