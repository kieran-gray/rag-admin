use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::server::application::comprehension::ports::{InsightSynthRequest, InsightSynthesizer};
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;
use crate::server::domain::comprehension::insight::Insight;
use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::comprehension::span::Span;

const PROMPT_TEMPLATE: &str = include_str!("../../application/comprehension/prompts/insight.txt");

pub struct LlmInsightSynthesizer {
    generation_service: Arc<GenerationService>,
    id_generator: Arc<dyn IdGenerator>,
}

impl LlmInsightSynthesizer {
    pub fn new(
        generation_service: Arc<GenerationService>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            generation_service,
            id_generator,
        })
    }
}

#[derive(Debug, Deserialize)]
struct InsightsWire {
    insights: Vec<InsightWire>,
}

#[derive(Debug, Deserialize)]
struct InsightWire {
    kind: String,
    summary: String,
    #[serde(default)]
    evidence_observation_ids: Vec<u32>,
    #[serde(default)]
    evidence_thread_ids: Vec<u32>,
}

#[async_trait]
impl InsightSynthesizer for LlmInsightSynthesizer {
    async fn synthesize(
        &self,
        generation_model_id: Uuid,
        req: InsightSynthRequest<'_>,
    ) -> Result<Vec<Insight>, AppError> {
        if req.observations.is_empty() && req.threads.is_empty() {
            return Ok(Vec::new());
        }

        let observations_text = req
            .observations
            .iter()
            .enumerate()
            .map(|(i, o)| {
                format!(
                    "{i}: [{kind}] {summary}",
                    kind = o.kind,
                    summary = o.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let threads_text = req
            .threads
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "{i}: [{kind}] {summary}",
                    kind = t.kind,
                    summary = t.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let user = PROMPT_TEMPLATE
            .replace("{observations}", &observations_text)
            .replace("{threads}", &threads_text);

        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: "You synthesize whole-document insights from observations and threads, returning only JSON.".into(),
                    user,
                    temperature: 0.4,
                    response_format: GenerationResponseFormat::Json,
                },
            )
            .await?;

        let wire = parse_insights(&response.content)?;
        let mut insights = Vec::new();
        for raw in wire.insights {
            let kind = raw.kind.trim().to_lowercase();
            let summary = raw.summary.trim().to_string();
            if kind.is_empty() || summary.is_empty() {
                continue;
            }
            let mut evidence: Vec<MapItemRef> = Vec::new();
            let mut spans: Vec<Span> = Vec::new();
            for id in raw.evidence_observation_ids {
                if let Some(obs) = req.observations.get(id as usize) {
                    evidence.push(MapItemRef::Observation {
                        map_id: req.map_id,
                        observation_id: obs.observation_id,
                    });
                    spans.extend(obs.spans.iter().copied());
                }
            }
            for id in raw.evidence_thread_ids {
                if let Some(thread) = req.threads.get(id as usize) {
                    evidence.push(MapItemRef::Thread {
                        map_id: req.map_id,
                        thread_id: thread.thread_id,
                    });
                    spans.extend(thread.spans.iter().copied());
                }
            }
            if evidence.is_empty() {
                continue;
            }
            insights.push(Insight {
                insight_id: self.id_generator.new_uuid(),
                kind,
                summary,
                evidence,
                spans: Span::normalize(spans),
            });
        }
        Ok(insights)
    }
}

fn parse_insights(content: &str) -> Result<InsightsWire, AppError> {
    let text = strip_code_fence(content.trim());
    serde_json::from_str(text).map_err(|e| AppError::Upstream(format!("parse insights JSON: {e}")))
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
    fn parses_insights_payload() {
        let wire = parse_insights(
            r#"{"insights":[{"kind":"theme","summary":"X","evidence_observation_ids":[0],"evidence_thread_ids":[]}]}"#,
        )
        .unwrap();
        assert_eq!(wire.insights.len(), 1);
    }

    #[test]
    fn parses_with_missing_optional_fields() {
        let wire = parse_insights(r#"{"insights":[{"kind":"theme","summary":"X"}]}"#).unwrap();
        assert_eq!(wire.insights.len(), 1);
        assert!(wire.insights[0].evidence_observation_ids.is_empty());
    }
}
