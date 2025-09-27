use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum TreeEngineError {
    #[error("Node not found: {0}")]
    NotFound(String),

    #[error("Probability out of range: {0} (must be between 0.0 and 1.0)")]
    ProbabilityOutOfRange(f64),

    #[error("Invalid input for field '{0}': {1}")]
    InvalidInput(String, String),

    #[error("Operation not allowed: {0}")]
    OperationNotAllowed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Functionality not implemented: {0}")]
    NotImplemented(String),
}

impl From<surrealdb::Error> for TreeEngineError {
    fn from(err: surrealdb::Error) -> Self {
        TreeEngineError::DatabaseError(err.to_string())
    }
}

pub type TreeResult<T> = Result<T, TreeEngineError>;
