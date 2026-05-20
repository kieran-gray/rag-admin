use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::configuration::{
    ChunkingConfigurationCatalogCommandHandler, ChunkingConfigurationQueryService,
    ConfigurationDefaultsCommandHandler, ConfigurationQueryService,
    EmbeddingModelCatalogCommandHandler, GenerationModelCatalogCommandHandler,
    PipelineConfigurationCatalogCommandHandler, PipelineConfigurationQueryService,
    PipelineResolver, SweepTemplateCommandHandler, SweepTemplateQueryService,
    VectorIndexCatalogCommandHandler,
};
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::IdGenerator;
use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::domain::configuration::chunking_configuration::{
    make_chunking_configuration_projector, ChunkingConfigurationCatalog,
};
use crate::server::domain::configuration::defaults::{
    make_configuration_defaults_projector, ConfigurationDefaults,
};
use crate::server::domain::configuration::embedding_model::{
    make_embedding_model_projector, EmbeddingModelCatalog,
};
use crate::server::domain::configuration::generation_model::{
    make_generation_model_projector, GenerationModelCatalog,
};
use crate::server::domain::configuration::pipeline_configuration::{
    make_pipeline_configuration_projector, PipelineConfigurationCatalog,
};
use crate::server::domain::configuration::sweep_template::{SweepTemplate, SweepTemplateProjector};
use crate::server::domain::configuration::vector_index::{
    make_vector_index_projector, VectorIndexCatalog,
};
use crate::server::infrastructure::shared::clients::CloudflareApi;
use crate::server::infrastructure::vector::{
    CloudflareVectorIndexProvider, PostgresVectorIndexProvider,
};
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use crate::shared::reference_data::VectorStoreKind;
use event_sourcing::event_bus::EventBus;

pub struct CatalogServices {
    pub embedding_model_command_handler: Arc<EmbeddingModelCatalogCommandHandler>,
    pub generation_model_command_handler: Arc<GenerationModelCatalogCommandHandler>,
    pub vector_index_command_handler: Arc<VectorIndexCatalogCommandHandler>,
    pub pipeline_configuration_command_handler: Arc<PipelineConfigurationCatalogCommandHandler>,
    pub chunking_configuration_command_handler: Arc<ChunkingConfigurationCatalogCommandHandler>,
    pub sweep_template_command_handler: Arc<SweepTemplateCommandHandler>,
    pub configuration_defaults_command_handler: Arc<ConfigurationDefaultsCommandHandler>,
    pub configuration_query_service: Arc<ConfigurationQueryService>,
    pub pipeline_configuration_query_service: Arc<PipelineConfigurationQueryService>,
    pub chunking_configuration_query_service: Arc<ChunkingConfigurationQueryService>,
    pub sweep_template_query_service: Arc<SweepTemplateQueryService>,
    pub pipeline_resolver: Arc<PipelineResolver>,
    pub vector_index_resolver: Arc<VectorIndexResolver>,
}

pub struct CatalogDeps<'a> {
    pub pool: PgPool,
    pub id_generator: Arc<dyn IdGenerator>,
    pub cf_api: Arc<CloudflareApi>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
    pub embedding_service: Arc<EmbeddingService>,
    pub generation_service: Arc<GenerationService>,
    pub event_bus: Arc<EventBus>,
    pub wakeups: &'a mut HashMap<String, Arc<Notify>>,
}

