#[derive(Debug, Clone)]
pub enum EvaluationDatasetError {
    AlreadyExists,
    NotFound,
    GenerationNotInProgress,
    AlreadyCompleted,
    AlreadyFailed,
    NoQuestionsAccepted,
    Deleted,
    EmptyLabel,
    InvalidCommand(String),
}

impl std::fmt::Display for EvaluationDatasetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationDatasetError::AlreadyExists => {
                write!(f, "evaluation dataset already exists")
            }
            EvaluationDatasetError::NotFound => write!(f, "evaluation dataset not found"),
            EvaluationDatasetError::GenerationNotInProgress => {
                write!(f, "dataset generation is not in progress")
            }
            EvaluationDatasetError::AlreadyCompleted => {
                write!(f, "dataset generation has already completed")
            }
            EvaluationDatasetError::AlreadyFailed => {
                write!(f, "dataset generation has already failed")
            }
            EvaluationDatasetError::NoQuestionsAccepted => {
                write!(f, "cannot complete dataset with no accepted questions")
            }
            EvaluationDatasetError::Deleted => {
                write!(f, "evaluation dataset has been deleted")
            }
            EvaluationDatasetError::EmptyLabel => {
                write!(f, "dataset label must not be empty")
            }
            EvaluationDatasetError::InvalidCommand(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EvaluationDatasetError {}
