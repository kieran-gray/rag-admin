use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::server::application::comprehension::ports::{
    ThreadSynthRequest, ThreadSynthResult, ThreadSynthesizer,
};
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;
use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::comprehension::span::Span;
use crate::server::domain::comprehension::thread::Thread;

const PROMPT_TEMPLATE: &str = include_str!("../../application/comprehension/prompts/thread.txt");

pub struct LlmThreadSynthesizer {
    generation_service: Arc<GenerationService>,
    id_generator: Arc<dyn IdGenerator>,
}

impl LlmThreadSynthesizer {
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
struct ThreadsWire {
    threads: Vec<ThreadWire>,
    updated_carried_summary: String,
}

#[derive(Debug, Deserialize)]
struct ThreadWire {
    kind: String,
    summary: String,
    evidence_ids: Vec<u32>,
}

#[async_trait]
impl ThreadSynthesizer for LlmThreadSynthesizer {
    async fn synthesize(
        &self,
        generation_model_id: Uuid,
        req: ThreadSynthRequest<'_>,
    ) -> Result<ThreadSynthResult, AppError> {
        if req.observations.is_empty() {
            return Ok(ThreadSynthResult {
                threads: Vec::new(),
                updated_carried_summary: req.carried_summary.to_string(),
            });
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

        let user = PROMPT_TEMPLATE
            .replace("{carried_summary}", req.carried_summary)
            .replace("{observations}", &observations_text);

        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: "You stitch atomic observations into longer-range threads and return only JSON.".into(),
                    user,
                    temperature: 0.4,
                    response_format: GenerationResponseFormat::Json,
                },
            )
            .await?;

        let wire = parse_threads(&response.content)?;

        let mut threads = Vec::new();
        for raw in wire.threads {
            let kind = raw.kind.trim().to_lowercase();
            let summary = raw.summary.trim().to_string();
            if kind.is_empty() || summary.is_empty() {
                continue;
            }
            let mut evidence = Vec::new();
            let mut spans: Vec<Span> = Vec::new();
            for id in raw.evidence_ids {
                let Some(obs) = req.observations.get(id as usize) else {
                    continue;
                };
                evidence.push(MapItemRef::Observation {
                    map_id: req.map_id,
                    observation_id: obs.observation_id,
                });
                spans.extend(obs.spans.iter().copied());
            }
            if evidence.is_empty() {
                continue;
            }
            threads.push(Thread {
                thread_id: self.id_generator.new_uuid(),
                section_sequence: req.section_sequence,
                kind,
                summary,
                evidence,
                spans,
            });
        }

        let updated = if wire.updated_carried_summary.trim().is_empty() {
            req.carried_summary.to_string()
        } else {
            wire.updated_carried_summary.trim().to_string()
        };

        Ok(ThreadSynthResult {
            threads,
            updated_carried_summary: updated,
        })
    }
}

fn parse_threads(content: &str) -> Result<ThreadsWire, AppError> {
    let text = strip_code_fence(content.trim());
    serde_json::from_str(text).map_err(|e| AppError::Upstream(format!("parse threads JSON: {e}")))
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
    fn parses_threads_payload() {
        let wire = parse_threads(
            r#"{"threads":[{"kind":"arc","summary":"X","evidence_ids":[0,1]}],"updated_carried_summary":"Y"}"#,
        )
        .unwrap();
        assert_eq!(wire.threads.len(), 1);
        assert_eq!(wire.updated_carried_summary, "Y");
    }

    #[test]
    fn parses_fenced_payload() {
        let wire = parse_threads("```json\n{\"threads\":[],\"updated_carried_summary\":\"\"}\n```")
            .unwrap();
        assert!(wire.threads.is_empty());
    }
}
