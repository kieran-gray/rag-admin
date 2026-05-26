use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::Aggregate;

use crate::server::domain::comprehension::map::aggregate::derive_map_id;
use crate::server::domain::evaluation::question::{CognitiveOperation, EvidenceKind};

use super::{
    commands::EvaluationDatasetCommand,
    events::{
        AxisWeight, DatasetDeleted, DatasetGenerationCancelled, DatasetGenerationCompleted,
        DatasetGenerationFailed, DatasetGenerationRequested, DatasetRenamed,
        EvaluationDatasetEvent, MapDependencyResolved, QuestionAccepted, QuestionRejected,
    },
    exceptions::EvaluationDatasetError,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatasetGenerationStatus {
    AwaitingMap,
    Generating,
    Completed,
    Cancelled,
    Failed { reason: String },
}

impl DatasetGenerationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AwaitingMap => "awaiting_map",
            Self::Generating => "generating",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed { .. } => "failed",
        }
    }

    pub fn from_parts(status: &str, failure_reason: Option<String>) -> Result<Self, String> {
        match status {
            "awaiting_map" => Ok(Self::AwaitingMap),
            "generating" => Ok(Self::Generating),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed {
                reason: failure_reason.unwrap_or_default(),
            }),
            other => Err(format!("unknown dataset status '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MapDependencyStatus {
    Pending,
    Ready,
    Failed,
}

pub type AxisKey = (CognitiveOperation, EvidenceKind);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDataset {
    pub dataset_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub target_question_count: u32,
    pub status: DatasetGenerationStatus,
    pub accepted_sequences: BTreeSet<u32>,
    pub deleted: bool,
    pub generation_model_id: Uuid,
    pub embedding_model_id: Uuid,
    pub duplicate_similarity_threshold_milli: u32,
    pub max_attempts: u32,
    pub attempt_count: u32,
    pub accepted_by_axes: HashMap<String, u32>,
    pub weight_matrix: Vec<AxisWeight>,
    pub map_id: Uuid,
    pub map_status: MapDependencyStatus,
    pub map_failure_reason: Option<String>,
}

impl EvaluationDataset {
    fn from_requested(e: &DatasetGenerationRequested) -> Self {
        let map_id = derive_map_id(e.document_id, e.document_version, &e.content_hash);
        Self {
            dataset_id: e.dataset_id,
            document_id: e.document_id,
            document_version: e.document_version,
            content_hash: e.content_hash.clone(),
            target_question_count: e.target_question_count,
            status: DatasetGenerationStatus::AwaitingMap,
            accepted_sequences: BTreeSet::new(),
            deleted: false,
            generation_model_id: e.generation_model_id,
            embedding_model_id: e.embedding_model_id,
            duplicate_similarity_threshold_milli: e.duplicate_similarity_threshold_milli,
            max_attempts: e.max_attempts,
            attempt_count: 0,
            accepted_by_axes: HashMap::new(),
            weight_matrix: e.weight_matrix.clone(),
            map_id,
            map_status: MapDependencyStatus::Pending,
            map_failure_reason: None,
        }
    }

    pub fn clean_accepted_count(&self) -> u32 {
        self.accepted_by_axes.values().copied().sum()
    }

    pub fn next_sequence(&self) -> u32 {
        self.accepted_sequences
            .iter()
            .copied()
            .max()
            .map(|n| n + 1)
            .unwrap_or(0)
    }
}

pub fn axis_key(operation: CognitiveOperation, evidence: EvidenceKind) -> String {
    format!("{}:{}", operation.as_str(), evidence.as_str())
}

impl Aggregate for EvaluationDataset {
    type Event = EvaluationDatasetEvent;
    type Command = EvaluationDatasetCommand;
    type Error = EvaluationDatasetError;

    fn aggregate_type() -> &'static str {
        "evaluation_dataset"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::DatasetGenerationRequested(_) | Self::Event::DatasetRenamed(_) => {}
            Self::Event::MapDependencyResolved(e) => {
                if e.ready {
                    self.map_status = MapDependencyStatus::Ready;
                    self.map_failure_reason = None;
                    if matches!(self.status, DatasetGenerationStatus::AwaitingMap) {
                        self.status = DatasetGenerationStatus::Generating;
                    }
                } else {
                    self.map_status = MapDependencyStatus::Failed;
                    self.map_failure_reason.clone_from(&e.failure_reason);
                    self.status = DatasetGenerationStatus::Failed {
                        reason: e
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| "map dependency failed".into()),
                    };
                }
            }
            Self::Event::QuestionAccepted(e) => {
                self.accepted_sequences.insert(e.sequence);
                let key = axis_key(e.dimensions.operation, e.dimensions.evidence);
                *self.accepted_by_axes.entry(key).or_insert(0) += 1;
                self.attempt_count = self.attempt_count.saturating_add(1);
            }
            Self::Event::QuestionRejected(_) => {
                self.attempt_count = self.attempt_count.saturating_add(1);
            }
            Self::Event::DatasetGenerationCompleted(_) => {
                self.status = DatasetGenerationStatus::Completed;
            }
            Self::Event::DatasetGenerationFailed(e) => {
                self.status = DatasetGenerationStatus::Failed {
                    reason: e.reason.clone(),
                };
            }
            Self::Event::DatasetGenerationCancelled(_) => {
                self.status = DatasetGenerationStatus::Cancelled;
            }
            Self::Event::DatasetDeleted(_) => {
                self.deleted = true;
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::RequestDatasetGeneration(cmd) => {
                if state.is_some() {
                    return Err(EvaluationDatasetError::AlreadyExists);
                }
                if cmd.weight_matrix.is_empty() {
                    return Err(EvaluationDatasetError::InvalidCommand(
                        "weight_matrix must not be empty".into(),
                    ));
                }
                Ok(vec![Self::Event::DatasetGenerationRequested(
                    DatasetGenerationRequested {
                        dataset_id: cmd.dataset_id,
                        document_id: cmd.document_id,
                        document_version: cmd.document_version,
                        content_hash: cmd.content_hash,
                        label: cmd.label,
                        target_question_count: cmd.target_question_count,
                        generation_model_id: cmd.generation_model_id,
                        generation_model: cmd.generation_model,
                        duplicate_similarity_threshold_milli: cmd
                            .duplicate_similarity_threshold_milli,
                        embedding_model_id: cmd.embedding_model_id,
                        max_attempts: cmd.max_attempts,
                        weight_matrix: cmd.weight_matrix,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }

            Self::Command::ResolveMapDependency(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                if matches!(
                    dataset.status,
                    DatasetGenerationStatus::Completed
                        | DatasetGenerationStatus::Cancelled
                        | DatasetGenerationStatus::Failed { .. }
                ) {
                    return Ok(vec![]);
                }
                if cmd.ready && matches!(dataset.map_status, MapDependencyStatus::Ready) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::MapDependencyResolved(
                    MapDependencyResolved {
                        dataset_id: dataset.dataset_id,
                        map_id: cmd.map_id,
                        ready: cmd.ready,
                        failure_reason: cmd.failure_reason,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }

            Self::Command::AcceptQuestion(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                if !matches!(dataset.status, DatasetGenerationStatus::Generating) {
                    return Err(EvaluationDatasetError::GenerationNotInProgress);
                }
                if dataset.accepted_sequences.contains(&cmd.sequence) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::QuestionAccepted(QuestionAccepted {
                    dataset_id: dataset.dataset_id,
                    sequence: cmd.sequence,
                    question: cmd.question,
                    references: cmd.references,
                    embedding: cmd.embedding,
                    dimensions: cmd.dimensions,
                    evidence_refs: cmd.evidence_refs,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::RejectQuestion(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                if !matches!(dataset.status, DatasetGenerationStatus::Generating) {
                    return Err(EvaluationDatasetError::GenerationNotInProgress);
                }
                Ok(vec![Self::Event::QuestionRejected(QuestionRejected {
                    dataset_id: dataset.dataset_id,
                    attempt: cmd.attempt,
                    reason: cmd.reason,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::CompleteDatasetGeneration(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                match &dataset.status {
                    DatasetGenerationStatus::Completed => return Ok(vec![]),
                    DatasetGenerationStatus::Failed { .. } => {
                        return Err(EvaluationDatasetError::AlreadyFailed)
                    }
                    DatasetGenerationStatus::Cancelled => {
                        return Err(EvaluationDatasetError::AlreadyCancelled)
                    }
                    DatasetGenerationStatus::AwaitingMap | DatasetGenerationStatus::Generating => {}
                }
                if dataset.accepted_sequences.is_empty() {
                    return Err(EvaluationDatasetError::NoQuestionsAccepted);
                }
                Ok(vec![Self::Event::DatasetGenerationCompleted(
                    DatasetGenerationCompleted {
                        dataset_id: dataset.dataset_id,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }

            Self::Command::FailDatasetGeneration(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                match &dataset.status {
                    DatasetGenerationStatus::Completed => {
                        return Err(EvaluationDatasetError::AlreadyCompleted)
                    }
                    DatasetGenerationStatus::Cancelled => {
                        return Err(EvaluationDatasetError::AlreadyCancelled)
                    }
                    DatasetGenerationStatus::Failed { .. } => return Ok(vec![]),
                    DatasetGenerationStatus::AwaitingMap | DatasetGenerationStatus::Generating => {}
                }
                Ok(vec![Self::Event::DatasetGenerationFailed(
                    DatasetGenerationFailed {
                        dataset_id: dataset.dataset_id,
                        reason: cmd.reason,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }

            Self::Command::CancelDatasetGeneration(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                match &dataset.status {
                    DatasetGenerationStatus::Completed => {
                        return Err(EvaluationDatasetError::AlreadyCompleted)
                    }
                    DatasetGenerationStatus::Cancelled => return Ok(vec![]),
                    DatasetGenerationStatus::Failed { .. } => {
                        return Err(EvaluationDatasetError::AlreadyFailed)
                    }
                    DatasetGenerationStatus::AwaitingMap | DatasetGenerationStatus::Generating => {}
                }
                Ok(vec![Self::Event::DatasetGenerationCancelled(
                    DatasetGenerationCancelled {
                        dataset_id: dataset.dataset_id,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }

            Self::Command::RenameDataset(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                if dataset.deleted {
                    return Err(EvaluationDatasetError::Deleted);
                }
                let trimmed = cmd.label.trim();
                if trimmed.is_empty() {
                    return Err(EvaluationDatasetError::EmptyLabel);
                }
                Ok(vec![Self::Event::DatasetRenamed(DatasetRenamed {
                    dataset_id: dataset.dataset_id,
                    label: trimmed.to_string(),
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::DeleteDataset(cmd) => {
                let dataset = state.ok_or(EvaluationDatasetError::NotFound)?;
                if dataset.deleted {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::DatasetDeleted(DatasetDeleted {
                    dataset_id: dataset.dataset_id,
                    occurred_at: cmd.occurred_at,
                })])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;

        for event in events {
            match (&mut state, event) {
                (None, Self::Event::DatasetGenerationRequested(e)) => {
                    state = Some(Self::from_requested(e));
                }
                (Some(_), Self::Event::DatasetGenerationRequested(_)) | (None, _) => return None,
                (Some(dataset), event) => dataset.apply(event),
            }
        }

        state
    }
}