impl CatalogServices {
    pub fn build(deps: CatalogDeps<'_>) -> Result<Self, SetupError> {
        let CatalogDeps {
            pool,
            id_generator,
            cf_api,
            repos,
            wirings,
            embedding_service,
            generation_service,
            event_bus,
            wakeups,
        } = deps;

        let vector_providers: HashMap<VectorStoreKind, Arc<dyn VectorIndexProvider>> =
            HashMap::from([
                (
                    VectorStoreKind::CloudflareVectorize,
                    CloudflareVectorIndexProvider::new(cf_api) as Arc<dyn VectorIndexProvider>,
                ),
                (
                    VectorStoreKind::Postgres,
                    PostgresVectorIndexProvider::new(pool) as Arc<dyn VectorIndexProvider>,
                ),
            ]);
        let vector_index_resolver =
            VectorIndexResolver::new(vector_providers, Arc::clone(&repos.vector_index));

        let pipeline_resolver = PipelineResolver::new(
            Arc::clone(&repos.pipeline_configuration),
            Arc::clone(&repos.connector),
            Arc::clone(&repos.configuration_defaults),
            embedding_service,
            generation_service,
            Arc::clone(&vector_index_resolver),
        );

        let embedding_model_command_handler = EmbeddingModelCatalogCommandHandler::new(Arc::clone(
            &wirings.embedding_model.command_processor,
        ));
        let generation_model_command_handler = GenerationModelCatalogCommandHandler::new(
            Arc::clone(&wirings.generation_model.command_processor),
        );
        let vector_index_command_handler = VectorIndexCatalogCommandHandler::new(Arc::clone(
            &wirings.vector_index.command_processor,
        ));
        let configuration_defaults_command_handler = ConfigurationDefaultsCommandHandler::new(
            Arc::clone(&wirings.defaults.command_processor),
        );
        let sweep_template_command_handler = SweepTemplateCommandHandler::new(
            Arc::clone(&wirings.sweep_template.command_processor),
            Arc::clone(&configuration_defaults_command_handler),
            Arc::clone(&id_generator),
        );
        let pipeline_configuration_command_handler =
            PipelineConfigurationCatalogCommandHandler::new(
                Arc::clone(&wirings.pipeline_configuration.command_processor),
                Arc::clone(&repos.embedding_model),
                Arc::clone(&repos.generation_model),
                Arc::clone(&repos.vector_index),
                Arc::clone(&configuration_defaults_command_handler),
            );
        let chunking_configuration_command_handler =
            ChunkingConfigurationCatalogCommandHandler::new(
                Arc::clone(&wirings.chunking_configuration.command_processor),
                Arc::clone(&configuration_defaults_command_handler),
            );

        let configuration_query_service = ConfigurationQueryService::new(
            Arc::clone(&repos.embedding_model),
            Arc::clone(&repos.generation_model),
            Arc::clone(&repos.vector_index),
        );
        let pipeline_configuration_query_service = PipelineConfigurationQueryService::new(
            Arc::clone(&repos.pipeline_configuration),
            Arc::clone(&repos.embedding_model),
            Arc::clone(&repos.generation_model),
            Arc::clone(&repos.vector_index),
            Arc::clone(&repos.configuration_defaults),
        );
        let chunking_configuration_query_service = ChunkingConfigurationQueryService::new(
            Arc::clone(&repos.chunking_configuration),
            Arc::clone(&repos.connector),
            Arc::clone(&repos.configuration_defaults),
        );
        let sweep_template_query_service =
            SweepTemplateQueryService::new(Arc::clone(&repos.sweep_template));

        spawn_drivers(repos, wirings, &event_bus, wakeups);

        Ok(Self {
            embedding_model_command_handler,
            generation_model_command_handler,
            vector_index_command_handler,
            pipeline_configuration_command_handler,
            chunking_configuration_command_handler,
            sweep_template_command_handler,
            configuration_defaults_command_handler,
            configuration_query_service,
            pipeline_configuration_query_service,
            chunking_configuration_query_service,
            sweep_template_query_service,
            pipeline_resolver,
            vector_index_resolver,
        })
    }
}

fn spawn_drivers(
    repos: &Repositories,
    wirings: &AggregateWirings,
    event_bus: &Arc<EventBus>,
    wakeups: &mut HashMap<String, Arc<Notify>>,
) {
    spawn_driver::<EmbeddingModelCatalog, ()>(
        Arc::clone(&wirings.embedding_model.event_store),
        vec![Arc::new(make_embedding_model_projector(
            Arc::clone(&repos.embedding_model) as _,
        ))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<GenerationModelCatalog, ()>(
        Arc::clone(&wirings.generation_model.event_store),
        vec![Arc::new(make_generation_model_projector(
            Arc::clone(&repos.generation_model) as _,
        ))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<VectorIndexCatalog, ()>(
        Arc::clone(&wirings.vector_index.event_store),
        vec![Arc::new(make_vector_index_projector(
            Arc::clone(&repos.vector_index) as _,
        ))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<SweepTemplate, ()>(
        Arc::clone(&wirings.sweep_template.event_store),
        vec![Arc::new(SweepTemplateProjector::new(Arc::clone(
            &repos.sweep_template,
        )))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<ChunkingConfigurationCatalog, ()>(
        Arc::clone(&wirings.chunking_configuration.event_store),
        vec![Arc::new(make_chunking_configuration_projector(Arc::clone(
            &repos.chunking_configuration,
        )
            as _))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<PipelineConfigurationCatalog, ()>(
        Arc::clone(&wirings.pipeline_configuration.event_store),
        vec![Arc::new(make_pipeline_configuration_projector(Arc::clone(
            &repos.pipeline_configuration,
        )
            as _))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<ConfigurationDefaults, ()>(
        Arc::clone(&wirings.defaults.event_store),
        vec![Arc::new(make_configuration_defaults_projector(
            Arc::clone(&repos.configuration_defaults),
            Arc::clone(&repos.sweep_template),
        ))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
}
