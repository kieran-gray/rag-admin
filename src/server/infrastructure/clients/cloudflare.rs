use std::sync::Arc;

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;

use crate::server::application::AppError;
use crate::server::infrastructure::http_client::ReqwestHttpClient;
use crate::server::setup::config::CloudflareConfig;

pub const CLOUDFLARE_API_BASE: &str = "https://api.cloudflare.com/client/v4";

pub struct CloudflareApi {
    http: Arc<ReqwestHttpClient>,
    config: CloudflareConfig,
}

impl CloudflareApi {
    pub fn new(http: Arc<ReqwestHttpClient>, config: CloudflareConfig) -> Self {
        Self { http, config }
    }

    pub fn account_id(&self) -> &str {
        &self.config.account_id
    }

    pub fn api_token(&self) -> &str {
        &self.config.api_token
    }

    fn auth_headers(token: &str, content_type: &str) -> Result<HeaderMap, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| AppError::Internal(format!("auth header: {e}")))?,
        );
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
        let headers = Self::auth_headers(self.api_token(), content_type)?;
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
