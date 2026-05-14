mod activity;
mod chunking;
mod configuration;
mod configuration_commands;
mod embedding;
mod evaluation;
mod events;
mod ingest;
mod query;
pub(crate) mod serde_compat;
mod settings;
mod source_document;
mod vector;

pub use activity::{
    classify, classify_event, ActivityDelta, ActivityJobDto, ActivityKind, ActivityStart,
    ActivityStatus,
};
pub use chunking::{
    BertChunkingConfig, ChunkParamDefinition, ChunkParamKey, ChunkStrategy, ChunkerDefinition,
    ChunkingConfig, LlmChunkingConfig, SectionChunkingConfig,
};
pub use configuration::{
    AiProviderKindDto, ChunkingConfigurationDto, ConfigurationDto, EmbeddingModelDto,
    GenerationModelDto, PipelineConfigurationDto, SweepTemplateDto, VectorIndexDto,
    VectorStoreKindDto,
};
pub use configuration_commands::{
    AddEmbeddingModelDto, AddGenerationModelDto, AddVectorIndexDto,
    ChunkingConfigurationCommandDto, CreateChunkingConfigurationDto,
    CreatePipelineConfigurationDto, CreateSweepTemplateDto, DeleteChunkingConfigurationDto,
    DeletePipelineConfigurationDto, DeleteSweepTemplateDto, EmbeddingModelCommandDto,
    GenerationModelCommandDto, PipelineConfigurationCommandDto, RemoveEmbeddingModelDto,
    RemoveGenerationModelDto, RemoveVectorIndexDto, SetDefaultSweepTemplateDto,
    SweepTemplateCommandDto, UpdateChunkingConfigurationDto, UpdateEmbeddingModelDto,
    UpdateGenerationModelDto, UpdatePipelineConfigurationDto, UpdateSweepTemplateDto,
    UpdateVectorIndexDto, VectorIndexCommandDto,
};
pub use embedding::{
    catalog_for_backend, CatalogEntry, EmbedResult, EmbedderBackend, EmbeddingModel,
    CLOUDFLARE_EMBEDDING_MODELS, OLLAMA_EMBEDDING_MODELS,
};
pub use evaluation::{
    evaluation_score, ordered_f32_vec, plain_f32_vec, ChunkingVariant, EvaluationAutotuneRequest,
    EvaluationAutotuneSummary, EvaluationDatasetDto, EvaluationDatasetSummaryDto,
    EvaluationGenerationBackend, EvaluationJobInfo, EvaluationMetrics, EvaluationQuestionDto,
    EvaluationQuestionResult, EvaluationReferenceDto, EvaluationReferenceResult,
    EvaluationResultSplit, EvaluationRunDto, EvaluationRunOptions, EvaluationRunResult,
    EvaluationRunSummary, EvaluationRunSummaryDto, EvaluationScorePolicy, EvaluationScoreWeights,
    EvaluationSettings, EvaluationVariantResult, RunEvaluationRequestDto,
};
pub use events::{aggregate as aggregate_type, PublishedEvent};
pub use ingest::{IngestOptions, LogEvent, LogLevel};
pub use query::{QueryHit, QueryRequest, QueryResult};
pub use settings::SettingsDto;
pub use source_document::{
    ChunkDto, DocumentListItemDto, IndexingDto, MarkdownBlockDto, MarkdownBlockKindDto,
    SourceDocumentDetailDto, SourceDocumentDto, SourceDocumentMarkdownDto,
};
pub use vector::{VectorIndexConfig, VectorProvider};
