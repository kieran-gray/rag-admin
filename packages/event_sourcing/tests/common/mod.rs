#![allow(dead_code, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::aggregate::Aggregate;
use event_sourcing::aggregate_repository::{AggregateSnapshot, SnapshotStore};
use event_sourcing::checkpoint::{CheckpointRepository, ProjectionCheckpoint};
use event_sourcing::envelope::{EventEnvelope, EventMetadata};
use event_sourcing::error::{EsError, ProjectionError};
use event_sourcing::event_store::{AppendedEvent, EventStore};
use event_sourcing::job_queue::{ClaimedJob, IdempotencyKey, JobQueue, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};
use event_sourcing::process_manager::{EffectError, EffectExecutor};
use event_sourcing::projector::Projector;

// ---- Test aggregate ----

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Counter {
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum CounterEvent {
    Inc,
}

#[derive(Debug)]
pub struct CounterError(pub String);

impl std::fmt::Display for CounterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CounterError {}

pub struct CounterCommand {
    pub events: Vec<CounterEvent>,
    pub fail_with: Option<CounterError>,
}

impl Aggregate for Counter {
    type Event = CounterEvent;
    type Command = CounterCommand;
    type Error = CounterError;

    fn aggregate_type() -> &'static str {
        "counter_test"
    }

    fn apply(&mut self, _event: &Self::Event) {
        self.count += 1;
    }

    fn handle_command(
        _state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if let Some(e) = command.fail_with {
            return Err(e);
        }
        Ok(command.events)
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        if events.is_empty() {
            None
        } else {
            Some(Self {
                count: events.len() as i64,
            })
        }
    }
}

// ---- In-memory event store ----

struct StoreInner {
    events: Vec<EventEnvelope<CounterEvent>>,
    log_counter: i64,
}

pub struct InMemEventStore {
    inner: Mutex<StoreInner>,
}

impl InMemEventStore {
    pub fn empty() -> Self {
        Self {
            inner: Mutex::new(StoreInner {
                events: vec![],
                log_counter: 0,
            }),
        }
    }

    pub fn with(events: Vec<EventEnvelope<CounterEvent>>) -> Self {
        let log_counter = events
            .iter()
            .map(|e| e.metadata.log_position)
            .max()
            .unwrap_or(0);
        Self {
            inner: Mutex::new(StoreInner {
                events,
                log_counter,
            }),
        }
    }
}

#[async_trait]
impl EventStore<CounterEvent> for InMemEventStore {
    async fn load(&self, stream_id: Uuid) -> Result<Vec<EventEnvelope<CounterEvent>>, EsError> {
        let guard = self.inner.lock().expect("lock");
        Ok(guard
            .events
            .iter()
            .filter(|e| e.metadata.stream_id == stream_id)
            .cloned()
            .collect())
    }

    async fn load_after(
        &self,
        stream_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<EventEnvelope<CounterEvent>>, EsError> {
        let guard = self.inner.lock().expect("lock");
        Ok(guard
            .events
            .iter()
            .filter(|e| e.metadata.stream_id == stream_id && e.metadata.sequence > after_sequence)
            .cloned()
            .collect())
    }

    async fn append(
        &self,
        stream_id: Uuid,
        expected_version: usize,
        events: &[CounterEvent],
    ) -> Result<Vec<AppendedEvent<CounterEvent>>, EsError> {
        let mut guard = self.inner.lock().expect("lock");
        let current = guard
            .events
            .iter()
            .filter(|e| e.metadata.stream_id == stream_id)
            .map(|e| e.metadata.sequence)
            .max()
            .unwrap_or(0) as usize;

        if current != expected_version {
            return Err(EsError::ConcurrencyConflict {
                aggregate: "counter_test",
                stream_id,
                expected: expected_version as i64,
                actual: current as i64,
            });
        }

        let mut result = vec![];
        for (i, event) in events.iter().enumerate() {
            let seq = (expected_version + i + 1) as i64;
            guard.log_counter += 1;
            let log_pos = guard.log_counter;
            let env = EventEnvelope {
                event: event.clone(),
                metadata: EventMetadata {
                    stream_id,
                    aggregate_type: "counter_test".into(),
                    sequence: seq,
                    log_position: log_pos,
                    event_type: "Inc".into(),
                    occurred_at: "2024-01-01T00:00:00Z".into(),
                },
            };
            guard.events.push(env.clone());
            result.push(env);
        }
        Ok(result)
    }

    async fn load_global_after(
        &self,
        after_log_position: i64,
        limit: i64,
    ) -> Result<Vec<EventEnvelope<CounterEvent>>, EsError> {
        let guard = self.inner.lock().expect("lock");
        Ok(guard
            .events
            .iter()
            .filter(|e| e.metadata.log_position > after_log_position)
            .take(limit as usize)
            .cloned()
            .collect())
    }
}

// ---- In-memory snapshot store ----

pub struct InMemSnapshotStore {
    snap: Mutex<Option<AggregateSnapshot<Counter>>>,
}

impl InMemSnapshotStore {
    pub fn empty() -> Self {
        Self {
            snap: Mutex::new(None),
        }
    }

