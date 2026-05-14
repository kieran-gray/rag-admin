use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{aggregate_type, PublishedEvent};

pub fn classify(
    stream_id: Uuid,
    aggregate_type_str: &str,
    event_type: &str,
    occurred_at: &str,
    event_data: &serde_json::Value,
) -> Option<ActivityDelta> {
    match (aggregate_type_str, event_type) {
        (aggregate_type::INDEXING, "IngestRequested") => {
            Some(ActivityDelta::Start(ActivityStart {
                stream_id,
                aggregate_type: aggregate_type_str.to_string(),
                kind: ActivityKind::Indexing,
                label: format!("Indexing {}", short_id(stream_id)),
                started_at: occurred_at.to_string(),
            }))
        }
        (aggregate_type::INDEXING, "IndexingCompleted") => Some(ActivityDelta::Complete {
            stream_id,
            occurred_at: occurred_at.to_string(),
        }),
        (aggregate_type::INDEXING, "IngestionFailed") => Some(ActivityDelta::Fail {
            stream_id,
            occurred_at: occurred_at.to_string(),
        }),
        (aggregate_type::INDEXING, "IndexingRemoved") => Some(ActivityDelta::Remove { stream_id }),
        (aggregate_type::INDEXING, "ChunkingCompleted" | "EmbeddingCompleted") => {
            let auto_advance = event_data
                .get("auto_advance")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if auto_advance {
                Some(ActivityDelta::Refresh { stream_id })
            } else {
                Some(ActivityDelta::Complete {
                    stream_id,
                    occurred_at: occurred_at.to_string(),
                })
            }
        }
        (
            aggregate_type::INDEXING,
            "ChunkingRequeued" | "EmbeddingRequeued" | "IndexingRequeued",
        ) => Some(ActivityDelta::Start(ActivityStart {
            stream_id,
            aggregate_type: aggregate_type_str.to_string(),
            kind: ActivityKind::Indexing,
            label: format!("Indexing {}", short_id(stream_id)),
            started_at: occurred_at.to_string(),
        })),
        (aggregate_type::INDEXING, "IngestionRetried") => {
            Some(ActivityDelta::Refresh { stream_id })
        }

        (aggregate_type::EVALUATION_DATASET, "DatasetGenerationRequested") => {
            Some(ActivityDelta::Start(ActivityStart {
                stream_id,
                aggregate_type: aggregate_type_str.to_string(),
                kind: ActivityKind::EvaluationDataset,
                label: format!("Dataset {}", short_id(stream_id)),
                started_at: occurred_at.to_string(),
            }))
        }
        (aggregate_type::EVALUATION_DATASET, "DatasetGenerationCompleted") => {
            Some(ActivityDelta::Complete {
                stream_id,
                occurred_at: occurred_at.to_string(),
            })
        }
        (aggregate_type::EVALUATION_DATASET, "DatasetGenerationFailed") => {
            Some(ActivityDelta::Fail {
                stream_id,
                occurred_at: occurred_at.to_string(),
            })
        }

        (aggregate_type::EVALUATION_RUN, "RunRequested") => {
            Some(ActivityDelta::Start(ActivityStart {
                stream_id,
                aggregate_type: aggregate_type_str.to_string(),
                kind: ActivityKind::EvaluationRun,
                label: format!("Run {}", short_id(stream_id)),
                started_at: occurred_at.to_string(),
            }))
        }
        (aggregate_type::EVALUATION_RUN, "RunCompleted") => Some(ActivityDelta::Complete {
            stream_id,
            occurred_at: occurred_at.to_string(),
        }),
        (aggregate_type::EVALUATION_RUN, "RunFailed") => Some(ActivityDelta::Fail {
            stream_id,
            occurred_at: occurred_at.to_string(),
        }),

        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityJobDto {
    pub stream_id: Uuid,
    pub aggregate_type: String,
    pub kind: ActivityKind,
    pub label: String,
    pub status: ActivityStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub stream_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityKind {
    Indexing,
    EvaluationDataset,
    EvaluationRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityStatus {
    Running,
    Completed,
    Failed,
}

impl ActivityStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, ActivityStatus::Completed | ActivityStatus::Failed)
    }
}

#[derive(Debug, Clone)]
pub enum ActivityDelta {
    Start(ActivityStart),
    Complete {
        stream_id: Uuid,
        occurred_at: String,
    },
    Fail {
        stream_id: Uuid,
        occurred_at: String,
    },
    Remove {
        stream_id: Uuid,
    },
    Refresh {
        stream_id: Uuid,
    },
}

#[derive(Debug, Clone)]
pub struct ActivityStart {
    pub stream_id: Uuid,
    pub aggregate_type: String,
    pub kind: ActivityKind,
    pub label: String,
    pub started_at: String,
}

pub fn classify_event(event: &PublishedEvent) -> Option<ActivityDelta> {
    classify(
        event.stream_id,
        &event.aggregate_type,
        &event.event_type,
        &event.occurred_at,
        &event.event_data,
    )
}

fn short_id(id: Uuid) -> String {
    id.to_string()[..8].to_string()
}
