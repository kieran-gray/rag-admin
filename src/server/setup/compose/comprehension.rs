use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Notify;

use crate::server::application::chunk_set::ChunkSetCommandHandler;
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::comprehension::ports::{
    InsightSynthesizer, ObservationExtractor, RoleTyper, TextSegmenter, ThreadSynthesizer,
};
use crate::server::application::comprehension::{
    ComprehensionQueryService, DocumentMapCommandHandler, DocumentMapEffectExecutor,
    ParagraphSegmenter,
};
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::domain::comprehension::map::aggregate::DocumentMap;
use crate::server::domain::comprehension::map::effects::DocumentMapEffect;
use crate::server::domain::comprehension::map::projector::DocumentMapProjector;
use crate::server::infrastructure::comprehension::{
    LlmInsightSynthesizer, LlmObservationExtractor, LlmRoleTyper, LlmThreadSynthesizer,
};
use crate::server::infrastructure::shared::event_sourcing::PostgresJobQueue;
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;
use event_sourcing::job_queue::JobQueue;
use event_sourcing::process_manager::ProcessManager;
use sqlx::PgPool;

pub struct ComprehensionServices {
    pub document_map_command_handler: Arc<DocumentMapCommandHandler>,
    pub comprehension_query_service: Arc<ComprehensionQueryService>,
}

pub struct ComprehensionDeps<'a> {
    pub pool: PgPool,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
    pub event_bus: Arc<EventBus>,
    pub job_registry: Arc<JobRegistry>,
    pub activity_registry: Arc<ActivityRegistry>,
    pub chunker_registry: Arc<ChunkerRegistry>,
    pub chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
    pub generation_service: Arc<GenerationService>,
    pub wakeups: &'a mut HashMap<String, Vec<Arc<Notify>>>,
}

impl ComprehensionServices {
    pub fn build(deps: ComprehensionDeps<'_>) -> Result<Self, SetupError> {
        let ComprehensionDeps {
            pool,
            clock,
            id_generator,
            repos,
            wirings,
            event_bus,
            job_registry,
            activity_registry,
            chunker_registry,
            chunk_set_command_handler,
            generation_service,
            wakeups,
        } = deps;

        let role_typer: Arc<dyn RoleTyper> = LlmRoleTyper::new(Arc::clone(&generation_service));
        let segmenter: Arc<dyn TextSegmenter> = Arc::new(ParagraphSegmenter::new());
        let observation_extractor: Arc<dyn ObservationExtractor> = LlmObservationExtractor::new(
            Arc::clone(&generation_service),
            Arc::clone(&id_generator),
            segmenter,
        );
        let thread_synthesizer: Arc<dyn ThreadSynthesizer> =
            LlmThreadSynthesizer::new(Arc::clone(&generation_service), Arc::clone(&id_generator));
        let insight_synthesizer: Arc<dyn InsightSynthesizer> =
            LlmInsightSynthesizer::new(Arc::clone(&generation_service), Arc::clone(&id_generator));

        let document_map_command_handler = DocumentMapCommandHandler::new(
            Arc::clone(&wirings.document_map.command_processor),
            Arc::clone(&repos.document_map),
            chunk_set_command_handler,
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.blob_store),
            chunker_registry,
            Arc::clone(&clock),
            Arc::clone(&id_generator),
        );

        let map_effect_executor = DocumentMapEffectExecutor::new(
            Arc::clone(&document_map_command_handler),
            Arc::clone(&repos.document_map),
            Arc::clone(&repos.chunk_set),
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.blob_store),
            role_typer,
            observation_extractor,
            thread_synthesizer,
            insight_synthesizer,
            job_registry,
            activity_registry,
        );

        let map_job_queue: Arc<dyn JobQueue<DocumentMapEffect>> =
            Arc::new(PostgresJobQueue::<DocumentMapEffect>::new(pool));

        let map_process_manager = Arc::new(ProcessManager::new(
            Arc::clone(&wirings.document_map.aggregate_repository),
            map_job_queue,
            map_effect_executor,
        ));

        spawn_driver::<DocumentMap, DocumentMapEffect>(
            Arc::clone(&wirings.document_map.event_store),
            vec![Arc::new(DocumentMapProjector::new(Arc::clone(
                &repos.document_map,
            )))],
            Some(map_process_manager),
            Arc::clone(&repos.checkpoint),
            event_bus,
            wakeups,
        );

        let comprehension_query_service = ComprehensionQueryService::new(
            Arc::clone(&repos.document_map),
            Arc::clone(&repos.source_document),
        );

        Ok(Self {
            document_map_command_handler,
            comprehension_query_service,
        })
    }
}
