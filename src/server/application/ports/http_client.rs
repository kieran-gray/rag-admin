use async_trait::async_trait;

use crate::server::application::AppError;

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get_text(&self, url: &str) -> Result<(u16, String), AppError>;
}
