pub mod chunking;
pub mod embedding;
pub mod evaluation;
pub mod pareto;
pub(crate) mod serde_compat;

pub use chunking::{BertChunkingConfig, ChunkingConfig, LlmChunkingConfig, SectionChunkingConfig};
pub use embedding::{EmbedResult, EmbedderBackend, EmbeddingModel};
pub use evaluation::{
    evaluation_score, ordered_f32_vec, plain_f32_vec, ChunkingVariant, EvaluationAutotuneRequest,
    EvaluationAutotuneSummary, EvaluationGenerationBackend, EvaluationMetrics,
    EvaluationQuestionResult, EvaluationReferenceResult, EvaluationResultSplit,
    EvaluationRunOptions, EvaluationRunResult, EvaluationRunSummary, EvaluationScorePolicy,
    EvaluationScoreWeights, EvaluationSettings, EvaluationVariantResult, OptimizationBudget,
    OptimizationConfig, OptimizationScope, OrderedF32, ReliabilityFlag,
};
pub use pareto::{pareto_frontier, ParetoPoint};
