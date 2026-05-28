use std::sync::Arc;

use uuid::Uuid;

use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::error::CommandError;

use crate::server::application::chunk_set::ChunkSetCommandHandler;
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::aggregate::derive_chunk_set_id;
use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::comprehension::map::aggregate::{derive_map_id, DocumentMap, MapStatus};
use crate::server::domain::comprehension::map::commands::{
    DocumentMapCommand, RecordInsights, RecordObservations, RecordSuggestedRoles, RecordThreads,
    RequestMapBuild,
};
use crate::server::domain::comprehension::map::events::DEFAULT_SECTION_SIZE;
use crate::server::domain::comprehension::map::exceptions::DocumentMapError;
use crate::server::domain::comprehension::map::repository::DocumentMapRepository;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::comprehension::thread::Thread;
use crate::server::domain::comprehension::Insight;
use crate::server::domain::shared::value_objects::{ChunkingConfig, SectionChunkingConfig};
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::shared::ChunkingConfig as ChunkingConfigDto;

pub const COMPREHENSION_SECTION_TOKENS: u32 = 1024;

pub fn comprehension_chunking_config() -> ChunkingConfig {
    ChunkingConfig::Section(SectionChunkingConfig {
        max_section_tokens: COMPREHENSION_SECTION_TOKENS,
    })
}

#[derive(Debug, Clone)]
pub struct MapBuildHandle {
    pub map_id: Uuid,
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    pub status: MapStatus,
}