    pub fn with(snap: AggregateSnapshot<Counter>) -> Self {
        Self {
            snap: Mutex::new(Some(snap)),
        }
    }

    pub fn get(&self) -> Option<AggregateSnapshot<Counter>> {
        self.snap.lock().expect("lock").clone()
    }
}

#[async_trait]
impl SnapshotStore<Counter> for InMemSnapshotStore {
    async fn load(&self, stream_id: Uuid) -> Result<Option<AggregateSnapshot<Counter>>, EsError> {
        let snap = self.snap.lock().expect("lock").clone();
        Ok(snap.filter(|s| s.stream_id == stream_id))
    }

    async fn save(&self, snapshot: &AggregateSnapshot<Counter>) -> Result<(), EsError> {
        *self.snap.lock().expect("lock") = Some(snapshot.clone());
        Ok(())
    }
}

// ---- Event store that always returns a concurrency conflict on append ----

pub struct ConflictingEventStore;

#[async_trait]
impl EventStore<CounterEvent> for ConflictingEventStore {
    async fn load(&self, _: Uuid) -> Result<Vec<EventEnvelope<CounterEvent>>, EsError> {
        Ok(vec![])
    }

    async fn load_after(
        &self,
        _: Uuid,
        _: i64,
    ) -> Result<Vec<EventEnvelope<CounterEvent>>, EsError> {
        Ok(vec![])
    }

    async fn append(
        &self,
        stream_id: Uuid,
        expected: usize,
        _: &[CounterEvent],
    ) -> Result<Vec<AppendedEvent<CounterEvent>>, EsError> {
        Err(EsError::ConcurrencyConflict {
            aggregate: "counter_test",
            stream_id,
            expected: expected as i64,
            actual: expected as i64 + 1,
        })
    }

    async fn load_global_after(
        &self,
        _: i64,
        _: i64,
    ) -> Result<Vec<EventEnvelope<CounterEvent>>, EsError> {
        Ok(vec![])
    }
}

// ---- Helpers ----

pub fn make_envelope(stream_id: Uuid, sequence: i64) -> EventEnvelope<CounterEvent> {
    EventEnvelope {
        event: CounterEvent::Inc,
        metadata: EventMetadata {
            stream_id,
            aggregate_type: "counter_test".into(),
            sequence,
            log_position: sequence,
            event_type: "Inc".into(),
            occurred_at: "2024-01-01T00:00:00Z".into(),
        },
    }
}

// ---- Test effect payload + policy on CounterEvent ----

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TestEffect {
    pub label: String,
    pub log_position: i64,
}

fn counter_policy(
    _ev: &CounterEvent,
    ctx: &PolicyContext<'_, Counter, CounterEvent>,
) -> Vec<NewJob<TestEffect>> {
    vec![NewJob {
        partition_key: ctx.envelope.metadata.stream_id,
        job_type: "test_effect",
        idempotency_key: IdempotencyKey::new(
            ctx.envelope.metadata.stream_id,
            ctx.envelope.metadata.log_position,
            "test_effect",
        ),
        payload: TestEffect {
            label: format!("count-{}", ctx.state.count),
            log_position: ctx.envelope.metadata.log_position,
        },
    }]
}

impl HasPolicies<Counter, TestEffect> for CounterEvent {
    fn policies() -> &'static [PolicyFn<Self, Counter, TestEffect>] {
        &[counter_policy]
    }
}

// ---- In-memory job queue ----

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Claimed,
    Acked,
    Dead,
}

#[derive(Clone)]
pub struct StoredJob<R> {
    pub id: Uuid,
    pub queue: String,
    pub partition_key: Option<Uuid>,
    pub job_type: String,
    pub idempotency_key: IdempotencyKey,
    pub attempts: i32,
    pub max_attempts: i32,
    pub payload: R,
    pub state: JobState,
    pub available_at: SystemTime,
    pub last_worker: Option<String>,
    pub lease_until: Option<SystemTime>,
    pub last_error: Option<String>,
}

