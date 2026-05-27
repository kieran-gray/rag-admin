pub mod chunking;
pub mod contracts;
pub mod embedding;
pub mod evaluation;
pub mod pareto;
pub mod reference_data;
pub(crate) mod serde_compat;

pub use chunking::{
    BertChunkingConfig, ChunkingConfig, DarnChunkingConfig, DarnGranularity, LlmChunkingConfig,
    SectionChunkingConfig,
};
pub use embedding::{
    EmbedInputType, EmbedManyCandidate, EmbedManyResult, EmbedMatrixResult, EmbedResult,
    EmbedderBackend, EmbeddingModel,
};
pub use evaluation::{
    evaluation_score, evaluation_score_with_judge, ordered_f32_vec, plain_f32_vec, ChunkingVariant,
    EvaluationMetrics, EvaluationQuestionResult, EvaluationReferenceResult, EvaluationResultSplit,
    EvaluationRunOptions, EvaluationRunResult, EvaluationRunSummary, EvaluationScorePolicy,
    EvaluationScoreWeights, EvaluationVariantResult, OptimizationBudget, OptimizationConfig,
    OptimizationScope, OrderedF32, ReliabilityFlag, JUDGE_SCORE_WEIGHT, RECALL_FLOOR,
};
pub use pareto::{pareto_frontier, ParetoPoint};
