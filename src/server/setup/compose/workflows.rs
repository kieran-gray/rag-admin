use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::evaluation::effects::{
    EvaluationDatasetEffect, EvaluationDatasetEffectExecutor, EvaluationRunEffect,
    EvaluationRunEffectExecutor,
};
use crate::server::application::indexing::{IndexingEffect, IndexingEffectExecutor};
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::SourceAdapterRegistry;
use crate::server::application::source_document::{
    SourceDocumentIngestService, SourceDocumentIngestServiceDeps,
};
use crate::server::application::spawn_activity_projection;
use crate::server::domain::configuration::embedding_model::{
    EmbeddingModelCatalog, EmbeddingModelProjector,
};
use crate::server::domain::configuration::generation_model::{
    GenerationModelCatalog, GenerationModelProjector,
};
use crate::server::domain::configuration::sweep_template::{SweepTemplate, SweepTemplateProjector};
use crate::server::domain::configuration::vector_index::{
    VectorIndexCatalog, VectorIndexProjector,
};
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::dataset::policies::derive_dataset_effects;
use crate::server::domain::evaluation::dataset::projector::EvaluationDatasetProjector;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::policies::derive_run_effects;
use crate::server::domain::evaluation::run::projector::EvaluationRunProjector;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::indexing::policies::derive_indexing_effects;
use crate::server::domain::indexing::projector::IndexingProjector;
use crate::server::domain::source_document::aggregate::SourceDocument;
use crate::server::domain::source_document::projector::SourceDocumentProjector;
use crate::server::event_sourcing::effect::EffectLedger;
use crate::server::event_sourcing::event_bus::EventBus;
use crate::server::event_sourcing::process_manager::ProcessManager;
use crate::server::infrastructure::event_sourcing::{
    spawn_postgres_event_listener, PostgresEffectLedger,
};
use crate::server::infrastructure::http_client::ReqwestHttpClient;
use crate::server::infrastructure::source_document::HttpBlogAdapter;
use crate::server::setup::compose::event_sourcing::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::compose::services::Services;
use crate::server::setup::config::Config;
use crate::server::setup::exceptions::SetupError;

pub struct Workflows {
    pub source_document_ingest_service: Arc<SourceDocumentIngestService>,
    pub source_adapter_registry: Arc<SourceAdapterRegistry>,
}

pub struct WorkflowsDeps<'a> {
    pub config: &'a Config,
    pub pool: PgPool,
    pub http: Arc<ReqwestHttpClient>,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub event_bus: Arc<EventBus>,
    pub repos: &'a Repositories,
    pub services: &'a Services,
    pub wirings: &'a AggregateWirings,
}

