use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use axum::extract::Path;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Extension;
use futures_util::stream::Stream;

use crate::server::application::{JobMessage, JobRegistry};
use crate::shared::LogEvent;

pub async fn job_logs_handler(
    Path(job_id): Path<String>,
    Extension(jobs): Extension<Arc<JobRegistry>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream_for_job(jobs, job_id);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

fn stream_for_job(
    jobs: Arc<JobRegistry>,
    job_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    stream! {
        let job = jobs.get(&job_id).await;
        let Some(job) = job else {
            let payload = serde_json::to_string(&LogEvent {
                level: crate::shared::LogLevel::Error,
                message: format!("unknown job id: {job_id}"),
            })
            .unwrap_or_default();
            yield Ok(Event::default().data(payload));
            yield Ok(Event::default().data("__done__"));
            return;
        };

        let mut rx = job.sender.subscribe();
        let (buffered_events, finished) = {
            let inner = job.inner.lock().await;
            (inner.buffered.clone(), inner.finished)
        };

        for evt in buffered_events {
            let payload: LogEvent = evt.into();
            let json = serde_json::to_string(&payload).unwrap_or_default();
            yield Ok(Event::default().data(json));
        }

        if finished {
            yield Ok(Event::default().data("__done__"));
            return;
        }

        loop {
            match rx.recv().await {
                Ok(JobMessage::Event(evt)) => {
                    let payload: LogEvent = evt.into();
                    let json = serde_json::to_string(&payload).unwrap_or_default();
                    yield Ok(Event::default().data(json));
                }
                Ok(JobMessage::Done) => {
                    yield Ok(Event::default().data("__done__"));
                    break;
                }
                Err(_) => break,
            }
        }
    }
}
