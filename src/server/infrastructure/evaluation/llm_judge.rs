use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::server::application::evaluation::ports::{JudgeVerdict, LlmJudge};
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::AppError;

const JUDGE_SYSTEM_PROMPT: &str = "You are a strict reviewer of retrieval quality. \
Given a question and a passage of retrieved text, decide whether the passage contains \
enough information to answer the question. Return only JSON: \
{\"sufficient\": bool, \"confidence\": 0-1 float, \"reason\": string}.";

pub struct LlmJudgeAdapter {
    generation_service: Arc<GenerationService>,
}

impl LlmJudgeAdapter {
    pub fn new(generation_service: Arc<GenerationService>) -> Arc<Self> {
        Arc::new(Self { generation_service })
    }
}

#[derive(Debug, Deserialize)]
struct VerdictWire {
    sufficient: bool,
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    reason: String,
}

#[async_trait]
impl LlmJudge for LlmJudgeAdapter {
    async fn judge_context(
        &self,
        generation_model_id: Uuid,
        question: &str,
        retrieved_text: &str,
    ) -> Result<JudgeVerdict, AppError> {
        let user = format!(
            "Question:\n{question}\n\nRetrieved passage:\n{retrieved_text}\n\nReturn only JSON."
        );
        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: JUDGE_SYSTEM_PROMPT.to_string(),
                    user,
                    temperature: 0.0,
                    response_format: GenerationResponseFormat::Text,
                },
            )
            .await?;
        parse_verdict(&response.content)
    }
}

fn parse_verdict(content: &str) -> Result<JudgeVerdict, AppError> {
    let text = strip_code_fence(content.trim());
    let wire: VerdictWire = serde_json::from_str(text)
        .map_err(|e| AppError::Upstream(format!("parse judge verdict JSON: {e}")))?;
    Ok(JudgeVerdict {
        sufficient: wire.sufficient,
        confidence: wire.confidence.clamp(0.0, 1.0),
        reason: wire.reason,
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
    fn parses_plain_verdict() {
        let v =
            parse_verdict(r#"{"sufficient":true,"confidence":0.92,"reason":"covers the fact"}"#)
                .unwrap();
        assert!(v.sufficient);
        assert!((v.confidence - 0.92).abs() < 1e-4);
        assert_eq!(v.reason, "covers the fact");
    }

    #[test]
    fn parses_fenced_verdict() {
        let v = parse_verdict(
            "```json\n{\"sufficient\":false,\"confidence\":0.1,\"reason\":\"empty\"}\n```",
        )
        .unwrap();
        assert!(!v.sufficient);
        assert!((v.confidence - 0.1).abs() < 1e-4);
    }

    #[test]
    fn clamps_confidence_to_unit_range() {
        let v = parse_verdict(r#"{"sufficient":true,"confidence":2.5,"reason":""}"#).unwrap();
        assert!((v.confidence - 1.0).abs() < 1e-6);
    }
}
