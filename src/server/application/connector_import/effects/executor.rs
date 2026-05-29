use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::configuration::{
    ChunkingConfigurationQueryService, IndexProfileResolver,
};
use crate::server::application::indexing::{IndexingCommandHandler, RequestIngestInput};
use crate::server::application::ports::Clock;
use crate::server::application::source_document::SourceDocumentIngestService;
use crate::server::application::{
    ActivityRegistry, AppError, InternalLogEvent, Job, JobIdStrategy, JobRegistry, JobSession,
};
use crate::server::domain::connector_import::{
    ConnectorImport, ConnectorImportCommand, ConnectorImportEffect, FailConnectorImport,
    ImportStatus, RunConnectorImportEffect,
};
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::shared::ChunkingConfig;
use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::process_manager::{EffectError, EffectExecutor};

use super::super::command_handler::ConnectorImportCommandHandler;

pub struct ConnectorImportEffectExecutor {
    aggregate_repository: Arc<AggregateRepository<ConnectorImport>>,
    command_handler: Arc<ConnectorImportCommandHandler>,
    ingest_service: Arc<SourceDocumentIngestService>,
    indexing_command_handler: Arc<IndexingCommandHandler>,
    index_profile_resolver: Arc<IndexProfileResolver>,
    chunking_query_service: Arc<ChunkingConfigurationQueryService>,
    session: JobSession<ConnectorImport>,
    clock: Arc<dyn Clock>,
}

impl ConnectorImportEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        aggregate_repository: Arc<AggregateRepository<ConnectorImport>>,
        command_handler: Arc<ConnectorImportCommandHandler>,
        ingest_service: Arc<SourceDocumentIngestService>,
        indexing_command_handler: Arc<IndexingCommandHandler>,
        index_profile_resolver: Arc<IndexProfileResolver>,
        chunking_query_service: Arc<ChunkingConfigurationQueryService>,
        command_processor: Arc<CommandProcessor<ConnectorImport>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        let session = JobSession::new(
            job_registry,
            activity_registry,
            command_processor,
            Arc::clone(&clock),
        );
        Arc::new(Self {
            aggregate_repository,
            command_handler,
            ingest_service,
            indexing_command_handler,
            index_profile_resolver,
            chunking_query_service,
            session,
            clock,
        })
    }

    async fn run(&self, effect: &RunConnectorImportEffect) -> Result<(), AppError> {
        let import_id = effect.import_id;
        let fail = self.fail_command_for(import_id);
        self.session
            .run(
                import_id,
                JobIdStrategy::PerStream,
                "connector import failed",
                fail,
                |job| self.run_items(import_id, job),
            )
            .await
    }

    fn fail_command_for(
        &self,
        import_id: Uuid,
    ) -> impl FnOnce(String) -> ConnectorImportCommand + use<> {
        let clock = Arc::clone(&self.clock);
        move |reason| {
            ConnectorImportCommand::FailConnectorImport(FailConnectorImport {
                import_id,
                error: reason,
                occurred_at: clock.now(),
            })
        }
    }

    async fn run_items(&self, import_id: Uuid, job: Arc<Job>) -> Result<(), AppError> {
        let import = self
            .aggregate_repository
            .load(import_id)
            .await
            .map_err(|e| AppError::Internal(format!("load connector import: {e}")))?
            .map(|loaded| loaded.aggregate)
            .ok_or_else(|| AppError::NotFound(format!("connector import {import_id}")))?;

        if import.status != ImportStatus::Running {
            return Ok(());
        }

        let pending: Vec<String> = import
            .items
            .iter()
            .filter(|item| !item.status.is_terminal())
            .map(|item| item.source_ref_key.clone())
            .collect();

        let indexing = if import.index_after_import {
            let profile = self
                .index_profile_resolver
                .resolve_for_connector(import.connector_id)
                .await?;
            let chunking = self
                .chunking_query_service
                .resolve_for_connector(import.connector_id)
                .await?;
            Some((profile.index_profile_id, chunking))
        } else {
            None
        };

        job.emit(InternalLogEvent::info(format!(
            "Importing {} item(s) from connector",
            pending.len()
        )))
        .await;

        for key in pending {
            self.import_one(&import, import_id, &key, indexing, &job)
                .await;
        }

        self.command_handler.complete(import_id).await?;
        job.emit(InternalLogEvent::success("Import finished")).await;
        Ok(())
    }

    async fn import_one(
        &self,
        import: &ConnectorImport,
        import_id: Uuid,
        key: &str,
        indexing: Option<(Uuid, ChunkingConfig)>,
        job: &Arc<Job>,
    ) {
        let Some(source_ref) = SourceRef::parse_route_key(key) else {
            job.emit(InternalLogEvent::error(format!(
                "Skipping {key}: invalid source ref"
            )))
            .await;
            self.record_failed(import_id, key, "invalid source ref key".into(), job)
                .await;
            return;
        };

        job.emit(InternalLogEvent::info(format!("Importing {key}")))
            .await;

        match self
            .ingest_service
            .import_from_connector(import.connector_id, source_ref)
            .await
        {
            Ok(dto) => {
                if let Err(e) = self
                    .command_handler
                    .record_imported(
                        import_id,
                        key.to_string(),
                        dto.document_id,
                        dto.latest_version,
                    )
                    .await
                {
                    job.emit(InternalLogEvent::error(format!(
                        "Imported {key} but failed to record: {e}"
                    )))
                    .await;
                    return;
                }
                job.emit(InternalLogEvent::success(format!("Imported {}", dto.title)))
                    .await;

                if let Some((index_profile_id, chunking_config)) = indexing {
                    match self
                        .indexing_command_handler
                        .request_ingest(RequestIngestInput {
                            document_id: dto.document_id,
                            index_profile_id,
                            document_version: dto.latest_version,
                            chunking_config,
                            auto_advance: true,
                        })
                        .await
                    {
                        Ok(_) => {
                            job.emit(InternalLogEvent::info(format!("Queued {key} for indexing")))
                                .await;
                        }
                        Err(e) => {
                            job.emit(InternalLogEvent::error(format!(
                                "Imported {key} but indexing request failed: {e}"
                            )))
                            .await;
                        }
                    }
                }
            }
            Err(e) => {
                job.emit(InternalLogEvent::error(format!(
                    "Failed to import {key}: {e}"
                )))
                .await;
                self.record_failed(import_id, key, e.to_string(), job).await;
            }
        }
    }

    async fn record_failed(&self, import_id: Uuid, key: &str, error: String, job: &Arc<Job>) {
        if let Err(e) = self
            .command_handler
            .record_failed(import_id, key.to_string(), error)
            .await
        {
            job.emit(InternalLogEvent::error(format!(
                "Failed to record import failure for {key}: {e}"
            )))
            .await;
        }
    }
}

#[async_trait]
impl EffectExecutor<ConnectorImportEffect> for ConnectorImportEffectExecutor {
    async fn execute(&self, effect: &ConnectorImportEffect) -> Result<(), EffectError> {
        let result = match effect {
            ConnectorImportEffect::Run(e) => self.run(e).await,
        };
        result.map_err(|e| Box::new(e) as EffectError)
    }
}
