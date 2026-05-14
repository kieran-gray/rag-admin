use async_trait::async_trait;
use serde_json::Value;

use crate::server::application::AppError;

#[async_trait]
pub trait KvStore: Send + Sync {
    async fn put_json(&self, key: &str, value: &Value) -> Result<(), AppError>;
}
