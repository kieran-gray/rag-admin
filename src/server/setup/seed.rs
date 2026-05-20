use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::{
    ChunkingConfigurationCatalogCommandHandler, ChunkingConfigurationQueryService,
    ConfigurationQueryService, EmbeddingModelCatalogCommandHandler,
    GenerationModelCatalogCommandHandler, PipelineConfigurationCatalogCommandHandler,
    PipelineConfigurationQueryService, SweepTemplateCommandHandler, SweepTemplateQueryService,
    VectorIndexCatalogCommandHandler,
};
use crate::shared::contracts::{
    AddEmbeddingModelDto, AddGenerationModelDto, AddVectorIndexDto,
    ChunkingConfigurationCommandDto, CreateChunkingConfigurationDto,
    CreatePipelineConfigurationDto, CreateSweepTemplateDto, EmbeddingModelCommandDto,
    GenerationModelCommandDto, PipelineConfigurationCommandDto, SetDefaultSweepTemplateDto,
    SweepTemplateCommandDto, VectorIndexCommandDto,
};
use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};
use crate::shared::{
    BertChunkingConfig, ChunkingConfig, DarnChunkingConfig, DarnGranularity, LlmChunkingConfig,
    SectionChunkingConfig,
};

const DEFAULT_SWEEP_NAME: &str = "default-sweep";
const DEFAULT_PIPELINE_NAME: &str = "default";
const DEFAULT_CHUNKING_NAME: &str = "section-384";
const DEFAULT_VECTOR_INDEX_NAME: &str = "default-pgvector";
const DEFAULT_EMBEDDING_MODEL: &str = "qwen3-embedding:0.6b";
const DEFAULT_GENERATION_MODEL: &str = "gemma3:12b-it-qat";

pub struct ChunkingSeed {
    pub name: &'static str,
    pub config: ChunkingConfig,
}

struct EmbeddingSeed {
    kind: AiProviderKind,
    model: &'static str,
    dimensions: u32,
}

struct GenerationSeed {
    kind: AiProviderKind,
    model: &'static str,
}

struct VectorIndexSeed {
    kind: VectorStoreKind,
    name: &'static str,
    dimensions: u32,
}

const EMBEDDING_SEEDS: &[EmbeddingSeed] = &[
    EmbeddingSeed {
        kind: AiProviderKind::Cloudflare,
        model: "@cf/baai/bge-base-en-v1.5",
        dimensions: 768,
    },
    EmbeddingSeed {
        kind: AiProviderKind::Cloudflare,
        model: "@cf/qwen/qwen3-embedding-0.6b",
        dimensions: 1024,
    },
    EmbeddingSeed {
        kind: AiProviderKind::Ollama,
        model: "qwen3-embedding:0.6b",
        dimensions: 1024,
    },
];

const GENERATION_SEEDS: &[GenerationSeed] = &[
    GenerationSeed {
        kind: AiProviderKind::Cloudflare,
        model: "@cf/zai-org/glm-4.7-flash",
    },
    GenerationSeed {
        kind: AiProviderKind::Cloudflare,
        model: "@cf/google/gemma-4-26b-a4b-it",
    },
    GenerationSeed {
        kind: AiProviderKind::Ollama,
        model: "gemma3:12b-it-qat",
    },
];

const VECTOR_INDEX_SEEDS: &[VectorIndexSeed] = &[
    VectorIndexSeed {
        kind: VectorStoreKind::CloudflareVectorize,
        name: "document-chunks",
        dimensions: 1024,
    },
    VectorIndexSeed {
        kind: VectorStoreKind::Postgres,
        name: DEFAULT_VECTOR_INDEX_NAME,
        dimensions: 1024,
    },
];

const LLM_CHUNKING_MODEL: &str = "gemma3:12b-it-qat";

