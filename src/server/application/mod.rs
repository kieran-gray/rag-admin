pub mod activity;
pub mod chunking;
pub mod configuration;
pub mod embedding;
pub mod evaluation;
pub mod exceptions;
pub mod indexing;
pub mod job;
pub mod llm;
pub mod markdown;
pub mod ports;
pub mod query;
pub mod source_document;

#[cfg(test)]
pub mod test_support;

pub use activity::{spawn_activity_projection, ActivityRegistry};
pub use exceptions::AppError;
pub use job::{InternalLogEvent, InternalLogLevel, Job, JobMessage, JobRegistry};
