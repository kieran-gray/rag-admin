use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast::error::RecvError, Mutex};
use tracing::warn;
use uuid::Uuid;

use crate::server::event_sourcing::event_bus::EventBus;
use crate::shared::{classify, ActivityDelta, ActivityJobDto, ActivityStart, ActivityStatus};

const TERMINAL_RETENTION: Duration = Duration::from_secs(15 * 60);

struct Row {
    dto: ActivityJobDto,

    terminal_at: Option<Instant>,
}

struct State {
    rows: HashMap<Uuid, Row>,

    pending_streams: HashMap<Uuid, String>,
}

pub struct ActivityRegistry {
    state: Mutex<State>,
}

impl ActivityRegistry {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                rows: HashMap::new(),
                pending_streams: HashMap::new(),
            }),
        }
    }

    pub async fn snapshot(&self) -> Vec<ActivityJobDto> {
        let now = Instant::now();
        let mut state = self.state.lock().await;
        state.rows.retain(|_, row| match row.terminal_at {
            Some(at) => now.duration_since(at) < TERMINAL_RETENTION,
            None => true,
        });

        let mut out: Vec<ActivityJobDto> = state.rows.values().map(|r| r.dto.clone()).collect();
        out.sort_by(|a, b| match (a.status, b.status) {
            (ActivityStatus::Running, ActivityStatus::Running) => a.started_at.cmp(&b.started_at),
            (ActivityStatus::Running, _) => std::cmp::Ordering::Less,
            (_, ActivityStatus::Running) => std::cmp::Ordering::Greater,
            _ => b
                .finished_at
                .cmp(&a.finished_at)
                .then(b.started_at.cmp(&a.started_at)),
        });
        out
    }

    pub async fn attach_stream(&self, stream_id: Uuid, stream_url: String) {
        let mut state = self.state.lock().await;
        match state.rows.get_mut(&stream_id) {
            Some(row) => row.dto.stream_url = Some(stream_url),
            None => {
                state.pending_streams.insert(stream_id, stream_url);
            }
        }
    }

    async fn apply(&self, delta: ActivityDelta) {
        let mut state = self.state.lock().await;
        match delta {
            ActivityDelta::Start(ActivityStart {
                stream_id,
                aggregate_type,
                kind,
                label,
                started_at,
            }) => {
                let pending_stream = state.pending_streams.remove(&stream_id);
                let entry = state.rows.entry(stream_id).or_insert(Row {
                    dto: ActivityJobDto {
                        stream_id,
                        aggregate_type: aggregate_type.clone(),
                        kind,
                        label: label.clone(),
                        status: ActivityStatus::Running,
                        started_at: started_at.clone(),
                        finished_at: None,
                        stream_url: None,
                    },
                    terminal_at: None,
                });

                entry.dto.status = ActivityStatus::Running;
                entry.dto.started_at = started_at;
                entry.dto.finished_at = None;
                entry.terminal_at = None;
                entry.dto.aggregate_type = aggregate_type;
                entry.dto.kind = kind;
                entry.dto.label = label;
                if let Some(url) = pending_stream {
                    entry.dto.stream_url = Some(url);
                }
            }
            ActivityDelta::Complete {
                stream_id,
                occurred_at,
            } => {
                if let Some(row) = state.rows.get_mut(&stream_id) {
                    row.dto.status = ActivityStatus::Completed;
                    row.dto.finished_at = Some(occurred_at);
                    row.terminal_at = Some(Instant::now());
                }
            }
            ActivityDelta::Fail {
                stream_id,
                occurred_at,
            } => {
                if let Some(row) = state.rows.get_mut(&stream_id) {
                    row.dto.status = ActivityStatus::Failed;
                    row.dto.finished_at = Some(occurred_at);
                    row.terminal_at = Some(Instant::now());
                }
            }
            ActivityDelta::Remove { stream_id } => {
                state.rows.remove(&stream_id);
                state.pending_streams.remove(&stream_id);
            }
            ActivityDelta::Refresh { .. } => {}
        }
    }
}

impl Default for ActivityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn spawn_activity_projection(registry: Arc<ActivityRegistry>, event_bus: Arc<EventBus>) {
    tokio::spawn(async move {
        let mut subscription = event_bus.subscribe();
        loop {
            match subscription.recv().await {
                Ok(event) => {
                    if let Some(delta) = classify(
                        event.stream_id,
                        &event.aggregate_type,
                        &event.event_type,
                        event.occurred_at.as_str(),
                        &event.event_data,
                    ) {
                        registry.apply(delta).await;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    warn!(dropped = n, "activity projection subscriber lagged");
                }
                Err(RecvError::Closed) => return,
            }
        }
    });
}
