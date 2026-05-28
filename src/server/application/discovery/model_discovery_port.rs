use async_trait::async_trait;

use crate::server::application::AppError;
use crate::shared::contracts::DiscoveredModelDto;
use crate::shared::reference_data::ModelCapability;

#[async_trait]
pub trait ModelDiscoveryPort: Send + Sync {
    async fn discover(
        &self,
        capability: ModelCapability,
    ) -> Result<Vec<DiscoveredModelDto>, AppError>;
}
