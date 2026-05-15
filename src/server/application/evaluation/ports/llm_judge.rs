use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;

#[derive(Debug, Clone)]
pub struct JudgeVerdict {
    pub sufficient: bool,
    pub confidence: f32,
    pub reason: String,
}

#[async_trait]
pub trait LlmJudge: Send + Sync {
    async fn judge_context(
        &self,
        generation_model_id: Uuid,
        question: &str,
        retrieved_text: &str,
    ) -> Result<JudgeVerdict, AppError>;
}
