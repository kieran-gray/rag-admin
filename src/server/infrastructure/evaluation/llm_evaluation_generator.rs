use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::server::application::evaluation::ports::{
    EvaluationGenerator, EvaluationPrompt, GeneratedEvaluationQuestion,
};
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::AppError;
use crate::server::domain::evaluation::question::QuestionDimensions;

pub struct LlmEvaluationGenerator {
    generation_service: Arc<GenerationService>,
}

impl LlmEvaluationGenerator {
    pub fn new(generation_service: Arc<GenerationService>) -> Arc<Self> {
        Arc::new(Self { generation_service })
    }
}

#[derive(Debug, Deserialize)]
struct GeneratedQuestionWire {
    question: String,
}

#[async_trait]
impl EvaluationGenerator for LlmEvaluationGenerator {
    async fn generate_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
        _dimensions: QuestionDimensions,
    ) -> Result<GeneratedEvaluationQuestion, AppError> {
        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: prompt.system,
                    user: prompt.user,
                    temperature: 1.0,
                    response_format: GenerationResponseFormat::Json,
                },
            )
            .await?;
        parse_generated_question(&response.content)
    }
}

fn parse_generated_question(content: &str) -> Result<GeneratedEvaluationQuestion, AppError> {
    let json_text = strip_code_fence(content.trim());
    let parsed: GeneratedQuestionWire = serde_json::from_str(json_text)
        .map_err(|e| AppError::Upstream(format!("parse generated question JSON: {e}")))?;
    let question = parsed.question.trim().to_string();
    if question.is_empty() {
        return Err(AppError::Upstream(
            "generated question was empty".to_string(),
        ));
    }
    Ok(GeneratedEvaluationQuestion { question })
}

fn strip_code_fence(content: &str) -> &str {
    let Some(stripped) = content.strip_prefix("```") else {
        return content;
    };
    let stripped = stripped
        .strip_prefix("json")
        .unwrap_or(stripped)
        .trim_start();
    stripped.strip_suffix("```").unwrap_or(stripped).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json() {
        let parsed = parse_generated_question(r#"{"question":"What happened?"}"#).unwrap();
        assert_eq!(parsed.question, "What happened?");
    }

    #[test]
    fn parses_fenced_json() {
        let parsed =
            parse_generated_question("```json\n{\"question\":\"What happened?\"}\n```").unwrap();
        assert_eq!(parsed.question, "What happened?");
    }

    #[test]
    fn rejects_empty_question() {
        let err = parse_generated_question(r#"{"question":"   "}"#).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }
}
