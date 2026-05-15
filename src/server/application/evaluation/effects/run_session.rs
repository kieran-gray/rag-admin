use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::Clock;
use crate::server::application::{ActivityRegistry, AppError, Job, JobRegistry};
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{EvaluationRunCommand, FailRun};
use crate::server::event_sourcing::command_processor::CommandProcessor;

pub struct RunSession {
    job_registry: Arc<JobRegistry>,
    activity_registry: Arc<ActivityRegistry>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    clock: Arc<dyn Clock>,
}

impl RunSession {
    pub fn new(
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            job_registry,
            activity_registry,
            command_processor,
            clock,
        }
    }

    pub async fn run<F, Fut>(
        &self,
        run_id: Uuid,
        failure_label: &str,
        body: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(Arc<Job>) -> Fut,
        Fut: std::future::Future<Output = Result<(), AppError>>,
    {
        let (job_id, job) = self.job_registry.create().await;
        let stream_url = format!("/api/job/logs/{job_id}");
        self.activity_registry
            .attach_stream(run_id, stream_url)
            .await;

        let result = body(Arc::clone(&job)).await;
        if let Err(e) = &result {
            job.error(&format!("{failure_label}: {e}")).await;
            let _ = self
                .command_processor
                .handle(
                    run_id,
                    EvaluationRunCommand::FailRun(FailRun {
                        run_id,
                        reason: e.to_string(),
                        occurred_at: self.clock.now(),
                    }),
                )
                .await;
        }
        job.finish().await;
        result
    }
}
