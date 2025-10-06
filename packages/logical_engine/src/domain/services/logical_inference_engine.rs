use crate::domain::errors::{EngineError, EngineResult};
use crate::domain::models::{AddBulkResult, Binding, InferenceResult, InferenceStatus, ValidateResult};
use nemo::api::{load_string, reason, Engine};
use regex::Regex;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// LogicalInferenceEngine wraps Nemo to provide logical inference capabilities
/// 
/// This service encapsulates Nemo's inference engine and provides a simplified API
/// for loading facts, rules, executing queries, and getting explanations.
pub struct LogicalInferenceEngine {
    /// The Nemo engine instance
    engine: Option<Engine>,
    
    /// Current program text (facts and rules)
    program_text: String,
    
    /// Mapping from predicate names to human-readable descriptions
    predicate_annotations: HashMap<String, String>,
    
    /// Default timeout for operations (in milliseconds)
    default_timeout_ms: u64,
}

impl LogicalInferenceEngine {
    /// Create a new LogicalInferenceEngine
    pub fn new() -> Self {
        Self {
            engine: None,
            program_text: String::new(),
            predicate_annotations: HashMap::new(),
            default_timeout_ms: 5000, // 5 seconds default
        }
    }

    /// Load a single fact into the knowledge base
    /// 
    /// # Arguments
    /// * `fact` - A Datalog fact (e.g., "perro(fido).")
    /// 
    /// # Example
    /// ```
    /// engine.load_fact("perro(fido).").await?;
    /// ```
    pub async fn load_fact(&mut self, fact: &str) -> EngineResult<()> {
        self.validate_datalog_syntax(fact)?;
        
        // Add to program text
        if !self.program_text.is_empty() {
            self.program_text.push('\n');
        }
        self.program_text.push_str(fact);
        
        // Reload the engine
        self.reload_engine().await
    }

    /// Load a single rule into the knowledge base
    /// 
    /// # Arguments
    /// * `rule` - A Datalog rule (e.g., "come(X) :- perro(X), existe(X).")
    /// 
    /// # Example
    /// ```
    /// engine.load_rule("come(X) :- perro(X), existe(X).").await?;
    /// ```
    pub async fn load_rule(&mut self, rule: &str) -> EngineResult<()> {
        self.validate_datalog_syntax(rule)?;
        
        // Add to program text
        if !self.program_text.is_empty() {
            self.program_text.push('\n');
        }
        self.program_text.push_str(rule);
        
        // Reload the engine
        self.reload_engine().await
    }

    /// Load multiple facts and/or rules in bulk
    /// 
    /// # Arguments
    /// * `datalog` - Multiple Datalog statements separated by newlines
    /// * `atomic` - If true, all statements must be valid or none are applied
    /// 
    /// # Returns
    /// AddBulkResult with details about which statements were added
    pub async fn load_bulk(&mut self, datalog: &str, atomic: bool) -> AddBulkResult {
        let lines: Vec<&str> = datalog.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('%'))
            .collect();
        
        let mut errors = Vec::new();
        let mut valid_lines = Vec::new();
        
        // Validate all lines
        for (idx, line) in lines.iter().enumerate() {
            match self.validate_datalog_syntax(line) {
                Ok(_) => valid_lines.push(*line),
                Err(e) => errors.push((idx + 1, e.to_string())),
            }
        }
        
        let total_count = lines.len();
        let added_count = valid_lines.len();
        let rolled_back = atomic && !errors.is_empty();
        
        // If atomic and there are errors, don't add anything
        if rolled_back {
            return AddBulkResult {
                added_count: 0,
                total_count,
                errors,
                rolled_back,
            };
        }
        
        // Add valid lines to program
        for line in valid_lines {
            if !self.program_text.is_empty() {
                self.program_text.push('\n');
            }
            self.program_text.push_str(line);
        }
        
        // Reload engine with new program
        if added_count > 0 {
            if let Err(e) = self.reload_engine().await {
                errors.push((0, format!("Engine reload failed: {}", e)));
                return AddBulkResult {
                    added_count: 0,
                    total_count,
                    errors,
                    rolled_back: true,
                };
            }
        }
        
