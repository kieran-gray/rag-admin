use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::server::application::configuration::ports::EvaluationDefaultsStore;
use crate::server::application::AppError;
use crate::shared::SettingsDto;

pub struct FileEvaluationDefaultsStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileEvaluationDefaultsStore {
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            path,
            lock: Mutex::new(()),
        })
    }
}

#[async_trait]
impl EvaluationDefaultsStore for FileEvaluationDefaultsStore {
    async fn load(&self) -> Result<SettingsDto, AppError> {
        let _guard = self.lock.lock().await;
        match tokio::fs::read(&self.path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                AppError::Internal(format!(
                    "parse evaluation defaults at {}: {e}",
                    self.path.display()
                ))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SettingsDto::default()),
            Err(e) => Err(AppError::Internal(format!(
                "read evaluation defaults at {}: {e}",
                self.path.display()
            ))),
        }
    }

    async fn save(&self, settings: SettingsDto) -> Result<(), AppError> {
        let _guard = self.lock.lock().await;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("create dir {}: {e}", parent.display())))?;
        }
        let bytes = serde_json::to_vec_pretty(&settings)
            .map_err(|e| AppError::Internal(format!("encode evaluation defaults: {e}")))?;
        tokio::fs::write(&self.path, bytes).await.map_err(|e| {
            AppError::Internal(format!(
                "write evaluation defaults at {}: {e}",
                self.path.display()
            ))
        })
    }
}
