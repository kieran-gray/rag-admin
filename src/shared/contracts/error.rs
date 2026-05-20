use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorKind {
    NotFound,
    Validation,
    Upstream,
    Internal,
}

impl ApiErrorKind {
    pub fn http_status(self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Validation => 400,
            Self::Upstream => 502,
            Self::Internal => 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::NotFound, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Validation, message)
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Upstream, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Internal, message)
    }

    pub fn http_status(&self) -> u16 {
        self.kind.http_status()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ApiError {}
