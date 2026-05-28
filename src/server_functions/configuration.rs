use leptos::prelude::*;

use crate::shared::contracts::{
    ChunkingConfigurationCommandDto, ChunkingConfigurationDto, ConfigurationDto,
    DiscoveryResponseDto, EmbeddingModelCommandDto, GenerationModelCommandDto,
    IndexDiscoveryResponseDto, IndexProfileCommandDto, IndexProfileDto, RerankerModelCommandDto,
    RetrievalProfileCommandDto, RetrievalProfileDto, SweepTemplateCommandDto, SweepTemplateDto,
    VectorIndexCommandDto,
};
use crate::shared::reference_data::{AiProviderKind, ModelCapability, VectorStoreKind};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::catalog::CatalogServices;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::discovery::DiscoveryServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(
    name = GetConfiguration,
    prefix = "/api",
    endpoint = "get_configuration"
)]
pub async fn get_configuration() -> Result<ConfigurationDto, ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .configuration_query_service
        .get()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetIndexProfiles,
    prefix = "/api",
    endpoint = "get_index_profiles"
)]
pub async fn get_index_profiles() -> Result<Vec<IndexProfileDto>, ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .index_profile_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetRetrievalProfiles,
    prefix = "/api",
    endpoint = "get_retrieval_profiles"
)]
pub async fn get_retrieval_profiles() -> Result<Vec<RetrievalProfileDto>, ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .retrieval_profile_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetChunkingConfigurations,
    prefix = "/api",
    endpoint = "get_chunking_configurations"
)]
pub async fn get_chunking_configurations() -> Result<Vec<ChunkingConfigurationDto>, ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .chunking_configuration_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetSweepTemplates,
    prefix = "/api",
    endpoint = "get_sweep_templates"
)]
pub async fn get_sweep_templates() -> Result<Vec<SweepTemplateDto>, ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .sweep_template_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyEmbeddingModelCommand,
    prefix = "/api",
    endpoint = "apply_embedding_model_command"
)]
pub async fn apply_embedding_model_command(
    command: EmbeddingModelCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .embedding_model_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyGenerationModelCommand,
    prefix = "/api",
    endpoint = "apply_generation_model_command"
)]
pub async fn apply_generation_model_command(
    command: GenerationModelCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .generation_model_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyRerankerModelCommand,
    prefix = "/api",
    endpoint = "apply_reranker_model_command"
)]
pub async fn apply_reranker_model_command(
    command: RerankerModelCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .reranker_model_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyVectorIndexCommand,
    prefix = "/api",
    endpoint = "apply_vector_index_command"
)]
pub async fn apply_vector_index_command(
    command: VectorIndexCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .vector_index_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyIndexProfileCommand,
    prefix = "/api",
    endpoint = "apply_index_profile_command"
)]
pub async fn apply_index_profile_command(
    command: IndexProfileCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .index_profile_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyRetrievalProfileCommand,
    prefix = "/api",
    endpoint = "apply_retrieval_profile_command"
)]
pub async fn apply_retrieval_profile_command(
    command: RetrievalProfileCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .retrieval_profile_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyChunkingConfigurationCommand,
    prefix = "/api",
    endpoint = "apply_chunking_configuration_command"
)]
pub async fn apply_chunking_configuration_command(
    command: ChunkingConfigurationCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .chunking_configuration_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplySweepTemplateCommand,
    prefix = "/api",
    endpoint = "apply_sweep_template_command"
)]
pub async fn apply_sweep_template_command(
    command: SweepTemplateCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<CatalogServices>>()?
        .sweep_template_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = DiscoverModels,
    prefix = "/api",
    endpoint = "discover_models"
)]
pub async fn discover_models(
    provider: AiProviderKind,
    capability: ModelCapability,
) -> Result<DiscoveryResponseDto, ServerFnError> {
    Ok(ctx::<Arc<DiscoveryServices>>()?
        .discovery_query_service
        .discover(provider, capability)
        .await)
}

#[server(
    name = DiscoverVectorIndexes,
    prefix = "/api",
    endpoint = "discover_vector_indexes"
)]
pub async fn discover_vector_indexes(
    kind: VectorStoreKind,
) -> Result<IndexDiscoveryResponseDto, ServerFnError> {
    Ok(ctx::<Arc<DiscoveryServices>>()?
        .index_discovery_query_service
        .discover(kind)
        .await)
}