pub struct InMemJobQueue<R> {
    jobs: Mutex<Vec<StoredJob<R>>>,
    max_attempts: i32,
}

impl<R: Clone> InMemJobQueue<R> {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
            max_attempts: 3,
        }
    }

    pub fn with_max_attempts(max_attempts: i32) -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
            max_attempts,
        }
    }

    pub fn snapshot(&self) -> Vec<StoredJob<R>> {
        self.jobs.lock().expect("lock").clone()
    }

    pub fn count_by_state(&self, state: JobState) -> usize {
        self.snapshot()
            .into_iter()
            .filter(|j| j.state == state)
            .count()
    }
}

#[async_trait]
impl<R> JobQueue<R> for InMemJobQueue<R>
where
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    async fn enqueue(&self, queue: &str, jobs: &[NewJob<R>]) -> Result<(), EsError> {
        let mut guard = self.jobs.lock().expect("lock");
        for job in jobs {
            if guard
                .iter()
                .any(|j| j.queue == queue && j.idempotency_key == job.idempotency_key)
            {
                continue;
            }
            guard.push(StoredJob {
                id: Uuid::now_v7(),
                queue: queue.to_string(),
                partition_key: Some(job.partition_key),
                job_type: job.job_type.to_string(),
                idempotency_key: job.idempotency_key.clone(),
                attempts: 0,
                max_attempts: self.max_attempts,
                payload: job.payload.clone(),
                state: JobState::Pending,
                available_at: SystemTime::now(),
                last_worker: None,
                lease_until: None,
                last_error: None,
            });
        }
        Ok(())
    }

    async fn claim(
        &self,
        queue: &str,
        worker_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedJob<R>>, EsError> {
        let now = SystemTime::now();
        let mut guard = self.jobs.lock().expect("lock");
        for j in guard.iter_mut() {
            if j.queue != queue {
                continue;
            }
            if j.state != JobState::Pending {
                continue;
            }
            if j.available_at > now {
                continue;
            }
            j.state = JobState::Claimed;
            j.attempts += 1;
            j.last_worker = Some(worker_id.to_string());
            j.lease_until = Some(now + lease);
            return Ok(Some(ClaimedJob {
                id: j.id,
                partition_key: j.partition_key,
                job_type: j.job_type.clone(),
                idempotency_key: j.idempotency_key.clone(),
                attempts: j.attempts,
                max_attempts: j.max_attempts,
                payload: j.payload.clone(),
            }));
        }
        Ok(None)
    }

    async fn heartbeat(
        &self,
        job_id: Uuid,
        worker_id: &str,
        lease: Duration,
    ) -> Result<bool, EsError> {
        let now = SystemTime::now();
        let mut guard = self.jobs.lock().expect("lock");
        let Some(j) = guard.iter_mut().find(|j| j.id == job_id) else {
            return Ok(false);
        };
        if j.state != JobState::Claimed {
            return Ok(false);
        }
        if j.last_worker.as_deref() != Some(worker_id) {
            return Ok(false);
        }
        j.lease_until = Some(now + lease);
        Ok(true)
    }

    async fn ack(&self, job_id: Uuid, worker_id: &str) -> Result<(), EsError> {
        let mut guard = self.jobs.lock().expect("lock");
        if let Some(j) = guard.iter_mut().find(|j| j.id == job_id) {
            if j.last_worker.as_deref() == Some(worker_id) {
                j.state = JobState::Acked;
            }
        }
        Ok(())
    }

    async fn nack(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
        backoff: Duration,
    ) -> Result<(), EsError> {
        let mut guard = self.jobs.lock().expect("lock");
        if let Some(j) = guard.iter_mut().find(|j| j.id == job_id) {
            if j.last_worker.as_deref() == Some(worker_id) {
                j.state = JobState::Pending;
                j.available_at = SystemTime::now() + backoff;
                j.last_error = Some(error.to_string());
            }
        }
        Ok(())
    }

    async fn dead_letter(&self, job_id: Uuid, error: &str) -> Result<(), EsError> {
        let mut guard = self.jobs.lock().expect("lock");
        if let Some(j) = guard.iter_mut().find(|j| j.id == job_id) {
            j.state = JobState::Dead;
            j.last_error = Some(error.to_string());
        }
        Ok(())
    }

    async fn cancel_partition(&self, queue: &str, partition_key: Uuid) -> Result<u64, EsError> {
        let mut guard = self.jobs.lock().expect("lock");
        let mut count: u64 = 0;
        for j in guard.iter_mut() {
            if j.queue == queue
                && j.partition_key == Some(partition_key)
                && j.state == JobState::Pending
            {
                j.state = JobState::Dead;
                j.last_error = Some("cancelled".into());
                count += 1;
            }
        }
        Ok(count)
    }
}

