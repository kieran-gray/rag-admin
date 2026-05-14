use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::{LogEvent, LogLevel};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InternalLogLevel {
    Info,
    Warn,
    Error,
    Success,
}

impl From<InternalLogLevel> for LogLevel {
    fn from(value: InternalLogLevel) -> Self {
        match value {
            InternalLogLevel::Info => LogLevel::Info,
            InternalLogLevel::Warn => LogLevel::Warn,
            InternalLogLevel::Error => LogLevel::Error,
            InternalLogLevel::Success => LogLevel::Success,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalLogEvent {
    pub level: InternalLogLevel,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl InternalLogEvent {
    pub fn info(msg: impl Into<String>) -> Self {
        Self::new(InternalLogLevel::Info, msg)
    }
    pub fn warn(msg: impl Into<String>) -> Self {
        Self::new(InternalLogLevel::Warn, msg)
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self::new(InternalLogLevel::Error, msg)
    }
    pub fn success(msg: impl Into<String>) -> Self {
        Self::new(InternalLogLevel::Success, msg)
    }

    fn new(level: InternalLogLevel, msg: impl Into<String>) -> Self {
        Self {
            level,
            message: msg.into(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl From<InternalLogEvent> for LogEvent {
    fn from(value: InternalLogEvent) -> Self {
        Self {
            level: value.level.into(),
            message: value.message,
        }
    }
}
