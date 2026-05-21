use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::chunk_set::{ChunkSetCommandHandler, ChunkSetQueryService};
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::IndexProfileResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::ports::KvStore;
use crate::server::application::indexing::{
    IndexingCommandHandler, IndexingEffect, IndexingEffectExecutor, VectorIndexResolver,
};
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::{ActivityRegistry, JobRegistry};
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::indexing::projector::IndexingProjector;
use crate::server::infrastructure::shared::event_sourcing::PostgresJobQueue;
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;
use event_sourcing::job_queue::JobQueue;
use event_sourcing::process_manager::ProcessManager;

pub struct IndexingServices {
    pub indexing_command_handler: Arc<IndexingCommandHandler>,
    pub chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
    pub chunk_set_query_service: Arc<ChunkSetQueryService>,
}

pub struct IndexingDeps<'a> {
    pub pool: PgPool,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
    pub event_bus: Arc<EventBus>,
    pub job_registry: Arc<JobRegistry>,
    pub activity_registry: Arc<ActivityRegistry>,
    pub chunker_registry: Arc<ChunkerRegistry>,
    pub embedding_service: Arc<EmbeddingService>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
    pub index_profile_resolver: Arc<IndexProfileResolver>,
    pub kv_store: Arc<dyn KvStore>,
    pub wakeups: &'a mut HashMap<String, Arc<Notify>>,
}

impl IndexingServices {
    pub fn build(deps: IndexingDeps<'_>) -> Result<Self, SetupError> {
        let IndexingDeps {
            pool,
            clock,
            id_generator,
            repos,
            wirings,
            event_bus,
            job_registry,
            activity_registry,
            chunker_registry,
            embedding_service,
            vector_index_resolver,
            index_profile_resolver,
            kv_store,
            wakeups,
        } = deps;

        let indexing_command_handler = IndexingCommandHandler::new(
            Arc::clone(&wirings.indexing.command_processor),
            Arc::clone(&id_generator),
            Arc::clone(&clock),
        );

        let chunk_set_command_handler = ChunkSetCommandHandler::new(Arc::clone(&repos.chunk_set));
        let chunk_set_query_service = ChunkSetQueryService::new(Arc::clone(&repos.chunk_set));

        let indexing_job_queue: Arc<dyn JobQueue<IndexingEffect>> =
            Arc::new(PostgresJobQueue::<IndexingEffect>::new(pool));

        let indexing_effect_executor = IndexingEffectExecutor::new(
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.indexing),
            Arc::clone(&repos.blob_store),
            chunker_registry,
            Arc::clone(&repos.chunk_set),
            embedding_service,
            Arc::clone(&repos.embedding_set),
            vector_index_resolver,
            index_profile_resolver,
            kv_store,
            Arc::clone(&wirings.indexing.command_processor),
            job_registry,
            activity_registry,
            clock,
            id_generator,
        );

        let indexing_process_manager = Arc::new(ProcessManager::<Indexing, IndexingEffect>::new(
            Arc::clone(&wirings.indexing.aggregate_repository),
            indexing_job_queue,
            indexing_effect_executor,
        ));

        spawn_driver::<Indexing, IndexingEffect>(
            Arc::clone(&wirings.indexing.event_store),
            vec![Arc::new(IndexingProjector::new(Arc::clone(
                &repos.indexing,
            )))],
            Some(indexing_process_manager),
            Arc::clone(&repos.checkpoint),
            event_bus,
            wakeups,
        );

        Ok(Self {
            indexing_command_handler,
            chunk_set_command_handler,
            chunk_set_query_service,
        })
    }
}
