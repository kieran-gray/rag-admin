use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::Value;

use crate::server::application::indexing::ports::KvStore;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::{CloudflareApi, CLOUDFLARE_API_BASE};

pub struct CloudflareKvStore {
    api: Arc<CloudflareApi>,
    namespace_id: String,
}

impl CloudflareKvStore {
    pub fn new(api: Arc<CloudflareApi>, namespace_id: String) -> Arc<Self> {
        Arc::new(Self { api, namespace_id })
    }
}

#[async_trait]
impl KvStore for CloudflareKvStore {
    async fn put_json(&self, key: &str, value: &Value) -> Result<(), AppError> {
        let url = self.value_url(key);
        let body_bytes = serde_json::to_vec(value)
            .map_err(|e| AppError::Internal(format!("encode kv value: {e}")))?;
        self.api
            .request(Method::PUT, &url, body_bytes, "application/json", "kv-put")
            .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), AppError> {
        let url = self.value_url(key);
        self.api
            .request(
                Method::DELETE,
                &url,
                vec![],
                "application/json",
                "kv-delete",
            )
            .await?;
        Ok(())
    }
}

impl CloudflareKvStore {
    fn value_url(&self, key: &str) -> String {
        format!(
            "{}/accounts/{}/storage/kv/namespaces/{}/values/{}",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
            self.namespace_id,
            urlencode(key)
        )
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(b as char);
        } else {
            _ = write!(out, "%{b:02X}");
        }
    }
    out
}
