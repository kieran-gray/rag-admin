use std::sync::Arc;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::Notify;
use tokio::time::timeout;
use tracing::{debug, error, info};

use super::aggregate::Aggregate;
use super::process_manager::ProcessManager;

const POLL_HEARTBEAT: Duration = Duration::from_secs(2);

pub struct EffectDispatcher<A, R>
where
    A: Aggregate,
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    process_manager: Arc<ProcessManager<A, R>>,
    wakeup: Arc<Notify>,
}

impl<A, R> EffectDispatcher<A, R>
where
    A: Aggregate + 'static,
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    pub fn new(process_manager: Arc<ProcessManager<A, R>>, wakeup: Arc<Notify>) -> Self {
        Self {
            process_manager,
            wakeup,
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!(aggregate = A::aggregate_type(), "effect dispatcher started");
        loop {
            loop {
                match self.process_manager.claim_and_dispatch_one().await {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => {
                        error!(
                            aggregate = A::aggregate_type(),
                            error = %e,
                            "effect dispatcher drain failed"
                        );
                        break;
                    }
                }
            }
            match timeout(POLL_HEARTBEAT, self.wakeup.notified()).await {
                Ok(()) => debug!(
                    aggregate = A::aggregate_type(),
                    "effect dispatcher woken by notify"
                ),
                Err(_) => debug!(
                    aggregate = A::aggregate_type(),
                    "effect dispatcher heartbeat tick"
                ),
            }
        }
    }
}
