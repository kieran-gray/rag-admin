use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use event_sourcing::process_manager::{EffectError, EffectExecutor};

use crate::server::application::comprehension::command_handler::DocumentMapCommandHandler;
use crate::server::application::comprehension::ports::{
    InsightSynthRequest, InsightSynthesizer, ObservationContext, ObservationExtractor, RoleTyper,
    ThreadSynthRequest, ThreadSynthesizer,
};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::comprehension::map::aggregate::section_for_chunk;
use crate::server::domain::comprehension::map::effects::{
    DocumentMapEffect, ExtractObservationsEffect, SuggestRolesEffect, SynthesizeInsightsEffect,
    SynthesizeThreadsEffect,
};
use crate::server::domain::comprehension::map::read_model::DocumentMapReadModel;
use crate::server::domain::comprehension::map::repository::DocumentMapRepository;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::source_document::repository::SourceDocumentRepository;

const ROLE_SAMPLE_CHARS: usize = 4_000;

pub struct DocumentMapEffectExecutor {
    command_handler: Arc<DocumentMapCommandHandler>,
    repository: Arc<dyn DocumentMapRepository>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    role_typer: Arc<dyn RoleTyper>,
    observation_extractor: Arc<dyn ObservationExtractor>,
    thread_synthesizer: Arc<dyn ThreadSynthesizer>,
    insight_synthesizer: Arc<dyn InsightSynthesizer>,
}

impl DocumentMapEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_handler: Arc<DocumentMapCommandHandler>,
        repository: Arc<dyn DocumentMapRepository>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        blob_store: Arc<dyn BlobStore>,
        role_typer: Arc<dyn RoleTyper>,
        observation_extractor: Arc<dyn ObservationExtractor>,
        thread_synthesizer: Arc<dyn ThreadSynthesizer>,
        insight_synthesizer: Arc<dyn InsightSynthesizer>,
    ) -> Arc<Self> {
        Arc::new(Self {
            command_handler,
            repository,
            chunk_set_repository,
            source_document_repository,
            blob_store,
            role_typer,
            observation_extractor,
            thread_synthesizer,
            insight_synthesizer,
        })
    }

    async fn load_summary(&self, map_id: Uuid) -> Result<Option<DocumentMapReadModel>, AppError> {
        Ok(self.repository.load_summary(map_id).await?)
    }

    async fn execute_suggest_roles(&self, effect: &SuggestRolesEffect) -> Result<(), AppError> {
        let map_id = effect.map_id;
        let Some(summary) = self.load_summary(map_id).await? else {
            return Ok(());
        };
        if summary.status == "ready" || summary.status == "failed" {
            return Ok(());
        }

        let document = self
            .source_document_repository
            .load(summary.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", summary.document_id)))?;
        let bytes = self.blob_store.get(&document.latest_content_hash).await?;
        let text = String::from_utf8(bytes).map_err(|e| {
            AppError::Internal(format!(
                "content for {} is not utf-8: {e}",
                summary.document_id
            ))
        })?;
        let sample = take_chars(&text, ROLE_SAMPLE_CHARS);

        let roles = match self
            .role_typer
            .suggest(summary.generation_model_id, &sample)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                self.command_handler
                    .fail_map(map_id, format!("role suggestion failed: {e}"))
                    .await?;
                return Ok(());
            }
        };

        let roles = if roles.is_empty() {
            vec![SuggestedRole {
                name: "general reader".into(),
                focus: "comprehension of the document as written".into(),
            }]
        } else {
            roles
        };

        self.command_handler
            .record_suggested_roles(map_id, roles)
            .await?;
        Ok(())
    }

    async fn execute_extract_observations(
        &self,
        effect: &ExtractObservationsEffect,
    ) -> Result<(), AppError> {
        let map_id = effect.map_id;
        let Some(summary) = self.load_summary(map_id).await? else {
            return Ok(());
        };
        if summary.status == "ready" || summary.status == "failed" {
            return Ok(());
        }

        let chunks = self
            .chunk_set_repository
            .load_chunks(summary.chunk_set_id)
            .await?;
        let Some(chunk) = chunks.iter().find(|c| c.sequence == effect.chunk_sequence) else {
            self.command_handler
                .record_chunk_extraction_failure(
                    map_id,
                    effect.chunk_sequence,
                    format!("chunk {} not found in chunk set", effect.chunk_sequence),
                )
                .await?;
            return Ok(());
        };

        let role_summary = render_role_summary(&summary.suggested_roles);
        let ctx = ObservationContext {
            document_id: summary.document_id,
            chunk,
            suggested_roles_summary: &role_summary,
        };

        let observations = match self
            .observation_extractor
            .extract(summary.generation_model_id, ctx)
            .await
        {
            Ok(o) => o,
            Err(e) => {
                self.command_handler
                    .record_chunk_extraction_failure(map_id, effect.chunk_sequence, e.to_string())
                    .await?;
                return Ok(());
            }
        };

        let validated = validate_observations(&summary, chunk, observations);
        self.command_handler
            .record_observations(map_id, effect.chunk_sequence, validated)
            .await?;
        Ok(())
    }

    async fn execute_synthesize_threads(
        &self,
        effect: &SynthesizeThreadsEffect,
    ) -> Result<(), AppError> {
        let map_id = effect.map_id;
        let Some(summary) = self.load_summary(map_id).await? else {
            return Ok(());
        };
        if summary.status == "ready" || summary.status == "failed" {
            return Ok(());
        }

        let section_size = summary.section_size.max(1);
        let from = effect.section_sequence.saturating_mul(section_size);
        let to_inclusive = (from + section_size).saturating_sub(1);

        let observations = self
            .repository
            .load_observations_for_section(map_id, from, to_inclusive)
            .await?;
        let carried = self.repository.carried_summary(map_id).await?;
        let req = ThreadSynthRequest {
            map_id,
            document_id: summary.document_id,
            section_sequence: effect.section_sequence,
            observations: &observations,
            carried_summary: &carried,
        };
        let result = match self
            .thread_synthesizer
            .synthesize(summary.generation_model_id, req)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                self.command_handler
                    .record_section_synthesis_failure(
                        map_id,
                        effect.section_sequence,
                        e.to_string(),
                    )
                    .await?;
                return Ok(());
            }
        };
        self.command_handler
            .record_threads(
                map_id,
                effect.section_sequence,
                result.updated_carried_summary,
                result.threads,
            )
            .await?;
        Ok(())
    }

    async fn execute_synthesize_insights(
        &self,
        effect: &SynthesizeInsightsEffect,
    ) -> Result<(), AppError> {
        let map_id = effect.map_id;
        let Some(summary) = self.load_summary(map_id).await? else {
            return Ok(());
        };
        if summary.status == "ready" || summary.status == "failed" {
            return Ok(());
        }
        let Some(detail) = self.repository.load_detail(map_id).await? else {
            return Ok(());
        };
        let req = InsightSynthRequest {
            map_id,
            document_id: summary.document_id,
            observations: &detail.observations,
            threads: &detail.threads,
        };
        let insights = match self
            .insight_synthesizer
            .synthesize(summary.generation_model_id, req)
            .await
        {
            Ok(i) => i,
            Err(e) => {
                self.command_handler
                    .fail_map(map_id, format!("insight synthesis failed: {e}"))
                    .await?;
                return Ok(());
            }
        };
        self.command_handler
            .record_insights(map_id, insights)
            .await?;
        Ok(())
    }
}

