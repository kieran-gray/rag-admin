use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::super::question::EvaluationReference;

pub struct RequestDatasetGeneration {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub label: String,
    pub target_question_count: u32,
    pub generation_model_id: Uuid,
    pub generation_model: String,
    pub excerpt_similarity_threshold_milli: u32,
    pub duplicate_similarity_threshold_milli: u32,
    pub embedding_model_id: Uuid,
    pub occurred_at: Timestamp,
}

pub struct AcceptQuestion {
    pub dataset_id: Uuid,
    pub sequence: u32,
    pub question: String,
    pub references: Vec<EvaluationReference>,
    pub embedding: Option<Vec<f32>>,
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
    AcceptQuestion(AcceptQuestion),
    RejectQuestion(RejectQuestion),
    CompleteDatasetGeneration(CompleteDatasetGeneration),
    FailDatasetGeneration(FailDatasetGeneration),
    RenameDataset(RenameDataset),
    DeleteDataset(DeleteDataset),
}

impl EvaluationDatasetCommand {
    pub fn dataset_id(&self) -> Uuid {
        match self {
            Self::RequestDatasetGeneration(c) => c.dataset_id,
            Self::AcceptQuestion(c) => c.dataset_id,
            Self::RejectQuestion(c) => c.dataset_id,
            Self::CompleteDatasetGeneration(c) => c.dataset_id,
            Self::FailDatasetGeneration(c) => c.dataset_id,
            Self::RenameDataset(c) => c.dataset_id,
            Self::DeleteDataset(c) => c.dataset_id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::RequestDatasetGeneration(_) => "request_dataset_generation",
            Self::AcceptQuestion(_) => "accept_question",
            Self::RejectQuestion(_) => "reject_question",
            Self::CompleteDatasetGeneration(_) => "complete_dataset_generation",
            Self::FailDatasetGeneration(_) => "fail_dataset_generation",
            Self::RenameDataset(_) => "rename_dataset",
            Self::DeleteDataset(_) => "delete_dataset",
        }
    }
}
