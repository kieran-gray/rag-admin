use std::time::Duration;

use reqwest::{header::HeaderMap, Client, Method};

use crate::server::application::AppError;

pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
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

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new().expect("default reqwest client")
    }
}
