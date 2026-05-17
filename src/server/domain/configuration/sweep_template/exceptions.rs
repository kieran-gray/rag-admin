use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum SweepTemplateError {
    NotFound,
    AlreadyExists,
    AlreadyDeleted,
    ValidationError(String),
    InvalidCommand(String),
}

impl fmt::Display for SweepTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SweepTemplateError::NotFound => write!(f, "sweep template not found"),
            SweepTemplateError::AlreadyExists => write!(f, "sweep template already exists"),
            SweepTemplateError::AlreadyDeleted => write!(f, "sweep template has been deleted"),
            SweepTemplateError::ValidationError(msg) | SweepTemplateError::InvalidCommand(msg) => {
                write!(f, "{msg}")
            }
        }
    }
}

impl Error for SweepTemplateError {}
