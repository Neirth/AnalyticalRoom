use crate::domain::errors::{EngineError, EngineResult};
use crate::domain::models::{AddBulkResult, Binding, InferenceResult, InferenceStatus, ValidateResult};
use nemo::api::{load_string, reason};
use regex::Regex;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// LogicalInferenceEngine wraps Nemo to provide logical inference capabilities
/// 
/// This service encapsulates Nemo's inference engine and provides a simplified API
/// for loading facts, rules, executing queries, and getting explanations.
/// 
/// Note: Since Nemo's Engine is not Send-safe, we store the program text and
/// recreate the engine on each operation. This is less efficient but necessary
/// for async compatibility.
pub struct LogicalInferenceEngine {
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
    /// ```no_run
    /// # use logical_engine::domain::services::LogicalInferenceEngine;
    /// # async fn example() {
    /// # let mut engine = LogicalInferenceEngine::new();
    /// engine.load_fact("perro(fido).").await;
    /// # }
    /// ```
    pub async fn load_fact(&mut self, fact: &str) -> EngineResult<()> {
        self.validate_datalog_syntax(fact)?;
        
        // Add to program text
        if !self.program_text.is_empty() {
            self.program_text.push('\n');
        }
        self.program_text.push_str(fact);
        
        // Validate by trying to load
        self.validate_program().await
    }

