use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

use tokio::sync::{broadcast, Mutex};

use crate::server::application::InternalLogEvent;

const BROADCAST_CAPACITY: usize = 256;

pub struct JobInner {
    pub buffered: Vec<InternalLogEvent>,
    pub finished: bool,
}

pub struct Job {
    pub inner: Mutex<JobInner>,
    pub sender: broadcast::Sender<JobMessage>,
}

#[derive(Debug, Clone)]
pub enum JobMessage {
    Event(InternalLogEvent),
    Done,
}

impl Job {
    fn new() -> Arc<Self> {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Arc::new(Self {
            inner: Mutex::new(JobInner {
                buffered: Vec::new(),
                finished: false,
            }),
            sender,
        })
    }

    pub async fn emit(&self, event: InternalLogEvent) {
        let meta = if event.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&event.metadata).unwrap_or_default())
        };
        match event.level {
            crate::server::application::InternalLogLevel::Info
            | crate::server::application::InternalLogLevel::Success => match &meta {
                Some(m) => info!(metadata = %m, "{}", event.message),
                None => info!("{}", event.message),
            },
            crate::server::application::InternalLogLevel::Warn => match &meta {
                Some(m) => warn!(metadata = %m, "{}", event.message),
                None => warn!("{}", event.message),
            },
            crate::server::application::InternalLogLevel::Error => match &meta {
                Some(m) => error!(metadata = %m, "{}", event.message),
                None => error!("{}", event.message),
            },
        }

        let mut inner = self.inner.lock().await;
        inner.buffered.push(event.clone());
        let _ = self.sender.send(JobMessage::Event(event));
    }

    pub async fn info(&self, log: &str) {
        self.emit(InternalLogEvent::info(log)).await;
    }

    pub async fn warn(&self, log: &str) {
        self.emit(InternalLogEvent::warn(log)).await;
    }

    pub async fn error(&self, log: &str) {
        self.emit(InternalLogEvent::error(log)).await;
    }

    pub async fn finish(&self) {
        let mut inner = self.inner.lock().await;
        inner.finished = true;
        let _ = self.sender.send(JobMessage::Done);
    }
}

pub struct JobRegistry {
    jobs: Mutex<HashMap<String, Arc<Job>>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create(&self) -> (String, Arc<Job>) {
        let id = generate_id();
        let job = Job::new();
        self.jobs.lock().await.insert(id.clone(), job.clone());
        (id, job)
    }

    pub async fn get(&self, id: &str) -> Option<Arc<Job>> {
        self.jobs.lock().await.get(id).cloned()
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}