pub fn seed_definitions(llm_generation_model_id: Option<Uuid>) -> Vec<ChunkingSeed> {
    let mut seeds = Vec::new();

    for max_section_tokens in [256u32, 384, 480, 512] {
        seeds.push(ChunkingSeed {
            name: leak_name(format!("section-{max_section_tokens}")),
            config: ChunkingConfig::Section(SectionChunkingConfig { max_section_tokens }),
        });
    }

    for (target, overlap) in [(256u32, 0u32), (384, 64), (448, 64)] {
        seeds.push(ChunkingSeed {
            name: leak_name(format!("bert-{target}-{overlap}")),
            config: ChunkingConfig::Bert(BertChunkingConfig {
                target_tokens: target,
                overlap_tokens: overlap,
                min_tokens: 96,
            }),
        });
    }

    for (max_chunk_size, overlap) in [(500u32, 50u32), (1000, 100)] {
        seeds.push(ChunkingSeed {
            name: leak_name(format!("darn-{max_chunk_size}-{overlap}")),
            config: ChunkingConfig::Darn(DarnChunkingConfig {
                max_chunk_size,
                overlap,
                granularity: DarnGranularity::Characters,
            }),
        });
    }

    if let Some(generation_model_id) = llm_generation_model_id {
        for (target, micro) in [(480u32, 64u32), (480, 96), (480, 128)] {
            seeds.push(ChunkingSeed {
                name: leak_name(format!("llm-{target}-{micro}")),
                config: ChunkingConfig::Llm(LlmChunkingConfig {
                    target_tokens: target,
                    micro_chunk_tokens: micro,
                    generation_model_id,
                }),
            });
        }
    }

    seeds
}