pub fn launch_workflows(deps: WorkflowsDeps<'_>) -> Result<Workflows, SetupError> {
    let WorkflowsDeps {
        config,
        pool,
        http,
        clock,
        id_generator,
        event_bus,
        repos,
        services,
        wirings,
    } = deps;

    spawn_activity_projection(
        Arc::clone(&services.activity_registry),
        Arc::clone(&event_bus),
    );

    let mut wakeups: HashMap<String, Arc<Notify>> = HashMap::new();
    let checkpoint = Arc::clone(&repos.checkpoint);

    spawn_driver::<EmbeddingModelCatalog, ()>(
        Arc::clone(&wirings.embedding_model.event_store),
        vec![Arc::new(EmbeddingModelProjector::new(Arc::clone(
            &repos.embedding_model,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<GenerationModelCatalog, ()>(
        Arc::clone(&wirings.generation_model.event_store),
        vec![Arc::new(GenerationModelProjector::new(Arc::clone(
            &repos.generation_model,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<VectorIndexCatalog, ()>(
        Arc::clone(&wirings.vector_index.event_store),
        vec![Arc::new(VectorIndexProjector::new(Arc::clone(
            &repos.vector_index,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<SweepTemplate, ()>(
        Arc::clone(&wirings.sweep_template.event_store),
        vec![Arc::new(SweepTemplateProjector::new(Arc::clone(
            &repos.sweep_template,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<SourceDocument, ()>(
        Arc::clone(&wirings.source_document.event_store),
        vec![Arc::new(SourceDocumentProjector::new(Arc::clone(
            &repos.source_document,
        )))],
        None,
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );

    let indexing_effect_ledger: Arc<dyn EffectLedger<IndexingEffect>> =
        Arc::new(PostgresEffectLedger::<IndexingEffect>::new(pool.clone()));

    let indexing_effect_executor = IndexingEffectExecutor::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.indexing),
        Arc::clone(&repos.blob_store),
        Arc::clone(&services.chunking_engine),
        Arc::clone(&repos.chunk_set),
        Arc::clone(&services.embedding_service),
        Arc::clone(&repos.embedding_set),
        Arc::clone(&services.vector_index_resolver),
        Arc::clone(&services.pipeline_resolver),
        Arc::clone(&repos.kv_store),
        Arc::clone(&wirings.indexing.command_processor),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );

    let indexing_process_manager = Arc::new(ProcessManager::<Indexing, IndexingEffect>::new(
        Arc::clone(&wirings.indexing.aggregate_repository),
        indexing_effect_ledger,
        indexing_effect_executor,
        derive_indexing_effects,
    ));

    spawn_driver::<Indexing, IndexingEffect>(
        Arc::clone(&wirings.indexing.event_store),
        vec![Arc::new(IndexingProjector::new(Arc::clone(
            &repos.indexing,
        )))],
        Some(indexing_process_manager),
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );

    let dataset_effect_ledger: Arc<dyn EffectLedger<EvaluationDatasetEffect>> = Arc::new(
        PostgresEffectLedger::<EvaluationDatasetEffect>::new(pool.clone()),
    );
    let run_effect_ledger: Arc<dyn EffectLedger<EvaluationRunEffect>> = Arc::new(
        PostgresEffectLedger::<EvaluationRunEffect>::new(pool.clone()),
    );

    let dataset_effect_executor = EvaluationDatasetEffectExecutor::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.blob_store),
        Arc::clone(&services.evaluation_generator),
        Arc::clone(&services.embedding_service),
        Arc::clone(&wirings.dataset.command_processor),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
    );

    let run_effect_executor = EvaluationRunEffectExecutor::new(
        Arc::clone(&repos.source_document),
        Arc::clone(&repos.blob_store),
        Arc::clone(&services.chunking_engine),
        Arc::clone(&repos.chunk_set),
        Arc::clone(&services.embedding_service),
        Arc::clone(&repos.embedding_set),
        Arc::clone(&repos.evaluation_dataset),
        Arc::clone(&services.evaluation_retriever),
        Arc::clone(&wirings.run.command_processor),
        Arc::clone(&services.pipeline_resolver),
        Arc::clone(&services.job_registry),
        Arc::clone(&services.activity_registry),
        Arc::clone(&clock),
        Arc::clone(&id_generator),
    );

    let dataset_process_manager = Arc::new(ProcessManager::new(
        Arc::clone(&wirings.dataset.aggregate_repository),
        dataset_effect_ledger,
        dataset_effect_executor,
        derive_dataset_effects,
    ));
    let run_process_manager = Arc::new(ProcessManager::new(
        Arc::clone(&wirings.run.aggregate_repository),
        run_effect_ledger,
        run_effect_executor,
        derive_run_effects,
    ));

    spawn_driver::<EvaluationDataset, EvaluationDatasetEffect>(
        Arc::clone(&wirings.dataset.event_store),
        vec![Arc::new(EvaluationDatasetProjector::new(Arc::clone(
            &repos.evaluation_dataset,
        )))],
        Some(dataset_process_manager),
        Arc::clone(&checkpoint),
        Arc::clone(&event_bus),
        &mut wakeups,
    );
    spawn_driver::<EvaluationRun, EvaluationRunEffect>(
        Arc::clone(&wirings.run.event_store),
        vec![Arc::new(EvaluationRunProjector::new(Arc::clone(
            &repos.evaluation_run,
        )))],
        Some(run_process_manager),
        checkpoint,
        Arc::clone(&event_bus),
        &mut wakeups,
    );

    spawn_postgres_event_listener(pool, wakeups);

    let mut source_adapter_registry = SourceAdapterRegistry::new();
    source_adapter_registry.register(HttpBlogAdapter::new(
        Arc::clone(&http),
        config.blog_url.clone(),
    ));
    let source_adapter_registry = Arc::new(source_adapter_registry);

    let source_document_ingest_service =
        SourceDocumentIngestService::new(SourceDocumentIngestServiceDeps {
            source_document_command_handler: Arc::clone(&services.source_document_command_handler),
            indexing_command_handler: Arc::clone(&services.indexing_command_handler),
            source_document_repository: Arc::clone(&repos.source_document),
            blob_store: Arc::clone(&repos.blob_store),
            source_adapter_registry: Arc::clone(&source_adapter_registry),
            pipeline_resolver: Arc::clone(&services.pipeline_resolver),
            clock,
            id_generator,
        });

    Ok(Workflows {
        source_document_ingest_service,
        source_adapter_registry,
    })
}