        AddBulkResult {
            added_count,
            total_count,
            errors,
            rolled_back,
        }
    }

    /// Execute a query against the knowledge base
    /// 
    /// # Arguments
    /// * `query_str` - A Datalog query (e.g., "?- come(X).")
    /// * `timeout_ms` - Maximum time to wait for the query in milliseconds
    /// 
    /// # Returns
    /// InferenceResult with the query result and trace
    pub async fn query(&mut self, query_str: &str, timeout_ms: u64) -> InferenceResult {
        // For now, return a basic implementation
        // In a full implementation, we would execute the query against Nemo
        // and extract bindings and traces
        
        let timeout_duration = Duration::from_millis(timeout_ms);
        let start = Instant::now();
        
        // Basic validation
        if let Err(e) = self.validate_query_syntax(query_str) {
            return InferenceResult {
                proven: false,
                status: InferenceStatus::Inconclusive,
                bindings: Vec::new(),
                trace_json: None,
                explanation: Some(format!("Query validation failed: {}", e)),
            };
        }
        
        // Check if we have an engine
        if self.engine.is_none() {
            return InferenceResult {
                proven: false,
                status: InferenceStatus::Inconclusive,
                bindings: Vec::new(),
                trace_json: None,
                explanation: Some("No knowledge base loaded".to_string()),
            };
        }
        
        // For a minimal implementation, we return inconclusive
        // A full implementation would execute the query on Nemo
        InferenceResult {
            proven: false,
            status: InferenceStatus::Inconclusive,
            bindings: Vec::new(),
            trace_json: None,
            explanation: Some("Query execution not fully implemented yet".to_string()),
        }
    }

    /// Materialize all derivable facts
    /// 
    /// This runs the inference engine to compute all facts that can be derived
    /// from the current rules and facts.
    pub async fn materialize(&mut self, timeout_ms: u64) -> EngineResult<()> {
        let timeout_duration = Duration::from_millis(timeout_ms);
        
        if let Some(ref mut engine) = self.engine {
            match timeout(timeout_duration, reason(engine)).await {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(EngineError::NemoError(e.to_string())),
                Err(_) => Err(EngineError::Timeout(timeout_ms)),
            }
        } else {
            Err(EngineError::OperationNotAllowed("No engine loaded".to_string()))
        }
    }

    /// Get JSON trace from the last query
    /// 
    /// Returns the trace in JSON format if available
    pub fn get_trace_json(&self) -> Option<serde_json::Value> {
        // Placeholder - would return actual trace from Nemo
        None
    }

    /// Reset the knowledge base
    /// 
    /// Clears all facts, rules, and resets the engine
    pub fn reset(&mut self) {
        self.engine = None;
        self.program_text.clear();
        self.predicate_annotations.clear();
    }

    /// List all premises (facts and rules) currently in the knowledge base
    /// 
    /// Returns the current program as a string
    pub fn list_premises(&self) -> String {
        if self.program_text.is_empty() {
            "% No premises loaded".to_string()
        } else {
            self.program_text.clone()
        }
    }

    /// Validate a rule for syntax and basic issues
    /// 
    /// # Arguments
    /// * `rule_str` - The rule to validate
    /// 
    /// # Returns
    /// ValidateResult with any errors or warnings found
    pub fn validate_rule(&self, rule_str: &str) -> ValidateResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Basic syntax validation
        if let Err(e) = self.validate_datalog_syntax(rule_str) {
            errors.push(e.to_string());
            return ValidateResult {
                is_valid: false,
                errors,
                warnings,
            };
        }
        
        // Check if it's a rule (contains :-)
        if rule_str.contains(":-") {
            // Extract variables from head and body
            let parts: Vec<&str> = rule_str.split(":-").collect();
            if parts.len() == 2 {
                let head_vars = self.extract_variables(parts[0]);
                let body_vars = self.extract_variables(parts[1]);
                
                // Check for unbound variables in head
                for var in &head_vars {
                    if !body_vars.contains(var) {
                        errors.push(format!("Variable {} in head is not bound in body", var));
                    }
                }
                
                // Warn if body is empty
                let body_trimmed = parts[1].trim().trim_end_matches('.');
                if body_trimmed.is_empty() {
                    warnings.push("Rule body is empty".to_string());
                }
            }
        }
        
        ValidateResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Add a predicate annotation for human-readable explanations
    /// 
    /// # Arguments
    /// * `predicate` - The predicate name (e.g., "perro")
    /// * `annotation` - Human-readable description (e.g., "is a dog")
    pub fn add_predicate_annotation(&mut self, predicate: String, annotation: String) {
        self.predicate_annotations.insert(predicate, annotation);
    }

    /// Explain an inference using natural language
    /// 
    /// # Arguments
    /// * `trace_json` - The trace JSON from Nemo
    /// * `short` - Whether to return a short summary
    /// 
    /// # Returns
    /// A human-readable explanation of the inference
    pub fn explain_inference(&self, trace_json: &serde_json::Value, short: bool) -> String {
        // Basic implementation - in a full version, we would parse the trace
        // and generate a detailed explanation
        
        if short {
            "Inference explanation: No detailed trace available".to_string()
        } else {
            format!("Detailed inference explanation:\n\nTrace data: {}\n\nNo detailed trace parsing implemented yet.", trace_json)
        }
    }

    // Private helper methods

    /// Validate basic Datalog syntax
    fn validate_datalog_syntax(&self, text: &str) -> EngineResult<()> {
        let trimmed = text.trim();
        
        if trimmed.is_empty() {
            return Err(EngineError::InvalidSyntax("Empty statement".to_string()));
        }
        
        // Basic regex for Datalog (simplified)
        // Fact: predicate(args).
        // Rule: head(args) :- body.
        let fact_regex = Regex::new(r"^[a-z][a-zA-Z0-9_]*\([^)]*\)\.$").unwrap();
        let rule_regex = Regex::new(r"^[a-z][a-zA-Z0-9_]*\([^)]*\)\s*:-\s*.+\.$").unwrap();
        
        if fact_regex.is_match(trimmed) || rule_regex.is_match(trimmed) {
            Ok(())
        } else {
            Err(EngineError::InvalidSyntax(format!("Invalid Datalog syntax: {}", trimmed)))
        }
    }

    /// Validate query syntax
    fn validate_query_syntax(&self, query: &str) -> EngineResult<()> {
        let trimmed = query.trim();
        
        if !trimmed.starts_with("?-") {
            return Err(EngineError::InvalidSyntax("Query must start with ?-".to_string()));
        }
        
        if !trimmed.ends_with('.') {
            return Err(EngineError::InvalidSyntax("Query must end with .".to_string()));
        }
        
        Ok(())
    }

    /// Extract variables from a Datalog expression (variables start with uppercase)
    fn extract_variables(&self, expr: &str) -> Vec<String> {
        let var_regex = Regex::new(r"\b[A-Z][a-zA-Z0-9_]*\b").unwrap();
        var_regex
            .find_iter(expr)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    /// Reload the Nemo engine with the current program
    async fn reload_engine(&mut self) -> EngineResult<()> {
        if self.program_text.is_empty() {
            self.engine = None;
            return Ok(());
        }
        
        match load_string(self.program_text.clone()).await {
            Ok(engine) => {
                self.engine = Some(engine);
                Ok(())
            }
            Err(e) => Err(EngineError::NemoError(e.to_string())),
        }
    }
}