fn leak_name(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[allow(clippy::too_many_arguments)]
pub async fn seed_if_empty(
    chunking_query: &Arc<ChunkingConfigurationQueryService>,
    sweep_template_query: &Arc<SweepTemplateQueryService>,
    configuration_query: &Arc<ConfigurationQueryService>,
    pipeline_query: &Arc<PipelineConfigurationQueryService>,
    embedding_handler: &Arc<EmbeddingModelCatalogCommandHandler>,
    generation_handler: &Arc<GenerationModelCatalogCommandHandler>,
    vector_index_handler: &Arc<VectorIndexCatalogCommandHandler>,
    chunking_handler: &Arc<ChunkingConfigurationCatalogCommandHandler>,
    pipeline_handler: &Arc<PipelineConfigurationCatalogCommandHandler>,
    sweep_template_handler: &Arc<SweepTemplateCommandHandler>,
) -> Result<(), String> {
    seed_models_if_empty(configuration_query, embedding_handler, generation_handler).await?;
    seed_vector_indexes_if_empty(configuration_query, vector_index_handler).await?;
    seed_chunking_configurations_if_empty(chunking_query, configuration_query, chunking_handler)
        .await?;
    seed_default_pipeline_if_empty(configuration_query, pipeline_query, pipeline_handler).await?;
    seed_default_sweep_template_if_empty(
        chunking_query,
        sweep_template_query,
        sweep_template_handler,
    )
    .await?;
    Ok(())
}

async fn seed_default_pipeline_if_empty(
    configuration_query: &Arc<ConfigurationQueryService>,
    pipeline_query: &Arc<PipelineConfigurationQueryService>,
    pipeline_handler: &Arc<PipelineConfigurationCatalogCommandHandler>,
) -> Result<(), String> {
    let existing = pipeline_query.list().await.map_err(|e| e.to_string())?;
    if !existing.is_empty() {
        return Ok(());
    }

    let catalog = configuration_query.get().await.map_err(|e| e.to_string())?;
    let Some(embedding) = catalog
        .embedding_models
        .iter()
        .find(|m| m.model == DEFAULT_EMBEDDING_MODEL && m.kind == AiProviderKind::Ollama)
    else {
        return Ok(());
    };
    let Some(generation) = catalog
        .generation_models
        .iter()
        .find(|m| m.model == DEFAULT_GENERATION_MODEL && m.kind == AiProviderKind::Ollama)
    else {
        return Ok(());
    };
    let Some(vector_index) = catalog
        .vector_indexes
        .iter()
        .find(|i| i.name == DEFAULT_VECTOR_INDEX_NAME && i.kind == VectorStoreKind::Postgres)
    else {
        return Ok(());
    };

    pipeline_handler
        .handle_dto(
            PipelineConfigurationCommandDto::CreatePipelineConfiguration(
                CreatePipelineConfigurationDto {
                    name: DEFAULT_PIPELINE_NAME.to_owned(),
                    embedding_model_id: embedding.embedding_model_id,
                    generation_model_id: generation.generation_model_id,
                    vector_index_id: vector_index.index_id,
                    is_default: true,
                },
            ),
        )
        .await
        .map_err(|e| format!("seed default pipeline: {e}"))?;
    Ok(())
}

async fn seed_default_sweep_template_if_empty(
    chunking_query: &Arc<ChunkingConfigurationQueryService>,
    sweep_template_query: &Arc<SweepTemplateQueryService>,
    sweep_template_handler: &Arc<SweepTemplateCommandHandler>,
) -> Result<(), String> {
    let existing = sweep_template_query
        .list()
        .await
        .map_err(|e| e.to_string())?;
    if !existing.is_empty() {
        return Ok(());
    }

    let chunking_configs = chunking_query.list().await.map_err(|e| e.to_string())?;
    if chunking_configs.is_empty() {
        return Ok(());
    }

    let members: Vec<_> = chunking_configs
        .into_iter()
        .map(|cc| cc.chunking_configuration_id)
        .collect();

    sweep_template_handler
        .handle_dto(SweepTemplateCommandDto::CreateSweepTemplate(
            CreateSweepTemplateDto {
                name: DEFAULT_SWEEP_NAME.to_owned(),
                members,
            },
        ))
        .await
        .map_err(|e| format!("seed default sweep: {e}"))?;

    let templates = sweep_template_query
        .list()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(t) = templates.into_iter().find(|t| t.name == DEFAULT_SWEEP_NAME) {
        sweep_template_handler
            .handle_dto(SweepTemplateCommandDto::SetDefaultSweepTemplate(
                SetDefaultSweepTemplateDto {
                    sweep_template_id: t.sweep_template_id,
                },
            ))
            .await
            .map_err(|e| format!("set default sweep: {e}"))?;
    }
    Ok(())
}

async fn seed_models_if_empty(
    configuration_query: &Arc<ConfigurationQueryService>,
    embedding_handler: &Arc<EmbeddingModelCatalogCommandHandler>,
    generation_handler: &Arc<GenerationModelCatalogCommandHandler>,
) -> Result<(), String> {
    let catalog = configuration_query.get().await.map_err(|e| e.to_string())?;

    if catalog.embedding_models.is_empty() {
        for seed in EMBEDDING_SEEDS {
            embedding_handler
                .handle_dto(EmbeddingModelCommandDto::AddEmbeddingModel(
                    AddEmbeddingModelDto {
                        kind: seed.kind,
                        model: seed.model.to_owned(),
                        dimensions: seed.dimensions,
                    },
                ))
                .await
                .map_err(|e| format!("seed embedding {}: {e}", seed.model))?;
        }
    }

    if catalog.generation_models.is_empty() {
        for seed in GENERATION_SEEDS {
            generation_handler
                .handle_dto(GenerationModelCommandDto::AddGenerationModel(
                    AddGenerationModelDto {
                        kind: seed.kind,
                        model: seed.model.to_owned(),
                    },
                ))
                .await
                .map_err(|e| format!("seed generation {}: {e}", seed.model))?;
        }
    }

    Ok(())
}

async fn seed_vector_indexes_if_empty(
    configuration_query: &Arc<ConfigurationQueryService>,
    vector_index_handler: &Arc<VectorIndexCatalogCommandHandler>,
) -> Result<(), String> {
    let catalog = configuration_query.get().await.map_err(|e| e.to_string())?;
    if !catalog.vector_indexes.is_empty() {
        return Ok(());
    }

    for seed in VECTOR_INDEX_SEEDS {
        vector_index_handler
            .handle_dto(VectorIndexCommandDto::AddVectorIndex(AddVectorIndexDto {
                kind: seed.kind,
                name: seed.name.to_owned(),
                dimensions: seed.dimensions,
            }))
            .await
            .map_err(|e| format!("seed vector index {}: {e}", seed.name))?;
    }

    Ok(())
}

async fn seed_chunking_configurations_if_empty(
    chunking_query: &Arc<ChunkingConfigurationQueryService>,
    configuration_query: &Arc<ConfigurationQueryService>,
    chunking_handler: &Arc<ChunkingConfigurationCatalogCommandHandler>,
) -> Result<(), String> {
    let existing = chunking_query.list().await.map_err(|e| e.to_string())?;
    if !existing.is_empty() {
        return Ok(());
    }

    let llm_generation_model_id = configuration_query
        .get()
        .await
        .map_err(|e| e.to_string())?
        .generation_models
        .iter()
        .find(|m| m.model == LLM_CHUNKING_MODEL)
        .map(|m| m.generation_model_id);

    for seed in seed_definitions(llm_generation_model_id) {
        let is_default = seed.name == DEFAULT_CHUNKING_NAME;
        chunking_handler
            .handle_dto(
                ChunkingConfigurationCommandDto::CreateChunkingConfiguration(
                    CreateChunkingConfigurationDto {
                        name: seed.name.to_owned(),
                        config: seed.config,
                        is_default,
                    },
                ),
            )
            .await
            .map_err(|e| format!("seed {}: {e}", seed.name))?;
    }
    Ok(())
}
