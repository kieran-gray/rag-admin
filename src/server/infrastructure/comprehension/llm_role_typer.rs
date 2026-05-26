use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

use crate::server::application::comprehension::ports::RoleTyper;
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::AppError;
use crate::server::domain::comprehension::role::SuggestedRole;

const PROMPT_TEMPLATE: &str =
    include_str!("../../application/comprehension/prompts/role_typer.txt");

pub struct LlmRoleTyper {
    generation_service: Arc<GenerationService>,
}

impl LlmRoleTyper {
    pub fn new(generation_service: Arc<GenerationService>) -> Arc<Self> {
        Arc::new(Self { generation_service })
    }
}

#[derive(Debug, Deserialize)]
struct RolesWire {
    roles: Vec<RoleWire>,
}

#[derive(Debug, Deserialize)]
struct RoleWire {
    name: String,
    focus: String,
}

#[async_trait]
impl RoleTyper for LlmRoleTyper {
    async fn suggest(
        &self,
        generation_model_id: Uuid,
        document_sample: &str,
    ) -> Result<Vec<SuggestedRole>, AppError> {
        let user = PROMPT_TEMPLATE.replace("{sample}", document_sample);
        let response = self
            .generation_service
            .generate(
                generation_model_id,
                GenerationPrompt {
                    system: "You produce concise JSON for a reading-comprehension test designer."
                        .into(),
                    user,
                    temperature: 0.7,
                    response_format: GenerationResponseFormat::Json,
                },
            )
            .await?;
        parse_roles(&response.content)
    }
}

fn parse_roles(content: &str) -> Result<Vec<SuggestedRole>, AppError> {
    let text = strip_code_fence(content.trim());
    let wire: RolesWire = serde_json::from_str(text)
        .map_err(|e| AppError::Upstream(format!("parse roles JSON: {e}")))?;
    Ok(wire
        .roles
        .into_iter()
        .filter_map(|r| {
            let name = r.name.trim().to_string();
            let focus = r.focus.trim().to_string();
            if name.is_empty() || focus.is_empty() {
                None
            } else {
                Some(SuggestedRole { name, focus })
            }
        })
        .collect())
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
        let roles =
            parse_roles(r#"{"roles":[{"name":"literary critic","focus":"motifs"}]}"#).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].name, "literary critic");
    }

    #[test]
    fn parses_fenced_json() {
        let roles = parse_roles(
            "```json\n{\"roles\":[{\"name\":\"engineer\",\"focus\":\"tradeoffs\"}]}\n```",
        )
        .unwrap();
        assert_eq!(roles.len(), 1);
    }

    #[test]
    fn drops_empty_entries() {
        let roles = parse_roles(
            r#"{"roles":[{"name":"","focus":"foo"},{"name":"x","focus":""},{"name":"keep","focus":"this"}]}"#,
        )
        .unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].name, "keep");
    }
}