impl Default for LogicalInferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_engine() {
        let engine = LogicalInferenceEngine::new();
        assert_eq!(engine.program_text, "");
        assert!(engine.engine.is_none());
    }

    #[tokio::test]
    async fn test_load_fact() {
        let mut engine = LogicalInferenceEngine::new();
        let result = engine.load_fact("perro(fido).").await;
        assert!(result.is_ok());
        assert!(engine.program_text.contains("perro(fido)"));
    }

    #[tokio::test]
    async fn test_load_rule() {
        let mut engine = LogicalInferenceEngine::new();
        let result = engine.load_rule("come(X) :- perro(X).").await;
        assert!(result.is_ok());
        assert!(engine.program_text.contains("come(X)"));
    }

    #[tokio::test]
    async fn test_invalid_syntax() {
        let engine = LogicalInferenceEngine::new();
        let result = engine.validate_datalog_syntax("invalid syntax here");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_rule_unbound_variable() {
        let engine = LogicalInferenceEngine::new();
        let result = engine.validate_rule("head(X, Y) :- body(X).");
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_reset() {
        let mut engine = LogicalInferenceEngine::new();
        engine.load_fact("test(a).").await.ok();
        engine.reset();
        assert_eq!(engine.program_text, "");
        assert!(engine.engine.is_none());
    }

    #[tokio::test]
    async fn test_list_premises_empty() {
        let engine = LogicalInferenceEngine::new();
        let premises = engine.list_premises();
        assert!(premises.contains("No premises"));
    }

    #[tokio::test]
    async fn test_bulk_add_atomic_success() {
        let mut engine = LogicalInferenceEngine::new();
        let datalog = "perro(fido).\nexiste(fido).";
        let result = engine.load_bulk(datalog, true).await;
        assert_eq!(result.added_count, 2);
        assert_eq!(result.total_count, 2);
        assert!(result.errors.is_empty());
        assert!(!result.rolled_back);
    }

    #[tokio::test]
    async fn test_bulk_add_atomic_failure() {
        let mut engine = LogicalInferenceEngine::new();
        let datalog = "perro(fido).\ninvalid syntax\nexiste(fido).";
        let result = engine.load_bulk(datalog, true).await;
        assert_eq!(result.added_count, 0);
        assert!(!result.errors.is_empty());
        assert!(result.rolled_back);
    }

    #[tokio::test]
    async fn test_bulk_add_non_atomic() {
        let mut engine = LogicalInferenceEngine::new();
        let datalog = "perro(fido).\ninvalid syntax\nexiste(fido).";
        let result = engine.load_bulk(datalog, false).await;
        assert_eq!(result.added_count, 2);
        assert_eq!(result.total_count, 3);
        assert_eq!(result.errors.len(), 1);
        assert!(!result.rolled_back);
    }
}
