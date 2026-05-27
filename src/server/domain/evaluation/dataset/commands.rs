use uuid::Uuid;

use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::shared::Timestamp;

use super::super::question::{EvaluationReference, QuestionDimensions};
use super::events::AxisWeight;

pub struct RequestDatasetGeneration {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub label: String,
    pub target_question_count: u32,
    pub generation_model_id: Uuid,
    pub generation_model: String,
    pub duplicate_similarity_threshold_milli: u32,
    pub embedding_model_id: Uuid,
    pub max_attempts: u32,
    pub weight_matrix: Vec<AxisWeight>,
    pub occurred_at: Timestamp,
}

pub struct ResolveMapDependency {
    pub dataset_id: Uuid,
    pub map_id: Uuid,
    pub ready: bool,
    pub failure_reason: Option<String>,
    pub occurred_at: Timestamp,
}

pub struct AcceptQuestion {
    pub dataset_id: Uuid,
    pub sequence: u32,
    pub question: String,
    pub references: Vec<EvaluationReference>,
    pub embedding: Option<Vec<f32>>,
    pub dimensions: QuestionDimensions,
    pub evidence_refs: Vec<MapItemRef>,
    pub occurred_at: Timestamp,
}

pub struct RejectQuestion {
    pub dataset_id: Uuid,
    pub attempt: u32,
    pub reason: String,
    pub occurred_at: Timestamp,
}

pub struct CompleteDatasetGeneration {
    pub dataset_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct FailDatasetGeneration {
    pub dataset_id: Uuid,
    pub reason: String,
    pub occurred_at: Timestamp,
}

pub struct CancelDatasetGeneration {
    pub dataset_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct RenameDataset {
    pub dataset_id: Uuid,
    pub label: String,
    pub occurred_at: Timestamp,
}

pub struct DeleteDataset {
    pub dataset_id: Uuid,
    pub occurred_at: Timestamp,
}

pub enum EvaluationDatasetCommand {
    RequestDatasetGeneration(RequestDatasetGeneration),
    ResolveMapDependency(ResolveMapDependency),
    AcceptQuestion(AcceptQuestion),
    RejectQuestion(RejectQuestion),
    CompleteDatasetGeneration(CompleteDatasetGeneration),
    FailDatasetGeneration(FailDatasetGeneration),
    CancelDatasetGeneration(CancelDatasetGeneration),
    RenameDataset(RenameDataset),
    DeleteDataset(DeleteDataset),
}

impl EvaluationDatasetCommand {
    pub fn dataset_id(&self) -> Uuid {
        match self {
            Self::RequestDatasetGeneration(c) => c.dataset_id,
            Self::ResolveMapDependency(c) => c.dataset_id,
            Self::AcceptQuestion(c) => c.dataset_id,
            Self::RejectQuestion(c) => c.dataset_id,
            Self::CompleteDatasetGeneration(c) => c.dataset_id,
            Self::FailDatasetGeneration(c) => c.dataset_id,
            Self::CancelDatasetGeneration(c) => c.dataset_id,
            Self::RenameDataset(c) => c.dataset_id,
            Self::DeleteDataset(c) => c.dataset_id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::RequestDatasetGeneration(_) => "request_dataset_generation",
            Self::ResolveMapDependency(_) => "resolve_map_dependency",
            Self::AcceptQuestion(_) => "accept_question",
            Self::RejectQuestion(_) => "reject_question",
            Self::CompleteDatasetGeneration(_) => "complete_dataset_generation",
            Self::FailDatasetGeneration(_) => "fail_dataset_generation",
            Self::CancelDatasetGeneration(_) => "cancel_dataset_generation",
            Self::RenameDataset(_) => "rename_dataset",
            Self::DeleteDataset(_) => "delete_dataset",
        }
    }
}
