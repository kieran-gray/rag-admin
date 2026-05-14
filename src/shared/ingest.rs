use serde::{Deserialize, Serialize};

use super::chunking::ChunkingConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Success,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct IngestOptions {
    pub force: bool,
    pub dry_run: bool,
    #[serde(default)]
    pub chunking_override: Option<ChunkingConfig>,
}
