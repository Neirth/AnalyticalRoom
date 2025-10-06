use serde::{Deserialize, Serialize};

/// Result of a logical inference query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    /// Whether the query was proven true
    pub proven: bool,
    
    /// Query status: TRUE, FALSE, INCONCLUSIVE, CANNOT_DEMONSTRATE
    pub status: InferenceStatus,
    
    /// Bindings for variables in the query (if any)
    pub bindings: Vec<Binding>,
    
    /// JSON trace from Nemo (if available)
    pub trace_json: Option<serde_json::Value>,
    
    /// Human-readable explanation
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InferenceStatus {
    /// Query proven true
    True,
    /// Query proven false
    False,
    /// Cannot determine (timeout, insufficient data)
    Inconclusive,
    /// Cannot demonstrate (loop detected, recursion)
    CannotDemonstrate,
}

/// Variable binding from query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub variable: String,
    pub value: String,
}

/// Result of rule validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResult {
    /// Whether the rule is valid
    pub is_valid: bool,
    
    /// List of errors found
    pub errors: Vec<String>,
    
    /// List of warnings
    pub warnings: Vec<String>,
}

/// Result of bulk add operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddBulkResult {
    /// Number of rules/facts successfully added
    pub added_count: usize,
    
    /// Total number of rules/facts attempted
    pub total_count: usize,
    
    /// Errors per line (line_number, error_message)
    pub errors: Vec<(usize, String)>,
    
    /// Whether the operation was atomic and failed
    pub rolled_back: bool,
}
