use std::collections::HashMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::ChunkingConfigurationCatalog;
use crate::server::domain::configuration::defaults::ConfigurationDefaults;
use crate::server::domain::configuration::embedding_model::EmbeddingModelCatalog;
use crate::server::domain::configuration::generation_model::GenerationModelCatalog;
use crate::server::domain::configuration::pipeline_configuration::PipelineConfigurationCatalog;
use crate::server::domain::configuration::sweep_template::SweepTemplate;
use crate::server::domain::configuration::vector_index::VectorIndexCatalog;
use crate::server::domain::connector::Connector;
use crate::server::domain::connector_sync::ConnectorSync;
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::source_document::aggregate::SourceDocument;
use crate::server::infrastructure::shared::event_sourcing::{
    PostgresAggregateSnapshotStore, PostgresEventStore,
};
use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::checkpoint::CheckpointRepository;
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::effect_dispatcher::EffectDispatcher;
use event_sourcing::event_bus::EventBus;
use event_sourcing::event_store::EventStore;
use event_sourcing::policy::HasPolicies;
use event_sourcing::process_manager::ProcessManager;
use event_sourcing::projection_driver::ProjectionDriver;
use event_sourcing::projector::Projector;
use event_sourcing::Aggregate;

pub struct AggregateWiring<A: Aggregate> {
    pub aggregate_repository: Arc<AggregateRepository<A>>,
    pub event_store: Arc<dyn EventStore<A::Event>>,
    pub command_processor: Arc<CommandProcessor<A>>,
}

pub struct AggregateWirings {
    pub embedding_model: AggregateWiring<EmbeddingModelCatalog>,
    pub generation_model: AggregateWiring<GenerationModelCatalog>,
    pub vector_index: AggregateWiring<VectorIndexCatalog>,
    pub chunking_configuration: AggregateWiring<ChunkingConfigurationCatalog>,
    pub pipeline_configuration: AggregateWiring<PipelineConfigurationCatalog>,
    pub sweep_template: AggregateWiring<SweepTemplate>,
    pub defaults: AggregateWiring<ConfigurationDefaults>,
    pub connector: AggregateWiring<Connector>,
    pub connector_sync: AggregateWiring<ConnectorSync>,
    pub source_document: AggregateWiring<SourceDocument>,
    pub indexing: AggregateWiring<Indexing>,
    pub dataset: AggregateWiring<EvaluationDataset>,
    pub run: AggregateWiring<EvaluationRun>,
}

pub fn build_aggregate_wirings(pool: &PgPool) -> AggregateWirings {
    AggregateWirings {
        embedding_model: build_aggregate_wiring::<EmbeddingModelCatalog>(pool),
        generation_model: build_aggregate_wiring::<GenerationModelCatalog>(pool),
        vector_index: build_aggregate_wiring::<VectorIndexCatalog>(pool),
        chunking_configuration: build_aggregate_wiring::<ChunkingConfigurationCatalog>(pool),
        pipeline_configuration: build_aggregate_wiring::<PipelineConfigurationCatalog>(pool),
        sweep_template: build_aggregate_wiring::<SweepTemplate>(pool),
        defaults: build_aggregate_wiring::<ConfigurationDefaults>(pool),
        connector: build_aggregate_wiring::<Connector>(pool),
        connector_sync: build_aggregate_wiring::<ConnectorSync>(pool),
        source_document: build_aggregate_wiring::<SourceDocument>(pool),
        indexing: build_aggregate_wiring::<Indexing>(pool),
        dataset: build_aggregate_wiring::<EvaluationDataset>(pool),
        run: build_aggregate_wiring::<EvaluationRun>(pool),
    }
}

fn build_aggregate_wiring<A>(pool: &PgPool) -> AggregateWiring<A>
where
    A: Aggregate + 'static,
    AppError: From<A::Error>,
{
    let event_store: Arc<dyn EventStore<A::Event>> = Arc::new(PostgresEventStore::<A::Event>::new(
        pool.clone(),
        A::aggregate_type(),
    ));
    let snapshot_store = Arc::new(PostgresAggregateSnapshotStore::<A>::new(pool.clone()));
    let aggregate_repository = Arc::new(AggregateRepository::<A>::new(
        Arc::clone(&event_store),
        snapshot_store,
    ));
    let command_processor = Arc::new(CommandProcessor::new(Arc::clone(&aggregate_repository)));
    AggregateWiring {
        aggregate_repository,
        event_store,
        command_processor,
    }
}

pub fn spawn_driver<A, R>(
    event_store: Arc<dyn EventStore<A::Event>>,
    projectors: Vec<Arc<dyn Projector<A::Event>>>,
    process_manager: Option<Arc<ProcessManager<A, R>>>,
    checkpoint_repository: Arc<dyn CheckpointRepository>,
    event_bus: Arc<EventBus>,
    wakeups: &mut HashMap<String, Arc<Notify>>,
) where
    A: Aggregate + 'static,
    A::Event: HasPolicies<A, R>,
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    AppError: From<A::Error>,
{
    let projection_wakeup = Arc::new(Notify::new());
    let effect_wakeup = Arc::new(Notify::new());

    let projection_driver = Arc::new(ProjectionDriver::<A, R>::new(
        event_store,
        projectors,
        checkpoint_repository,
        event_bus,
        process_manager.clone(),
        Arc::clone(&projection_wakeup),
        Arc::clone(&effect_wakeup),
    ));
    wakeups.insert(A::aggregate_type().to_owned(), projection_wakeup);
    tokio::spawn(projection_driver.run());

    if let Some(pm) = process_manager {
        let dispatcher = Arc::new(EffectDispatcher::<A, R>::new(pm, effect_wakeup));
        tokio::spawn(dispatcher.run());
    }
}
