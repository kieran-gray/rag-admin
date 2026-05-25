use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Notify;

use crate::server::application::configuration::{
    ChunkingConfigurationCatalogCommandHandler, ChunkingConfigurationQueryService,
    ConfigurationDefaultsCommandHandler, ConfigurationQueryService,
    EmbeddingModelCatalogCommandHandler, GenerationModelCatalogCommandHandler,
    IndexProfileCatalogCommandHandler, IndexProfileQueryService, IndexProfileResolver,
    RerankerModelCatalogCommandHandler, RetrievalProfileCatalogCommandHandler,
    RetrievalProfileQueryService, RetrievalProfileResolver, SweepTemplateCommandHandler,
    SweepTemplateQueryService, VectorIndexCatalogCommandHandler,
};
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::{VectorIndexProviderRegistry, VectorIndexResolver};
use crate::server::application::llm::GenerationService;
use crate::server::application::ports::IdGenerator;
use crate::server::application::rerank::RerankService;
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
use crate::server::domain::configuration::index_profile::{
    make_index_profile_projector, IndexProfileCatalog,
};
use crate::server::domain::configuration::reranker_model::{
    make_reranker_model_projector, RerankerModelCatalog,
};
use crate::server::domain::configuration::retrieval_profile::{
    make_retrieval_profile_projector, RetrievalProfileCatalog,
};
use crate::server::domain::configuration::sweep_template::{SweepTemplate, SweepTemplateProjector};
use crate::server::domain::configuration::vector_index::{
    make_vector_index_projector, VectorIndexCatalog,
};
use crate::server::infrastructure::shared::clients::CloudflareApi;
use crate::server::infrastructure::vector::{
    cloudflare_vector_index_factory as cf_vector, postgres_vector_index as pg_vector,
};
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;

pub struct CatalogServices {
    pub embedding_model_command_handler: Arc<EmbeddingModelCatalogCommandHandler>,
    pub generation_model_command_handler: Arc<GenerationModelCatalogCommandHandler>,
    pub reranker_model_command_handler: Arc<RerankerModelCatalogCommandHandler>,
    pub vector_index_command_handler: Arc<VectorIndexCatalogCommandHandler>,
    pub index_profile_command_handler: Arc<IndexProfileCatalogCommandHandler>,
    pub retrieval_profile_command_handler: Arc<RetrievalProfileCatalogCommandHandler>,
    pub chunking_configuration_command_handler: Arc<ChunkingConfigurationCatalogCommandHandler>,
    pub sweep_template_command_handler: Arc<SweepTemplateCommandHandler>,
    pub configuration_defaults_command_handler: Arc<ConfigurationDefaultsCommandHandler>,
    pub configuration_query_service: Arc<ConfigurationQueryService>,
    pub index_profile_query_service: Arc<IndexProfileQueryService>,
    pub retrieval_profile_query_service: Arc<RetrievalProfileQueryService>,
    pub chunking_configuration_query_service: Arc<ChunkingConfigurationQueryService>,
    pub sweep_template_query_service: Arc<SweepTemplateQueryService>,
    pub index_profile_resolver: Arc<IndexProfileResolver>,
    pub retrieval_profile_resolver: Arc<RetrievalProfileResolver>,
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
    pub rerank_service: Arc<RerankService>,
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
            rerank_service,
            event_bus,
            wakeups,
        } = deps;

        let mut vector_providers = VectorIndexProviderRegistry::new();
        cf_vector::register(&mut vector_providers, cf_api);
        pg_vector::register(&mut vector_providers, pool);
        let vector_index_resolver =
            VectorIndexResolver::new(vector_providers, Arc::clone(&repos.vector_index));

        let index_profile_resolver = IndexProfileResolver::new(
            Arc::clone(&repos.index_profile),
            Arc::clone(&repos.connector),
            Arc::clone(&repos.configuration_defaults),
            Arc::clone(&embedding_service),
            Arc::clone(&vector_index_resolver),
        );

        let retrieval_profile_resolver = RetrievalProfileResolver::new(
            Arc::clone(&repos.retrieval_profile),
            Arc::clone(&repos.connector),
            Arc::clone(&repos.configuration_defaults),
            Arc::clone(&index_profile_resolver),
            Arc::clone(&generation_service),
            Arc::clone(&rerank_service),
        );

        let embedding_model_command_handler = EmbeddingModelCatalogCommandHandler::new(Arc::clone(
            &wirings.embedding_model.command_processor,
        ));
        let generation_model_command_handler = GenerationModelCatalogCommandHandler::new(
            Arc::clone(&wirings.generation_model.command_processor),
        );
        let reranker_model_command_handler = RerankerModelCatalogCommandHandler::new(Arc::clone(
            &wirings.reranker_model.command_processor,
        ));
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
        let index_profile_command_handler = IndexProfileCatalogCommandHandler::new(
            Arc::clone(&wirings.index_profile.command_processor),
            Arc::clone(&repos.embedding_model),
            Arc::clone(&repos.vector_index),
            Arc::clone(&configuration_defaults_command_handler),
        );
        let retrieval_profile_command_handler = RetrievalProfileCatalogCommandHandler::new(
            Arc::clone(&wirings.retrieval_profile.command_processor),
            Arc::clone(&repos.index_profile),
            Arc::clone(&repos.generation_model),
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
            Arc::clone(&repos.reranker_model),
            Arc::clone(&repos.vector_index),
            Arc::clone(&repos.index_profile),
            Arc::clone(&repos.configuration_defaults),
        );
        let index_profile_query_service = IndexProfileQueryService::new(
            Arc::clone(&repos.index_profile),
            Arc::clone(&repos.embedding_model),
            Arc::clone(&repos.vector_index),
            Arc::clone(&repos.configuration_defaults),
        );
        let retrieval_profile_query_service = RetrievalProfileQueryService::new(
            Arc::clone(&repos.retrieval_profile),
            Arc::clone(&repos.index_profile),
            Arc::clone(&repos.generation_model),
            Arc::clone(&repos.reranker_model),
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
            reranker_model_command_handler,
            vector_index_command_handler,
            index_profile_command_handler,
            retrieval_profile_command_handler,
            chunking_configuration_command_handler,
            sweep_template_command_handler,
            configuration_defaults_command_handler,
            configuration_query_service,
            index_profile_query_service,
            retrieval_profile_query_service,
            chunking_configuration_query_service,
            sweep_template_query_service,
            index_profile_resolver,
            retrieval_profile_resolver,
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
    spawn_driver::<RerankerModelCatalog, ()>(
        Arc::clone(&wirings.reranker_model.event_store),
        vec![Arc::new(make_reranker_model_projector(
            Arc::clone(&repos.reranker_model) as _,
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
    spawn_driver::<IndexProfileCatalog, ()>(
        Arc::clone(&wirings.index_profile.event_store),
        vec![Arc::new(make_index_profile_projector(
            Arc::clone(&repos.index_profile) as _,
        ))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<RetrievalProfileCatalog, ()>(
        Arc::clone(&wirings.retrieval_profile.event_store),
        vec![Arc::new(make_retrieval_profile_projector(
            Arc::clone(&repos.retrieval_profile) as _,
        ))],
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
