use std::future::Future;
use std::sync::Arc;
use tracing::error;

use uuid::Uuid;

use crate::server::application::ports::Clock;
use crate::server::application::{ActivityRegistry, AppError, Job, JobRegistry};
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::Aggregate;

pub struct JobSession<A>
where
    A: Aggregate,
{
    job_registry: Arc<JobRegistry>,
    activity_registry: Arc<ActivityRegistry>,
    command_processor: Arc<CommandProcessor<A>>,
    clock: Arc<dyn Clock>,
}

#[derive(Debug, Clone)]
pub enum JobIdStrategy {
    Fresh,
    PerStream,
}

impl<A> JobSession<A>
where
    A: Aggregate,
    AppError: From<A::Error>,
{
    pub fn new(
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        command_processor: Arc<CommandProcessor<A>>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            job_registry,
            activity_registry,
            command_processor,
            clock,
        }
    }

    pub fn clock(&self) -> Arc<dyn Clock> {
        Arc::clone(&self.clock)
    }

    pub fn command_processor(&self) -> Arc<CommandProcessor<A>> {
        Arc::clone(&self.command_processor)
    }

    pub async fn run<F, Fut, FailFn>(
        &self,
        stream_id: Uuid,
        strategy: JobIdStrategy,
        failure_label: &'static str,
        fail_command: FailFn,
        body: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(Arc<Job>) -> Fut,
        Fut: Future<Output = Result<(), AppError>>,
        FailFn: FnOnce(String) -> A::Command,
    {
        let job = self.open_job(stream_id, strategy).await;
        let result = body(Arc::clone(&job)).await;
        if let Err(e) = &result {
            job.error(&format!("{failure_label}: {e}")).await;
            if let Err(fail_command_error) = self
                .command_processor
                .handle(stream_id, fail_command(e.to_string()))
                .await
            {
                error!("Failed to handle failure command: {fail_command_error}");
            }
        }
        job.finish().await;
        result
    }

    async fn open_job(&self, stream_id: Uuid, strategy: JobIdStrategy) -> Arc<Job> {
        match strategy {
            JobIdStrategy::Fresh => {
                let (job_id, job) = self.job_registry.create().await;
                let stream_url = format!("/api/job/logs/{job_id}");
                self.activity_registry
                    .attach_stream(stream_id, stream_url)
                    .await;
                job
            }
            JobIdStrategy::PerStream => {
                let job_id = stream_id.to_string();
                let (job, created) = self.job_registry.get_or_create(job_id.clone()).await;
                if created {
                    let stream_url = format!("/api/job/logs/{job_id}");
                    self.activity_registry
                        .attach_stream(stream_id, stream_url)
                        .await;
                }
                job
            }
        }
    }
}
