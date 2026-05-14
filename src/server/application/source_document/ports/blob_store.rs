use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::domain::source_document::version::ContentHash;

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn put(&self, content: &[u8]) -> Result<ContentHash, AppError>;

    async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>, AppError>;

    async fn delete(&self, hash: &ContentHash) -> Result<(), AppError>;
}
