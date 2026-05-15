use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;

#[derive(Debug, Clone)]
pub struct EvaluationPrompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEvaluationQuestion {
    pub question: String,
    pub references: Vec<String>,
    pub category: crate::server::domain::evaluation::question::QuestionCategory,
}

#[async_trait]
pub trait EvaluationGenerator: Send + Sync {
    async fn generate_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
        category: crate::server::domain::evaluation::question::QuestionCategory,
    ) -> Result<GeneratedEvaluationQuestion, AppError>;

    async fn paraphrase_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
    ) -> Result<String, AppError>;
}
