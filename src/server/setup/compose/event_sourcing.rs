use std::collections::HashMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::EmbeddingModelCatalog;
use crate::server::domain::configuration::generation_model::GenerationModelCatalog;
use crate::server::domain::configuration::sweep_template::SweepTemplate;
use crate::server::domain::configuration::vector_index::VectorIndexCatalog;
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::source_document::aggregate::SourceDocument;
use crate::server::event_sourcing::aggregate_repository::AggregateRepository;
use crate::server::event_sourcing::checkpoint::CheckpointRepository;
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::event_bus::EventBus;
use crate::server::event_sourcing::event_store::EventStore;
use crate::server::event_sourcing::process_manager::ProcessManager;
use crate::server::event_sourcing::projection_driver::ProjectionDriver;
use crate::server::event_sourcing::projector::Projector;
use crate::server::event_sourcing::Aggregate;
use crate::server::infrastructure::event_sourcing::{
    PostgresAggregateSnapshotStore, PostgresEventStore,
};

pub struct AggregateWiring<A: Aggregate> {
    pub aggregate_repository: Arc<AggregateRepository<A>>,
    pub event_store: Arc<dyn EventStore<A::Event>>,
    pub command_processor: Arc<CommandProcessor<A>>,
}

pub struct AggregateWirings {
    pub embedding_model: AggregateWiring<EmbeddingModelCatalog>,
    pub generation_model: AggregateWiring<GenerationModelCatalog>,
    pub vector_index: AggregateWiring<VectorIndexCatalog>,
    pub sweep_template: AggregateWiring<SweepTemplate>,
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
        sweep_template: build_aggregate_wiring::<SweepTemplate>(pool),
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
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    AppError: From<A::Error>,
{
    let wakeup = Arc::new(Notify::new());
    let driver = Arc::new(ProjectionDriver::<A, R>::new(
        event_store,
        projectors,
        checkpoint_repository,
        event_bus,
        process_manager,
        Arc::clone(&wakeup),
    ));
    wakeups.insert(A::aggregate_type().to_owned(), wakeup);
    tokio::spawn(driver.run());
}
