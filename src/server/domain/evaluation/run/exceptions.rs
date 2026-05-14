#[derive(Debug, Clone)]
pub enum EvaluationRunError {
    AlreadyExists,
    NotFound,
    AlreadyCompleted,
    AlreadyFailed,
    NotAllVariantsScored,
    InvalidCommand(String),
}

impl std::fmt::Display for EvaluationRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationRunError::AlreadyExists => write!(f, "evaluation run already exists"),
            EvaluationRunError::NotFound => write!(f, "evaluation run not found"),
            EvaluationRunError::AlreadyCompleted => {
                write!(f, "evaluation run has already completed")
            }
            EvaluationRunError::AlreadyFailed => write!(f, "evaluation run has already failed"),
            EvaluationRunError::NotAllVariantsScored => {
                write!(f, "cannot complete run before all variants are scored")
            }
            EvaluationRunError::InvalidCommand(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EvaluationRunError {}
