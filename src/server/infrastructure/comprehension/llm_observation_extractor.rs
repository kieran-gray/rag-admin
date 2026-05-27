use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::server::application::comprehension::ports::{
    snap_to_segments, ObservationContext, ObservationExtractor, TextSegmenter,
};
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::span::Span;

const PROMPT_TEMPLATE: &str =
    include_str!("../../application/comprehension/prompts/observation.txt");

pub struct LlmObservationExtractor {
    generation_service: Arc<GenerationService>,
    id_generator: Arc<dyn IdGenerator>,
    segmenter: Arc<dyn TextSegmenter>,
}

impl LlmObservationExtractor {
    pub fn new(
        generation_service: Arc<GenerationService>,
        id_generator: Arc<dyn IdGenerator>,
        segmenter: Arc<dyn TextSegmenter>,
    ) -> Arc<Self> {
        Arc::new(Self {
            generation_service,
            id_generator,
            segmenter,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ObservationsWire {
    observations: Vec<ObservationWire>,
}

#[derive(Debug, Deserialize)]
struct ObservationWire {
    kind: String,
    summary: String,
    spans: Vec<SpanWire>,
}

#[derive(Debug, Deserialize)]
struct SpanWire {
    char_start: u32,
    char_end: u32,
}

#[async_trait]
impl ObservationExtractor for LlmObservationExtractor {
    async fn extract(
        &self,
        generation_model_id: Uuid,
        ctx: ObservationContext<'_>,
    ) -> Result<Vec<Observation>, AppError> {
        let user = PROMPT_TEMPLATE
            .replace("{roles}", ctx.suggested_roles_summary)
            .replace("{heading}", &ctx.chunk.heading)
            .replace("{chunk_text}", &ctx.chunk.text);

        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: "You extract atomic, grounded observations from a chunk of text and return only JSON.".into(),
                    user,
                    temperature: 0.2,
                    response_format: GenerationResponseFormat::Json,
                },
            )
            .await?;

        let wire = parse_observations(&response.content)?;
        let chunk_text_len = ctx.chunk.text.chars().count() as u32;
        let chunk_byte_len = ctx.chunk.text.len() as u32;
        let segments = self.segmenter.segments(&ctx.chunk.text);
        let mut out = Vec::new();
        for raw in wire.observations {
            let kind = raw.kind.trim().to_lowercase();
            let summary = raw.summary.trim().to_string();
            if kind.is_empty() || summary.is_empty() {
                continue;
            }
            let raw_spans: Vec<Span> = raw
                .spans
                .into_iter()
                .filter_map(|s| {
                    if s.char_end <= s.char_start {
                        return None;
                    }
                    let max = chunk_text_len.max(chunk_byte_len);
                    if s.char_end > max {
                        return None;
                    }
                    Some(Span {
                        document_id: ctx.document_id,
                        char_start: ctx.chunk.char_start.saturating_add(s.char_start),
                        char_end: ctx.chunk.char_start.saturating_add(s.char_end),
                    })
                })
                .map(|s| snap_to_segments(s, &segments, ctx.chunk.char_start))
                .collect();
            let spans = Span::normalize(raw_spans);
            if spans.is_empty() {
                continue;
            }
            out.push(Observation {
                observation_id: self.id_generator.new_uuid(),
                chunk_sequence: ctx.chunk.sequence,
                kind,
                summary,
                spans,
            });
        }
        Ok(out)
    }
}

fn parse_observations(content: &str) -> Result<ObservationsWire, AppError> {
    let text = strip_code_fence(content.trim());
    serde_json::from_str(text)
        .map_err(|e| AppError::Upstream(format!("parse observations JSON: {e}")))
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
    fn parses_observations_payload() {
        let wire = parse_observations(
            r#"{"observations":[{"kind":"entity","summary":"X","spans":[{"char_start":0,"char_end":4}]}]}"#,
        )
        .unwrap();
        assert_eq!(wire.observations.len(), 1);
    }

    #[test]
    fn parses_fenced_payload() {
        let wire = parse_observations("```json\n{\"observations\":[]}\n```").unwrap();
        assert!(wire.observations.is_empty());
    }
}
