use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::server::application::evaluation::ports::{
    EvaluationGenerator, EvaluationPrompt, GeneratedEvaluationQuestion,
};
use crate::server::application::llm::ChatService;
use crate::server::application::ports::ChatResponseFormat;
use crate::server::application::AppError;

pub struct ChatBasedEvaluationGenerator {
    chat_service: Arc<ChatService>,
}

impl ChatBasedEvaluationGenerator {
    pub fn new(chat_service: Arc<ChatService>) -> Arc<Self> {
        Arc::new(Self { chat_service })
    }
}

#[derive(Debug, Deserialize)]
struct GeneratedQuestionWire {
    question: String,
    references: Vec<String>,
}

#[async_trait]
impl EvaluationGenerator for ChatBasedEvaluationGenerator {
    async fn generate_question(
        &self,
        generation_model_id: Uuid,
        prompt: EvaluationPrompt,
    ) -> Result<GeneratedEvaluationQuestion, AppError> {
        let response = self
            .chat_service
            .chat(
                generation_model_id,
                crate::server::application::llm::service::ChatPrompt {
                    system: prompt.system,
                    user: prompt.user,
                    temperature: 1.0,
                    response_format: ChatResponseFormat::Json,
                },
            )
            .await?;

        parse_generated_question(&response.content)
    }
}

fn parse_generated_question(content: &str) -> Result<GeneratedEvaluationQuestion, AppError> {
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
    if parsed.references.is_empty() {
        return Err(AppError::Upstream(
            "generated question had no references".to_string(),
        ));
    }

    let references: Vec<String> = parsed
        .references
        .into_iter()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .collect();
    if references.is_empty() {
        return Err(AppError::Upstream(
            "generated references were empty".to_string(),
        ));
    }

    Ok(GeneratedEvaluationQuestion {
        question: parsed.question.trim().to_string(),
        references,
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

    #[test]
    fn parses_plain_json_response() {
        let parsed = parse_generated_question(
            r#"{"question":"What happened?","references":["The thing happened."]}"#,
        )
        .unwrap();

        assert_eq!(parsed.question, "What happened?");
        assert_eq!(parsed.references, vec!["The thing happened."]);
    }

    #[test]
    fn parses_fenced_json_response() {
        let parsed = parse_generated_question(
            "```json\n{\"question\":\"What happened?\",\"references\":[\"The thing happened.\"]}\n```",
        )
        .unwrap();

        assert_eq!(parsed.question, "What happened?");
    }
}
