use std::time::Duration;

use async_trait::async_trait;
use reqwest::{header::HeaderMap, Client, Method};

use crate::server::application::ports::HttpClient;
use crate::server::application::AppError;

pub struct ReqwestHttpClient {
    client: Client,
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get_text(&self, url: &str) -> Result<(u16, String), AppError> {
        self.request_text(Method::GET, url, HeaderMap::new(), None)
            .await
    }
}

impl ReqwestHttpClient {
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(Duration::from_mins(1))
            .build()
            .map_err(|e| AppError::Internal(format!("http client build: {e}")))?;
        Ok(Self { client })
    }

    pub fn raw(&self) -> &Client {
        &self.client
    }

    pub async fn request_text(
        &self,
        method: Method,
        url: &str,
        headers: HeaderMap,
        body: Option<Vec<u8>>,
    ) -> Result<(u16, String), AppError> {
        let mut builder = self.client.request(method, url).headers(headers);
        if let Some(b) = body {
            builder = builder.body(b);
        }
        let response = builder
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("send to {url}: {e}")))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| AppError::Upstream(format!("read body from {url}: {e}")))?;
        Ok((status, body))
    }
}
