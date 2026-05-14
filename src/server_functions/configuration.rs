use leptos::prelude::*;

use crate::shared::{
    ChunkingConfigurationCommandDto, ChunkingConfigurationDto, ConfigurationDto,
    EmbeddingModelCommandDto, GenerationModelCommandDto, PipelineConfigurationCommandDto,
    PipelineConfigurationDto, SweepTemplateCommandDto, SweepTemplateDto, VectorIndexCommandDto,
};

#[cfg(feature = "ssr")]
use crate::server::application::configuration::{
    ChunkingConfigurationQueryService, ChunkingConfigurationService, ConfigurationQueryService,
    EmbeddingModelCatalogCommandHandler, GenerationModelCatalogCommandHandler,
    PipelineConfigurationQueryService, PipelineConfigurationService, SweepTemplateCommandHandler,
    SweepTemplateQueryService, VectorIndexCatalogCommandHandler,
};
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
    ctx::<Arc<ConfigurationQueryService>>()?
        .get()
        .await
        .map_err(map_app_error)
}

#[server(
    name = GetPipelineConfigurations,
    prefix = "/api",
    endpoint = "get_pipeline_configurations"
)]
pub async fn get_pipeline_configurations() -> Result<Vec<PipelineConfigurationDto>, ServerFnError> {
    ctx::<Arc<PipelineConfigurationQueryService>>()?
        .list()
        .await
        .map_err(map_app_error)
}

#[server(
    name = GetChunkingConfigurations,
    prefix = "/api",
    endpoint = "get_chunking_configurations"
)]
pub async fn get_chunking_configurations() -> Result<Vec<ChunkingConfigurationDto>, ServerFnError> {
    ctx::<Arc<ChunkingConfigurationQueryService>>()?
        .list()
        .await
        .map_err(map_app_error)
}

#[server(
    name = GetSweepTemplates,
    prefix = "/api",
    endpoint = "get_sweep_templates"
)]
pub async fn get_sweep_templates() -> Result<Vec<SweepTemplateDto>, ServerFnError> {
    ctx::<Arc<SweepTemplateQueryService>>()?
        .list()
        .await
        .map_err(map_app_error)
}

#[server(
    name = ApplyEmbeddingModelCommand,
    prefix = "/api",
    endpoint = "apply_embedding_model_command"
)]
pub async fn apply_embedding_model_command(
    command: EmbeddingModelCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<EmbeddingModelCatalogCommandHandler>>()?
        .handle_dto(command)
        .await
        .map_err(map_app_error)
}

#[server(
    name = ApplyGenerationModelCommand,
    prefix = "/api",
    endpoint = "apply_generation_model_command"
)]
pub async fn apply_generation_model_command(
    command: GenerationModelCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<GenerationModelCatalogCommandHandler>>()?
        .handle_dto(command)
        .await
        .map_err(map_app_error)
}

#[server(
    name = ApplyVectorIndexCommand,
    prefix = "/api",
    endpoint = "apply_vector_index_command"
)]
pub async fn apply_vector_index_command(
    command: VectorIndexCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<VectorIndexCatalogCommandHandler>>()?
        .handle_dto(command)
        .await
        .map_err(map_app_error)
}

#[server(
    name = ApplyPipelineConfigurationCommand,
    prefix = "/api",
    endpoint = "apply_pipeline_configuration_command"
)]
pub async fn apply_pipeline_configuration_command(
    command: PipelineConfigurationCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<PipelineConfigurationService>>()?
        .handle_dto(command)
        .await
        .map_err(map_app_error)
}

#[server(
    name = ApplyChunkingConfigurationCommand,
    prefix = "/api",
    endpoint = "apply_chunking_configuration_command"
)]
pub async fn apply_chunking_configuration_command(
    command: ChunkingConfigurationCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<ChunkingConfigurationService>>()?
        .handle_dto(command)
        .await
        .map_err(map_app_error)
}

#[server(
    name = ApplySweepTemplateCommand,
    prefix = "/api",
    endpoint = "apply_sweep_template_command"
)]
pub async fn apply_sweep_template_command(
    command: SweepTemplateCommandDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<SweepTemplateCommandHandler>>()?
        .handle_dto(command)
        .await
        .map_err(map_app_error)
}
