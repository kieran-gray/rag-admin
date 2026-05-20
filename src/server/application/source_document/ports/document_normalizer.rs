use async_trait::async_trait;

use crate::server::application::AppError;

use super::content_type::ContentType;
use super::normalized_document::NormalizedDocument;
use super::raw_document::RawDocument;

#[async_trait]
pub trait DocumentNormalizer: Send + Sync {
    fn content_type(&self) -> ContentType;

    async fn normalize(&self, raw: RawDocument) -> Result<NormalizedDocument, AppError>;
}
