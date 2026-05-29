use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::contracts::aggregate_types as aggregate_type;
use crate::shared::contracts::events::PublishedEvent;

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
    DocumentMap,
    ConnectorImport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl ActivityStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            ActivityStatus::Completed | ActivityStatus::Failed | ActivityStatus::Cancelled
        )
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
    Cancel {
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
        (aggregate_type::INDEXING, "IndexingCompleted")
        | (aggregate_type::EVALUATION_RUN, "RunCompleted")
        | (aggregate_type::EVALUATION_DATASET, "DatasetGenerationCompleted")
        | (aggregate_type::DOCUMENT_MAP, "InsightsSynthesized") => Some(ActivityDelta::Complete {
            stream_id,
            occurred_at: occurred_at.to_string(),
        }),
        (aggregate_type::INDEXING, "IngestionFailed")
        | (aggregate_type::EVALUATION_RUN, "RunFailed")
        | (aggregate_type::EVALUATION_DATASET, "DatasetGenerationFailed") => {
            Some(ActivityDelta::Fail {
                stream_id,
                occurred_at: occurred_at.to_string(),
            })
        }
        (aggregate_type::INDEXING, "IndexingRemoved") => Some(ActivityDelta::Remove { stream_id }),
        (aggregate_type::INDEXING, "ChunkingCompleted" | "EmbeddingCompleted") => {
            let auto_advance = event_payload(event_data)
                .get("auto_advance")
                .and_then(serde_json::Value::as_bool)
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
        (aggregate_type::EVALUATION_DATASET, "DatasetGenerationCancelled") => {
            Some(ActivityDelta::Cancel {
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

        (aggregate_type::DOCUMENT_MAP, "MapBuildRequested") => {
            Some(ActivityDelta::Start(ActivityStart {
                stream_id,
                aggregate_type: aggregate_type_str.to_string(),
                kind: ActivityKind::DocumentMap,
                label: format!("Map {}", short_id(stream_id)),
                started_at: occurred_at.to_string(),
            }))
        }

        (aggregate_type::CONNECTOR_IMPORT, "ConnectorImportStarted") => {
            let count = event_payload(event_data)
                .get("source_ref_keys")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            Some(ActivityDelta::Start(ActivityStart {
                stream_id,
                aggregate_type: aggregate_type_str.to_string(),
                kind: ActivityKind::ConnectorImport,
                label: format!("Importing {count} item(s)"),
                started_at: occurred_at.to_string(),
            }))
        }
        (aggregate_type::CONNECTOR_IMPORT, "ConnectorImportCompleted") => {
            Some(ActivityDelta::Complete {
                stream_id,
                occurred_at: occurred_at.to_string(),
            })
        }
        (aggregate_type::CONNECTOR_IMPORT, "ConnectorImportFailed") => Some(ActivityDelta::Fail {
            stream_id,
            occurred_at: occurred_at.to_string(),
        }),

        _ => None,
    }
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
    id.to_string().chars().take(8).collect()
}

fn event_payload(event_data: &serde_json::Value) -> &serde_json::Value {
    event_data.get("data").unwrap_or(event_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stream_id() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }

    #[test]
    fn chunking_completed_with_auto_advance_false_marks_complete() {
        let payload = json!({
            "type": "ChunkingCompleted",
            "data": {
                "chunk_set_id": Uuid::nil(),
                "chunk_count": 3,
                "auto_advance": false,
                "occurred_at": "2024-01-01T00:00:00Z",
            }
        });
        let delta = classify(
            stream_id(),
            aggregate_type::INDEXING,
            "ChunkingCompleted",
            "2024-01-01T00:00:01Z",
            &payload,
        );
        assert!(matches!(delta, Some(ActivityDelta::Complete { .. })));
    }

    #[test]
    fn embedding_completed_with_auto_advance_true_refreshes() {
        let payload = json!({
            "type": "EmbeddingCompleted",
            "data": {
                "embedding_set_id": Uuid::nil(),
                "auto_advance": true,
                "occurred_at": "2024-01-01T00:00:00Z",
            }
        });
        let delta = classify(
            stream_id(),
            aggregate_type::INDEXING,
            "EmbeddingCompleted",
            "2024-01-01T00:00:01Z",
            &payload,
        );
        assert!(matches!(delta, Some(ActivityDelta::Refresh { .. })));
    }

    #[test]
    fn connector_import_started_begins_activity_with_item_count() {
        let payload = json!({
            "type": "ConnectorImportStarted",
            "data": {
                "import_id": Uuid::nil(),
                "connector_id": Uuid::nil(),
                "source_ref_keys": ["url:a", "url:b"],
                "index_after_import": true,
                "occurred_at": "2024-01-01T00:00:00Z",
            }
        });
        let delta = classify(
            stream_id(),
            aggregate_type::CONNECTOR_IMPORT,
            "ConnectorImportStarted",
            "2024-01-01T00:00:01Z",
            &payload,
        );
        let Some(ActivityDelta::Start(start)) = delta else {
            panic!("expected start delta");
        };
        assert_eq!(start.kind, ActivityKind::ConnectorImport);
        assert_eq!(start.label, "Importing 2 item(s)");
    }

    #[test]
    fn connector_import_completed_marks_complete() {
        let payload = json!({
            "type": "ConnectorImportCompleted",
            "data": { "import_id": Uuid::nil(), "imported": 2, "failed": 0, "occurred_at": "x" }
        });
        let delta = classify(
            stream_id(),
            aggregate_type::CONNECTOR_IMPORT,
            "ConnectorImportCompleted",
            "2024-01-01T00:00:01Z",
            &payload,
        );
        assert!(matches!(delta, Some(ActivityDelta::Complete { .. })));
    }
}
