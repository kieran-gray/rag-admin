use async_trait::async_trait;

use crate::server::application::AppError;
use crate::shared::contracts::DiscoveredIndexDto;

#[async_trait]
pub trait VectorIndexDiscoveryPort: Send + Sync {
    async fn discover(&self) -> Result<Vec<DiscoveredIndexDto>, AppError>;
}
