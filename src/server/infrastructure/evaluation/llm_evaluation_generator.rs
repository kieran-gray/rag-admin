use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::server::application::evaluation::ports::{
    EvaluationGenerator, EvaluationPrompt, GeneratedEvaluationQuestion,
};
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::AppError;
use crate::server::domain::evaluation::question::QuestionCategory;

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
    references: Vec<String>,
}

#[async_trait]
impl EvaluationGenerator for LlmEvaluationGenerator {
    async fn generate_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
        category: QuestionCategory,
    ) -> Result<GeneratedEvaluationQuestion, AppError> {
        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: prompt.system,
                    user: prompt.user,
                    temperature: 1.0,
                    response_format: GenerationResponseFormat::Text,
                },
            )
            .await?;

        parse_generated_question(&response.content, category)
    }

    async fn paraphrase_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
    ) -> Result<String, AppError> {
        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: prompt.system,
                    user: prompt.user,
                    temperature: 1.0,
                    response_format: GenerationResponseFormat::Text,
                },
            )
            .await?;
        parse_paraphrase(&response.content)
    }
}

#[derive(Debug, Deserialize)]
struct ParaphraseWire {
    paraphrase: String,
}

fn parse_paraphrase(content: &str) -> Result<String, AppError> {
    let text = strip_code_fence(content.trim());
    let wire: ParaphraseWire = serde_json::from_str(text)
        .map_err(|e| AppError::Upstream(format!("parse paraphrase JSON: {e}")))?;
    let trimmed = wire.paraphrase.trim();
    if trimmed.is_empty() {
        return Err(AppError::Upstream("paraphrase was empty".into()));
    }
    Ok(trimmed.to_string())
}

fn parse_generated_question(
    content: &str,
    category: QuestionCategory,
) -> Result<GeneratedEvaluationQuestion, AppError> {
    let json_text = strip_code_fence(content.trim());
    let value: Value = serde_json::from_str(json_text)
        .map_err(|e| AppError::Upstream(format!("parse generated question JSON: {e}")))?;
    let parsed: GeneratedQuestionWire = serde_json::from_value(value)
        .map_err(|e| AppError::Upstream(format!("generated question shape was invalid: {e}")))?;

    if parsed.question.trim().is_empty() {
        return Err(AppError::Upstream(
            "generated question was empty".to_string(),
        ));
    }

    let references: Vec<String> = parsed
        .references
        .into_iter()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .collect();

    if references.is_empty() && category != QuestionCategory::Trick {
        return Err(AppError::Upstream(
            "generated question had no references".to_string(),
        ));
    }

    Ok(GeneratedEvaluationQuestion {
        question: parsed.question.trim().to_string(),
        references,
        category,
    })
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

    use crate::server::domain::evaluation::question::QuestionCategory;

    #[test]
    fn parses_plain_json_response() {
        let parsed = parse_generated_question(
            r#"{"question":"What happened?","references":["The thing happened."]}"#,
            QuestionCategory::FactRetrieval,
        )
        .unwrap();

        assert_eq!(parsed.question, "What happened?");
        assert_eq!(parsed.references, vec!["The thing happened."]);
        assert_eq!(parsed.category, QuestionCategory::FactRetrieval);
    }

    #[test]
    fn parses_fenced_json_response() {
        let parsed = parse_generated_question(
            "```json\n{\"question\":\"What happened?\",\"references\":[\"The thing happened.\"]}\n```",
            QuestionCategory::Architecture,
        )
        .unwrap();

        assert_eq!(parsed.question, "What happened?");
        assert_eq!(parsed.category, QuestionCategory::Architecture);
    }

    #[test]
    fn parses_paraphrase_plain_json() {
        let v = parse_paraphrase(r#"{"paraphrase":"section max tokens how many?"}"#).unwrap();
        assert_eq!(v, "section max tokens how many?");
    }

    #[test]
    fn parses_paraphrase_fenced_json() {
        let v = parse_paraphrase("```json\n{\"paraphrase\":\"wht is max chunk tokens?\"}\n```")
            .unwrap();
        assert_eq!(v, "wht is max chunk tokens?");
    }

    #[test]
    fn rejects_empty_paraphrase() {
        assert!(parse_paraphrase(r#"{"paraphrase":"   "}"#).is_err());
    }

    #[test]
    fn accepts_trick_question_with_empty_references() {
        let parsed = parse_generated_question(
            r#"{"question":"Which benchmark was used to validate the chunker?","references":[]}"#,
            QuestionCategory::Trick,
        )
        .unwrap();

        assert_eq!(parsed.category, QuestionCategory::Trick);
        assert!(parsed.references.is_empty());
    }

    #[test]
    fn still_rejects_non_trick_questions_with_empty_references() {
        let err = parse_generated_question(
            r#"{"question":"What is X?","references":[]}"#,
            QuestionCategory::FactRetrieval,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no references"));
    }
}