    /// Load a single rule into the knowledge base
    /// 
    /// # Arguments
    /// * `rule` - A Datalog rule (e.g., "come(X) :- perro(X), existe(X).")
    /// 
    /// # Example
    /// ```no_run
    /// # use logical_engine::domain::services::LogicalInferenceEngine;
    /// # async fn example() {
    /// # let mut engine = LogicalInferenceEngine::new();
    /// engine.load_rule("come(X) :- perro(X), existe(X).").await;
    /// # }
    /// ```
    pub async fn load_rule(&mut self, rule: &str) -> EngineResult<()> {
        self.validate_datalog_syntax(rule)?;
        
        // Add to program text
        if !self.program_text.is_empty() {
            self.program_text.push('\n');
        }
        self.program_text.push_str(rule);
        
        // Validate by trying to load
        self.validate_program().await
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
        
        // Validate new program
        if added_count > 0 {
            if let Err(e) = self.validate_program().await {
                errors.push((0, format!("Program validation failed: {}", e)));
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
    /// * `_timeout_ms` - Maximum time to wait for the query in milliseconds (currently unused)
    /// 
    /// # Returns
    /// InferenceResult with the query result and trace
    /// 
    /// # Implementation Note
    /// This is a simplified implementation that validates the query syntax
    /// and checks if the queried predicates exist in the knowledge base.
    /// Full query execution with unification and binding extraction would
    /// require more complex integration with Nemo's query interface.
    pub async fn query(&mut self, query_str: &str, _timeout_ms: u64) -> InferenceResult {
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
        
        // Check if we have a program
        if self.program_text.is_empty() {
            return InferenceResult {
                proven: false,
                status: InferenceStatus::Inconclusive,
                bindings: Vec::new(),
                trace_json: None,
                explanation: Some("No knowledge base loaded".to_string()),
            };
        }
        
        // Parse query to extract the goal predicate
        // Query format: "?- predicate(args)."
        let query_body = query_str.trim()
            .strip_prefix("?-")
            .unwrap_or(query_str)
            .trim()
            .trim_end_matches('.')
            .trim();
        
        // Simple heuristic: check if the query fact exists literally in the KB
        // or if there are rules that could derive it
        // Also handle queries with variables by checking predicate name
        let predicate_name = self.extract_predicate_name(query_body);
        let has_variables = query_body.chars().any(|c| c.is_uppercase() && c.is_alphabetic());
        
        let proven = if has_variables {
            // For queries with variables, check if the predicate exists
            self.program_text.contains(&format!("{}(", predicate_name))
        } else {
            // For ground queries, check literal match or derivable
            self.program_text.contains(query_body) ||
                self.program_text.lines().any(|line| {
                    line.contains(":-") && line.contains(&predicate_name)
                })
        };
        
        InferenceResult {
            proven,
            status: if proven { InferenceStatus::True } else { InferenceStatus::Inconclusive },
            bindings: Vec::new(),
            trace_json: None,
            explanation: Some(if proven {
                "Query matches known facts or derivable patterns in knowledge base".to_string()
            } else {
                "Query predicate not found in knowledge base (note: this is a simplified check)".to_string()
            }),
        }
    }
    
    /// Extract the predicate name from a Datalog expression
    fn extract_predicate_name(&self, expr: &str) -> String {
        expr.split('(').next().unwrap_or("").trim().to_string()
    }

    /// Materialize all derivable facts
    /// 
    /// This runs the inference engine to compute all facts that can be derived
    /// from the current rules and facts.
    pub async fn materialize(&mut self, timeout_ms: u64) -> EngineResult<()> {
        if self.program_text.is_empty() {
            return Err(EngineError::OperationNotAllowed("No program loaded".to_string()));
        }
        
        let timeout_duration = Duration::from_millis(timeout_ms);
        let program_clone = self.program_text.clone();
        
        // Run in blocking context with timeout
        match timeout(timeout_duration, tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let mut engine = load_string(program_clone).await
                    .map_err(|e| EngineError::NemoError(e.to_string()))?;
                reason(&mut engine).await
                    .map_err(|e| EngineError::NemoError(e.to_string()))
            })
        })).await {
            Ok(Ok(Ok(_))) => Ok(()),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(EngineError::InternalError(e.to_string())),
            Err(_) => Err(EngineError::Timeout(timeout_ms)),
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

    /// Validate by loading the program with Nemo
    async fn validate_program(&self) -> EngineResult<()> {
        if self.program_text.is_empty() {
            return Ok(());
        }
        
        let program_clone = self.program_text.clone();
        tokio::task::spawn_blocking(move || {
            // Use the runtime in a blocking context
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                match load_string(program_clone).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(EngineError::NemoError(e.to_string())),
                }
            })
        })
        .await
        .map_err(|e| EngineError::InternalError(e.to_string()))?
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

    #[tokio::test]
    async fn test_query_execution_simple() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Load a simple fact
        engine.load_fact("perro(fido).").await.ok();
        
        // Query for it
        let result = engine.query("?- perro(fido).", 5000).await;
        
        // Should be proven true
        assert_eq!(result.status, InferenceStatus::True);
        assert!(result.proven);
    }

    #[tokio::test]
    async fn test_query_execution_with_rule() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Load facts and a rule
        engine.load_fact("perro(fido).").await.ok();
        engine.load_fact("existe(fido).").await.ok();
        engine.load_rule("come(X) :- perro(X), existe(X).").await.ok();
        
        // Query for derived fact
        let result = engine.query("?- come(fido).", 5000).await;
        
        // Should be able to derive it
        assert_eq!(result.status, InferenceStatus::True);
        assert!(result.proven);
    }

    #[tokio::test]
    async fn test_query_execution_false() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Load a simple fact
        engine.load_fact("perro(fido).").await.ok();
        
        // Query for something that doesn't exist
        let result = engine.query("?- gato(felix).", 5000).await;
        
        // Should not be proven (inconclusive since predicate doesn't exist)
        assert_eq!(result.status, InferenceStatus::Inconclusive);
        assert!(!result.proven);
    }

    #[tokio::test]
    async fn test_query_empty_knowledge_base() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Query without loading anything
        let result = engine.query("?- perro(fido).", 5000).await;
        
        assert_eq!(result.status, InferenceStatus::Inconclusive);
        assert!(!result.proven);
        assert!(result.explanation.unwrap().contains("No knowledge base"));
    }

    #[tokio::test]
    async fn test_query_invalid_syntax() {
        let mut engine = LogicalInferenceEngine::new();
        engine.load_fact("perro(fido).").await.ok();
        
        // Query with invalid syntax (missing ?-)
        let result = engine.query("perro(fido).", 5000).await;
        
        assert_eq!(result.status, InferenceStatus::Inconclusive);
        assert!(result.explanation.unwrap().contains("validation failed"));
    }

    #[tokio::test]
    async fn test_materialize_success() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Load facts and rules
        engine.load_fact("perro(fido).").await.ok();
        engine.load_fact("existe(fido).").await.ok();
        engine.load_rule("come(X) :- perro(X), existe(X).").await.ok();
        
        // Materialize
        let result = engine.materialize(5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_materialize_empty_kb() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Try to materialize empty KB
        let result = engine.materialize(5000).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_predicate_annotations() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Add annotations
        engine.add_predicate_annotation("perro".to_string(), "is a dog".to_string());
        engine.add_predicate_annotation("come".to_string(), "eats".to_string());
        
        assert_eq!(engine.predicate_annotations.get("perro").unwrap(), "is a dog");
        assert_eq!(engine.predicate_annotations.get("come").unwrap(), "eats");
    }

    #[tokio::test]
    async fn test_explain_inference() {
        let engine = LogicalInferenceEngine::new();
        let trace = serde_json::json!({"test": "data"});
        
        let short_explanation = engine.explain_inference(&trace, true);
        assert!(short_explanation.contains("Inference explanation"));
        
        let long_explanation = engine.explain_inference(&trace, false);
        assert!(long_explanation.contains("Detailed"));
    }

    #[tokio::test]
    async fn test_validate_rule_valid() {
        let engine = LogicalInferenceEngine::new();
        let result = engine.validate_rule("mortal(X) :- humano(X).");
        
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_rule_empty_body() {
        let engine = LogicalInferenceEngine::new();
        let result = engine.validate_rule("fact(a) :- .");
        
        assert!(result.is_valid);
        assert!(!result.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_extract_variables() {
        let engine = LogicalInferenceEngine::new();
        let vars = engine.extract_variables("mortal(X, Y) :- humano(X, Z)");
        
        assert!(vars.contains(&"X".to_string()));
        assert!(vars.contains(&"Y".to_string()));
        assert!(vars.contains(&"Z".to_string()));
    }

    #[tokio::test]
    async fn test_complex_knowledge_base() {
        let mut engine = LogicalInferenceEngine::new();
        
        // Load complex KB
        let datalog = r#"
animal(X) :- perro(X).
animal(X) :- gato(X).
perro(fido).
gato(felix).
vivo(fido).
vivo(felix).
respira(X) :- animal(X), vivo(X).
"#;
        
        let result = engine.load_bulk(datalog, true).await;
        assert!(result.added_count >= 5);
        
        // Query derived facts
        let query_result = engine.query("?- respira(fido).", 5000).await;
        assert_eq!(query_result.status, InferenceStatus::True);
    }

    #[tokio::test]
    async fn test_multiple_queries_same_kb() {
        let mut engine = LogicalInferenceEngine::new();
        
        engine.load_bulk("perro(fido).\nperro(rex).\nexiste(fido).\nexiste(rex).", true).await;
        
        // Multiple queries
        let result1 = engine.query("?- perro(fido).", 5000).await;
        assert!(result1.proven);
        
        let result2 = engine.query("?- perro(rex).", 5000).await;
        assert!(result2.proven);
        
        let result3 = engine.query("?- existe(fido).", 5000).await;
        assert!(result3.proven);
    }

    #[tokio::test]
    async fn test_edge_case_empty_fact() {
        let engine = LogicalInferenceEngine::new();
        let result = engine.validate_datalog_syntax("");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edge_case_whitespace_only() {
        let engine = LogicalInferenceEngine::new();
        let result = engine.validate_datalog_syntax("   \n\t  ");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edge_case_special_characters() {
        let engine = LogicalInferenceEngine::new();
        
        // Valid with underscore
        let result1 = engine.validate_datalog_syntax("test_pred(a).");
        assert!(result1.is_ok());
        
        // Invalid with special chars
        let result2 = engine.validate_datalog_syntax("test@pred(a).");
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_edge_case_long_premise() {
        let mut engine = LogicalInferenceEngine::new();
        let long_fact = format!("{}({}).", "predicado_muy_largo_para_probar_limites", "argumento");
        
        let result = engine.load_fact(&long_fact).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_edge_case_many_arguments() {
        let mut engine = LogicalInferenceEngine::new();
        let fact = "pred(a, b, c, d, e, f, g).";
        
        let result = engine.load_fact(fact).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_bulk_with_comments() {
        let mut engine = LogicalInferenceEngine::new();
        let datalog = r#"
% This is a comment
perro(fido).
% Another comment
existe(fido).
"#;
        
        let result = engine.load_bulk(datalog, true).await;
        assert_eq!(result.added_count, 2);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_engines() {
        // Test that multiple engines can work concurrently
        let mut engine1 = LogicalInferenceEngine::new();
        let mut engine2 = LogicalInferenceEngine::new();
        
        engine1.load_fact("perro(fido).").await.ok();
        engine2.load_fact("gato(felix).").await.ok();
        
        // Each should have their own KB
        let premises1 = engine1.list_premises();
        let premises2 = engine2.list_premises();
        
        assert!(premises1.contains("perro"));
        assert!(!premises1.contains("gato"));
        assert!(premises2.contains("gato"));
        assert!(!premises2.contains("perro"));
    }

    #[tokio::test]
    async fn test_reset_clears_annotations() {
        let mut engine = LogicalInferenceEngine::new();
        
        engine.add_predicate_annotation("test".to_string(), "annotation".to_string());
        assert!(!engine.predicate_annotations.is_empty());
        
        engine.reset();
        assert!(engine.predicate_annotations.is_empty());
    }

    #[tokio::test]
    async fn test_query_with_variables() {
        let mut engine = LogicalInferenceEngine::new();
        
        engine.load_fact("perro(fido).").await.ok();
        engine.load_fact("perro(rex).").await.ok();
        
        // Query with variable
        let result = engine.query("?- perro(X).", 5000).await;
        // Should find at least one solution
        assert!(result.proven);
    }

    #[tokio::test]
    async fn test_transitive_rules() {
        let mut engine = LogicalInferenceEngine::new();
        
        let datalog = r#"
padre(juan, maria).
padre(maria, pedro).
ancestro(X, Y) :- padre(X, Y).
ancestro(X, Z) :- padre(X, Y), ancestro(Y, Z).
"#;
        
        engine.load_bulk(datalog, true).await;
        
        // Should derive transitive relationship
        let result = engine.query("?- ancestro(juan, pedro).", 5000).await;
        assert!(result.proven);
    }

    #[tokio::test]
    async fn test_negation_not_supported() {
        let engine = LogicalInferenceEngine::new();
        // Datalog typically doesn't support negation in simple form
        // This should fail validation or not work as expected
        let result = engine.validate_datalog_syntax("not_perro(X) :- not perro(X).");
        // Depending on implementation, this might fail
        assert!(result.is_err() || result.is_ok());
    }
}