fn validate_observations(
    _summary: &DocumentMapReadModel,
    chunk: &Chunk,
    observations: Vec<Observation>,
) -> Vec<Observation> {
    observations
        .into_iter()
        .filter(|o| {
            o.spans
                .iter()
                .all(|s| s.char_start >= chunk.char_start && s.char_end <= chunk.char_end)
        })
        .collect()
}

fn render_role_summary(roles: &[SuggestedRole]) -> String {
    if roles.is_empty() {
        return "(no roles suggested yet)".to_string();
    }
    roles
        .iter()
        .map(|r| format!("- {}: {}", r.name, r.focus))
        .collect::<Vec<_>>()
        .join("\n")
}

fn take_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[async_trait]
impl EffectExecutor<DocumentMapEffect> for DocumentMapEffectExecutor {
    async fn execute(&self, effect: &DocumentMapEffect) -> Result<(), EffectError> {
        let result: Result<(), AppError> = match effect {
            DocumentMapEffect::SuggestRoles(e) => self.execute_suggest_roles(e).await,
            DocumentMapEffect::ExtractObservations(e) => self.execute_extract_observations(e).await,
            DocumentMapEffect::SynthesizeThreads(e) => self.execute_synthesize_threads(e).await,
            DocumentMapEffect::SynthesizeInsights(e) => self.execute_synthesize_insights(e).await,
        };
        result.map_err(|e| Box::new(e) as EffectError)
    }
}

#[allow(dead_code)]
pub fn section_for(chunk_sequence: u32, section_size: u32) -> u32 {
    section_for_chunk(chunk_sequence, section_size)
}