pub struct DocumentMapCommandHandler {
    processor: Arc<CommandProcessor<DocumentMap>>,
    repository: Arc<dyn DocumentMapRepository>,
    chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    chunker_registry: Arc<ChunkerRegistry>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl DocumentMapCommandHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        processor: Arc<CommandProcessor<DocumentMap>>,
        repository: Arc<dyn DocumentMapRepository>,
        chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        blob_store: Arc<dyn BlobStore>,
        chunker_registry: Arc<ChunkerRegistry>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            repository,
            chunk_set_command_handler,
            source_document_repository,
            blob_store,
            chunker_registry,
            clock,
            id_generator,
        })
    }

    pub async fn request_build_for_document(
        &self,
        document_id: Uuid,
        document_version: u32,
        generation_model_id: Uuid,
    ) -> Result<MapBuildHandle, AppError> {
        self.build_for_document(document_id, document_version, generation_model_id, false)
            .await
    }

    pub async fn rebuild_for_document(
        &self,
        document_id: Uuid,
        document_version: u32,
        generation_model_id: Uuid,
    ) -> Result<MapBuildHandle, AppError> {
        self.build_for_document(document_id, document_version, generation_model_id, true)
            .await
    }

    async fn build_for_document(
        &self,
        document_id: Uuid,
        document_version: u32,
        generation_model_id: Uuid,
        force_rebuild: bool,
    ) -> Result<MapBuildHandle, AppError> {
        let document = self
            .source_document_repository
            .load(document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {document_id}")))?;
        let content_hash = document.latest_content_hash.as_hex().to_string();

        let existing = self
            .repository
            .find_for_document(document_id, document_version)
            .await?;

        if let Some(existing) = existing {
            if force_rebuild {
                self.repository.delete(existing.map_id).await?;
                self.repository.delete_event_stream(existing.map_id).await?;
            } else {
                let status = parse_status_str(&existing.status);
                return Ok(MapBuildHandle {
                    map_id: existing.map_id,
                    chunk_set_id: existing.chunk_set_id,
                    chunk_count: existing.chunk_count,
                    status,
                });
            }
        }

        let chunking_config = comprehension_chunking_config();
        let chunking_dto: ChunkingConfigDto = chunking_config.into();

        let bytes = self.blob_store.get(&document.latest_content_hash).await?;
        let markdown = String::from_utf8(bytes).map_err(|e| {
            AppError::Internal(format!("content for {document_id} is not utf-8: {e}"))
        })?;

        let chunk_outputs = self
            .chunker_registry
            .chunk_markdown(&chunking_dto, &markdown)
            .await
            .map_err(|e| AppError::Internal(format!("comprehension chunking failed: {e}")))?;
        let chunk_count = chunk_outputs.len() as u32;
        let chunk_set_id = derive_chunk_set_id(document_id, document_version, &chunking_config);
        let chunks: Vec<Chunk> = chunk_outputs
            .into_iter()
            .enumerate()
            .map(|(i, co)| Chunk {
                chunk_id: self.id_generator.new_uuid(),
                sequence: i as u32,
                heading: co.heading,
                text: co.text,
                char_start: co.char_start,
                char_end: co.char_end,
            })
            .collect();

        if !chunks.is_empty() {
            self.chunk_set_command_handler
                .create(
                    chunk_set_id,
                    document_id,
                    document_version,
                    chunking_config,
                    chunks,
                )
                .await?;
        }

        let map_id = derive_map_id(document_id, document_version, &content_hash);
        let cmd = DocumentMapCommand::RequestMapBuild(RequestMapBuild {
            map_id,
            document_id,
            document_version,
            content_hash,
            chunk_set_id,
            chunk_count,
            section_size: DEFAULT_SECTION_SIZE,
            generation_model_id,
            occurred_at: self.clock.now(),
        });
        match self.processor.handle(map_id, cmd).await {
            Ok(_) | Err(CommandError::Aggregate(DocumentMapError::AlreadyExists)) => {}
            Err(e) => return Err(map_command_error(e)),
        }

        Ok(MapBuildHandle {
            map_id,
            chunk_set_id,
            chunk_count,
            status: MapStatus::Pending,
        })
    }

    pub async fn record_suggested_roles(
        &self,
        map_id: Uuid,
        roles: Vec<SuggestedRole>,
    ) -> Result<(), AppError> {
        let cmd = DocumentMapCommand::RecordSuggestedRoles(RecordSuggestedRoles {
            map_id,
            roles,
            occurred_at: self.clock.now(),
        });
        self.processor
            .handle(map_id, cmd)
            .await
            .map_err(map_command_error)?;
        Ok(())
    }

    pub async fn record_observations(
        &self,
        map_id: Uuid,
        chunk_sequence: u32,
        observations: Vec<Observation>,
    ) -> Result<(), AppError> {
        let cmd = DocumentMapCommand::RecordObservations(RecordObservations {
            map_id,
            chunk_sequence,
            observations,
            occurred_at: self.clock.now(),
        });
        self.processor
            .handle(map_id, cmd)
            .await
            .map_err(map_command_error)?;
        Ok(())
    }

    pub async fn record_threads(
        &self,
        map_id: Uuid,
        section_sequence: u32,
        carried_summary: String,
        threads: Vec<Thread>,
    ) -> Result<(), AppError> {
        let cmd = DocumentMapCommand::RecordThreads(RecordThreads {
            map_id,
            section_sequence,
            threads,
            carried_summary,
            occurred_at: self.clock.now(),
        });
        self.processor
            .handle(map_id, cmd)
            .await
            .map_err(map_command_error)?;
        Ok(())
    }

    pub async fn record_insights(
        &self,
        map_id: Uuid,
        insights: Vec<Insight>,
    ) -> Result<(), AppError> {
        let cmd = DocumentMapCommand::RecordInsights(RecordInsights {
            map_id,
            insights,
            occurred_at: self.clock.now(),
        });
        self.processor
            .handle(map_id, cmd)
            .await
            .map_err(map_command_error)?;
        Ok(())
    }
}

fn parse_status_str(status: &str) -> MapStatus {
    match status {
        "typing_roles" => MapStatus::TypingRoles,
        "extracting_observations" => MapStatus::ExtractingObservations {
            remaining_chunks: 0,
        },
        "synthesizing_threads" => MapStatus::SynthesizingThreads {
            remaining_sections: 0,
        },
        "synthesizing_insights" => MapStatus::SynthesizingInsights,
        "ready" => MapStatus::Ready,
        _ => MapStatus::Pending,
    }
}

fn map_command_error(e: CommandError<DocumentMapError>) -> AppError {
    match e {
        CommandError::Aggregate(err) => err.into(),
        CommandError::EventStore(err) => err.into(),
    }
}