// ---- In-memory checkpoint repository ----

pub struct InMemCheckpointRepository {
    inner: Mutex<HashMap<String, ProjectionCheckpoint>>,
    fail_load: Mutex<bool>,
    fail_upsert: Mutex<bool>,
}

impl InMemCheckpointRepository {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            fail_load: Mutex::new(false),
            fail_upsert: Mutex::new(false),
        }
    }

    pub fn put(&self, cp: ProjectionCheckpoint) {
        self.inner
            .lock()
            .expect("lock")
            .insert(cp.projector_name.clone(), cp);
    }

    pub fn snapshot(&self) -> HashMap<String, ProjectionCheckpoint> {
        self.inner.lock().expect("lock").clone()
    }
}

#[async_trait]
impl CheckpointRepository for InMemCheckpointRepository {
    async fn load(&self, projector_name: &str) -> Result<Option<ProjectionCheckpoint>, EsError> {
        if *self.fail_load.lock().expect("lock") {
            return Err(EsError::Storage("forced load failure".into()));
        }
        Ok(self
            .inner
            .lock()
            .expect("lock")
            .get(projector_name)
            .cloned())
    }

    async fn upsert(&self, checkpoint: &ProjectionCheckpoint) -> Result<(), EsError> {
        if *self.fail_upsert.lock().expect("lock") {
            return Err(EsError::Storage("forced upsert failure".into()));
        }
        self.inner
            .lock()
            .expect("lock")
            .insert(checkpoint.projector_name.clone(), checkpoint.clone());
        Ok(())
    }
}

// ---- Recording projector with configurable failures ----

pub struct RecordingProjector {
    name: String,
    recorded: Mutex<Vec<EventEnvelope<CounterEvent>>>,
    fail_remaining: Mutex<usize>,
}

impl RecordingProjector {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            recorded: Mutex::new(Vec::new()),
            fail_remaining: Mutex::new(0),
        }
    }

    pub fn fail_next(&self, times: usize) {
        *self.fail_remaining.lock().expect("lock") = times;
    }

    pub fn recorded(&self) -> Vec<EventEnvelope<CounterEvent>> {
        self.recorded.lock().expect("lock").clone()
    }
}

#[async_trait]
impl Projector<CounterEvent> for RecordingProjector {
    fn name(&self) -> &str {
        &self.name
    }

    async fn project(&self, events: &[EventEnvelope<CounterEvent>]) -> Result<(), ProjectionError> {
        let should_fail = {
            let mut fail = self.fail_remaining.lock().expect("lock");
            if *fail > 0 {
                *fail -= 1;
                true
            } else {
                false
            }
        };
        if should_fail {
            return Err(ProjectionError::InvalidState(
                "forced projector failure".into(),
            ));
        }
        self.recorded
            .lock()
            .expect("lock")
            .extend(events.iter().cloned());
        Ok(())
    }
}

// ---- Recording effect executor with configurable failures ----

pub struct RecordingExecutor {
    calls: Mutex<Vec<TestEffect>>,
    fail_remaining: Mutex<usize>,
}

impl RecordingExecutor {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_remaining: Mutex::new(0),
        }
    }

    pub fn fail_next(&self, times: usize) {
        *self.fail_remaining.lock().expect("lock") = times;
    }

    pub fn calls(&self) -> Vec<TestEffect> {
        self.calls.lock().expect("lock").clone()
    }
}

#[async_trait]
impl EffectExecutor<TestEffect> for RecordingExecutor {
    async fn execute(&self, effect: &TestEffect) -> Result<(), EffectError> {
        let should_fail = {
            let mut fail = self.fail_remaining.lock().expect("lock");
            if *fail > 0 {
                *fail -= 1;
                true
            } else {
                false
            }
        };
        if should_fail {
            return Err("forced executor failure".into());
        }
        self.calls.lock().expect("lock").push(effect.clone());
        Ok(())
    }
}
