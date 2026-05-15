use async_trait::async_trait;

use crate::contracts::SettingsDto;
use crate::server::application::AppError;

#[async_trait]
pub trait EvaluationDefaultsStore: Send + Sync {
    async fn load(&self) -> Result<SettingsDto, AppError>;
    async fn save(&self, settings: SettingsDto) -> Result<(), AppError>;
}
