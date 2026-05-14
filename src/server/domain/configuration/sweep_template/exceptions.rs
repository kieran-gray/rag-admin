#[derive(Debug, Clone)]
pub enum SweepTemplateError {
    NotFound,
    AlreadyExists,
    AlreadyDeleted,
    ValidationError(String),
    InvalidCommand(String),
}

impl std::fmt::Display for SweepTemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SweepTemplateError::NotFound => write!(f, "sweep template not found"),
            SweepTemplateError::AlreadyExists => write!(f, "sweep template already exists"),
            SweepTemplateError::AlreadyDeleted => write!(f, "sweep template has been deleted"),
            SweepTemplateError::ValidationError(msg) => write!(f, "{msg}"),
            SweepTemplateError::InvalidCommand(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for SweepTemplateError {}
