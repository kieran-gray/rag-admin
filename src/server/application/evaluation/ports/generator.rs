use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::evaluation::question::QuestionDimensions;

#[derive(Debug, Clone)]
pub struct EvaluationPrompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEvaluationQuestion {
    pub question: String,
}

#[async_trait]
pub trait EvaluationGenerator: Send + Sync {
    async fn generate_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
        dimensions: QuestionDimensions,
    ) -> Result<GeneratedEvaluationQuestion, AppError>;
}
