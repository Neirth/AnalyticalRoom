use thiserror::Error;

/// Error types for the logical inference engine
#[derive(Error, Debug, Clone)]
pub enum EngineError {
    /// Operation timed out
    #[error("Operation timed out after {0}ms")]
    Timeout(u64),

    /// Invalid Datalog syntax
    #[error("Invalid Datalog syntax: {0}")]
    InvalidSyntax(String),

    /// Empty or invalid rule
    #[error("Invalid rule: {0}")]
    InvalidRule(String),

    /// Unbound variables in rule
    #[error("Unbound variables in rule: {0}")]
    UnboundVariables(String),

    /// Infinite loop detected
    #[error("Infinite loop or recursion detected: {0}")]
    InfiniteLoop(String),

    /// Nemo engine error
    #[error("Nemo engine error: {0}")]
    NemoError(String),

    /// Operation not allowed
    #[error("Operation not allowed: {0}")]
    OperationNotAllowed(String),

    /// Other internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type EngineResult<T> = Result<T, EngineError>;
