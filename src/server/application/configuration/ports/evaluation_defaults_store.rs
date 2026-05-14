use async_trait::async_trait;

use crate::server::application::AppError;
use crate::shared::SettingsDto;

#[async_trait]
pub trait EvaluationDefaultsStore: Send + Sync {
    async fn load(&self) -> Result<SettingsDto, AppError>;
    async fn save(&self, settings: SettingsDto) -> Result<(), AppError>;
}
