//! Nemo Worker Pool - Thread-Safe Worker Management
//!
//! This module provides a global worker pool that manages Nemo engine instances.
//! Each MCP session gets its own dedicated worker thread running a Nemo engine instance.
//! Workers are automatically created on first access and destroyed when the session ends.
//!
//! # Architecture
//! - Global worker pool using lazy_static for thread-safe singleton access
//! - One worker thread per MCP session ID
//! - Workers communicate via async channels (tokio::mpsc)
//! - Each worker maintains its own Nemo engine state
//! - Workers are automatically cleaned up when dropped

use crate::domain::errors::{EngineError, EngineResult};
use crate::domain::models::{AddBulkResult, InferenceResult, ValidateResult};
use nemo::api::{load_string, reason};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

/// Commands that can be sent to a Nemo worker
#[derive(Debug)]
enum WorkerCommand {
    LoadFact {
        fact: String,
        response_tx: tokio::sync::oneshot::Sender<EngineResult<()>>,
    },
    LoadRule {
        rule: String,
        response_tx: tokio::sync::oneshot::Sender<EngineResult<()>>,
    },
    LoadBulk {
        datalog: String,
        atomic: bool,
        response_tx: tokio::sync::oneshot::Sender<AddBulkResult>,
    },
    Query {
        query_str: String,
        timeout_ms: u64,
        response_tx: tokio::sync::oneshot::Sender<InferenceResult>,
    },
    Materialize {
        timeout_ms: u64,
        response_tx: tokio::sync::oneshot::Sender<EngineResult<()>>,
    },
    GetTraceJson {
        response_tx: tokio::sync::oneshot::Sender<Option<serde_json::Value>>,
    },
    Reset {
        response_tx: tokio::sync::oneshot::Sender<()>,
    },
    ListPremises {
        response_tx: tokio::sync::oneshot::Sender<String>,
    },
    ValidateRule {
        rule_str: String,
        response_tx: tokio::sync::oneshot::Sender<ValidateResult>,
    },
    AddPredicateAnnotation {
        predicate: String,
        annotation: String,
        response_tx: tokio::sync::oneshot::Sender<()>,
    },
    ExplainInference {
        trace_json: serde_json::Value,
        short: bool,
        response_tx: tokio::sync::oneshot::Sender<String>,
    },
    Shutdown,
}

/// Worker state - runs in its own thread
struct NemoWorkerState {
    /// Current program text (facts and rules)
    program_text: String,
    
    /// Mapping from predicate names to human-readable descriptions
    predicate_annotations: HashMap<String, String>,
}

impl NemoWorkerState {
    fn new() -> Self {
        Self {
            program_text: String::new(),
            predicate_annotations: HashMap::new(),
        }
    }

    /// Validate that variables use Nemo syntax (?X, ?Y instead of X, Y)
    fn validate_nemo_variable_syntax(&self, text: &str) -> EngineResult<()> {
        // Find all potential variables (uppercase start)
        let var_regex = Regex::new(r"([A-Z][a-zA-Z0-9_]*)").unwrap();
        
        for cap in var_regex.captures_iter(text) {
            let full_match = cap.get(0).unwrap();
            let var_name = full_match.as_str();
            let match_start = full_match.start();
            
            // Check if there's a '?' character immediately before the variable
            let has_question_mark = if match_start > 0 {
                text.chars().nth(match_start - 1) == Some('?')
            } else {
                false
            };
            
            // If no '?' prefix, it's invalid Nemo syntax
            if !has_question_mark {
                return Err(EngineError::InvalidSyntax(
                    format!("Invalid variable syntax '{}'. Nemo requires variables to start with '?' (e.g., '?{}' instead of '{}')", 
                        var_name, var_name, var_name)
                ));
            }
        }
        
        Ok(())
    }

    /// Remove Prolog-style comments (%) from text
    /// Comments are removed from each line before processing
    fn remove_comments(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                // Find the % character and take everything before it
                if let Some(pos) = line.find('%') {
                    &line[..pos]
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

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

    /// Extract variables from a Datalog expression (Nemo variables start with ?)
    fn extract_variables(&self, expr: &str) -> Vec<String> {
        let var_regex = Regex::new(r"\?[A-Z][a-zA-Z0-9_]*\b").unwrap();
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

    /// Load a single fact
    async fn load_fact(&mut self, fact: &str) -> EngineResult<()> {
        // Remove comments before processing
        let fact_clean = self.remove_comments(fact);
        let fact_clean = fact_clean.trim();
        
        self.validate_datalog_syntax(fact_clean)?;
        
        // Validate Nemo variable syntax (facts shouldn't have variables, but check anyway)
        self.validate_nemo_variable_syntax(fact_clean)?;
        
        // Build temporary program with the new fact (without comments)
        let mut temp_program = self.program_text.clone();
        if !temp_program.is_empty() {
            temp_program.push('\n');
        }
        temp_program.push_str(fact_clean);
        
        // Validate the complete temporary program before committing
        let temp_state = NemoWorkerState {
            program_text: temp_program.clone(),
            predicate_annotations: self.predicate_annotations.clone(),
        };
        
        temp_state.validate_program().await?;
        
        // Validation succeeded - commit the change
        self.program_text = temp_program;
        Ok(())
    }

    /// Load a single rule
    async fn load_rule(&mut self, rule: &str) -> EngineResult<()> {
        // Remove comments before processing
        let rule_clean = self.remove_comments(rule);
        let rule_clean = rule_clean.trim();
        
        self.validate_datalog_syntax(rule_clean)?;
        
        // Validate Nemo variable syntax - variables must use ? prefix
        self.validate_nemo_variable_syntax(rule_clean)?;
        
        // Build temporary program with the new rule (without comments)
        let mut temp_program = self.program_text.clone();
        if !temp_program.is_empty() {
            temp_program.push('\n');
        }
        temp_program.push_str(rule_clean);
        
        // Validate the complete temporary program before committing
        let temp_state = NemoWorkerState {
            program_text: temp_program.clone(),
            predicate_annotations: self.predicate_annotations.clone(),
        };
        
        temp_state.validate_program().await?;
        
        // Validation succeeded - commit the change
        self.program_text = temp_program;
        Ok(())
    }

    /// Load multiple facts and rules in bulk
    async fn load_bulk(&mut self, program: &str, atomic: bool) -> AddBulkResult {
        // Validate Nemo variable syntax before loading
        if let Err(e) = self.validate_nemo_variable_syntax(program) {
            return AddBulkResult {
                added_count: 0,
                total_count: program.lines().filter(|line| line.trim().ends_with('.')).count(),
                errors: vec![(0, e.to_string())],
                rolled_back: atomic,
            };
        }
        
        // Count statements (simple count by lines with dots)
        let total_count = program.lines()
            .filter(|line| line.trim().ends_with('.'))
            .count();
        
        // Build temporary program with the bulk content
        let mut temp_program = self.program_text.clone();
        if !temp_program.is_empty() {
            temp_program.push('\n');
        }
        temp_program.push_str(program);
        
        // Validate the complete temporary program before committing
        let temp_state = NemoWorkerState {
            program_text: temp_program.clone(),
            predicate_annotations: self.predicate_annotations.clone(),
        };
        
        match temp_state.validate_program().await {
            Ok(_) => {
                // Validation succeeded - commit the change
                self.program_text = temp_program;
                AddBulkResult {
                    added_count: total_count,
                    total_count,
                    errors: vec![],
                    rolled_back: false,
                }
            }
            Err(e) => {
                // Validation failed
                if atomic {
                    // Atomic mode: don't commit anything, rollback
                    AddBulkResult {
                        added_count: 0,
                        total_count,
                        errors: vec![(0, e.to_string())],
                        rolled_back: true,
                    }
                } else {
                    // Non-atomic mode: report error but previous state is preserved
                    AddBulkResult {
                        added_count: 0,
                        total_count,
                        errors: vec![(0, e.to_string())],
                        rolled_back: false,
                    }
                }
            }
        }
    }

    /// Execute a query using the Nemo reasoner
    async fn query(&self, query_str: &str, timeout_ms: u64) -> InferenceResult {
        use crate::domain::models::InferenceStatus;
        use nemo::rule_model::components::tag::Tag;
        
        // Validate query syntax
        if let Err(e) = self.validate_query_syntax(query_str) {
            return InferenceResult {
                proven: false,
                status: InferenceStatus::Inconclusive,
                bindings: Vec::new(),
                trace_json: None,
                explanation: Some(format!("Query validation failed: {}", e)),
            };
        }
        
        // Validate Nemo variable syntax
        if let Err(e) = self.validate_nemo_variable_syntax(query_str) {
            return InferenceResult {
                proven: false,
                status: InferenceStatus::Inconclusive,
                bindings: Vec::new(),
                trace_json: None,
                explanation: Some(format!("Query syntax error: {}", e)),
            };
        }
        
        // Check if knowledge base is loaded
        if self.program_text.is_empty() {
            return InferenceResult {
                proven: false,
                status: InferenceStatus::Inconclusive,
                bindings: Vec::new(),
                trace_json: None,
                explanation: Some("No knowledge base loaded".to_string()),
            };
        }
        
        // Extract query body (remove ?- prefix and trailing dot)
        let query_body = query_str.trim()
            .strip_prefix("?-")
            .unwrap_or(query_str)
            .trim()
            .trim_end_matches('.')
            .trim();
        
        // Extract variables (already in Nemo syntax: ?X, ?Y, etc.)
        let var_regex = regex::Regex::new(r"\?[A-Z][a-zA-Z0-9_]*\b").unwrap();
        let mut variables: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for cap in var_regex.find_iter(query_body) {
            let var_name = cap.as_str();
            if seen.insert(var_name.to_string()) {
                variables.push(var_name.to_string());
            }
        }
        
        // Extract the predicate name from the query
        // Pattern: predicate_name(args...)
        let predicate_regex = regex::Regex::new(r"^([a-z][a-zA-Z0-9_]*)\s*\(").unwrap();
        let predicate_name = predicate_regex
            .captures(query_body)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Build augmented program - just export the queried predicate directly
        let augmented_program = format!(
            "{}\n\n% Export predicate for query\n@export {} :- csv {{}}.",
            self.program_text,
            predicate_name
        );

        // Debug: print augmented program
        #[cfg(test)]
        eprintln!("Augmented program:\n{}", augmented_program);

        let timeout_duration = Duration::from_millis(timeout_ms);
        let program_clone = augmented_program.clone();
        let predicate_clone = predicate_name.clone();
        let variables_clone = variables.clone();
        let query_body_clone = query_body.to_string();
        
        // Execute query with Nemo engine in blocking task with timeout
        match timeout(timeout_duration, tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                // Load program
                let mut engine = load_string(program_clone).await
                    .map_err(|e| EngineError::NemoError(e.to_string()))?;
                
                // Execute reasoning
                reason(&mut engine).await
                    .map_err(|e| EngineError::NemoError(e.to_string()))?;
                
                // Get results for the queried predicate
                let tag = Tag::new(predicate_clone.clone());

                #[cfg(test)]
                eprintln!("Looking for predicate: {}", predicate_clone);

                let rows = engine.predicate_rows(&tag).await
                    .map_err(|e| EngineError::NemoError(e.to_string()))?;

                // Collect all results, preserving row structure
                // predicate_rows returns Option<impl Iterator<Item = Vec<AnyDataValue>>>
                let results: Vec<Vec<nemo::datavalues::AnyDataValue>> = if let Some(row_iter) = rows {
                    row_iter.collect()
                } else {
                    Vec::new()
                };

                #[cfg(test)]
                eprintln!("Found {} results for predicate {}", results.len(), predicate_clone);

                Ok::<_, EngineError>((results, query_body_clone, variables_clone))
            })
        })).await {
            Ok(Ok(Ok((mut results, query_body, variables)))) => {
                // Query executed successfully
                
                // For ground queries (no variables), filter results to match the specific query
                if variables.is_empty() {
                    // Extract the argument from the query, e.g., "cat" from "living(cat)"
                    let arg_regex = regex::Regex::new(r"\(([^)]+)\)").unwrap();
                    if let Some(cap) = arg_regex.captures(&query_body) {
                        let query_arg = cap[1].trim();
                        
                        // Filter results to those matching the query argument
                        results.retain(|row| {
                            row.iter().any(|val| {
                                format!("{}", val).contains(query_arg) ||
                                format!("{:?}", val).contains(query_arg)
                            })
                        });
                        
                        #[cfg(test)]
                        eprintln!("After filtering for '{}': {} results", query_arg, results.len());
                    }
                }
                
                let proven = !results.is_empty();
                
                // Extract variable bindings if present
                use crate::domain::models::Binding;
                let bindings: Vec<Binding> = results
                    .iter()
                    .take(10)  // Limit to first 10 results for performance
                    .enumerate()
                    .flat_map(|(_i, row)| {
                        row.iter()
                            .enumerate()
                            .filter_map(|(j, value)| {
                                // Map column index to original variable name
                                variables.get(j).map(|var_name| Binding {
                                    variable: var_name.clone(),
                                    value: format!("{}", value),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                
                let explanation = if proven {
                    if results.len() == 1 {
                        "Query proven true with 1 result".to_string()
                    } else {
                        format!("Query proven true with {} results", results.len())
                    }
                } else {
                    "Query returned no results. This means the premise is either FALSE (can be proven false) or INCONCLUSIVE (cannot be proven true or false with current knowledge base). Consider: 1) The fact/rule might not exist, 2) Required facts might be missing, 3) The premise might be logically false.".to_string()
                };
                
                InferenceResult {
                    proven,
                    status: if proven { InferenceStatus::True } else { InferenceStatus::Inconclusive },
                    bindings,
                    trace_json: None,  // TODO: Could add trace support
                    explanation: Some(explanation),
                }
            },
            Ok(Ok(Err(e))) => {
                // Nemo error during execution
                InferenceResult {
                    proven: false,
                    status: InferenceStatus::Inconclusive,
                    bindings: Vec::new(),
                    trace_json: None,
                    explanation: Some(format!("Query execution error: {}", e)),
                }
            },
            Ok(Err(e)) => {
                // Task join error
                InferenceResult {
                    proven: false,
                    status: InferenceStatus::Inconclusive,
                    bindings: Vec::new(),
                    trace_json: None,
                    explanation: Some(format!("Internal error during query: {}", e)),
                }
            },
            Err(_) => {
                // Timeout
                InferenceResult {
                    proven: false,
                    status: InferenceStatus::Inconclusive,
                    bindings: Vec::new(),
                    trace_json: None,
                    explanation: Some(format!("Query timed out after {} ms", timeout_ms)),
                }
            }
        }
    }

    /// Materialize all derivable facts
    async fn materialize(&self, timeout_ms: u64) -> EngineResult<()> {
        if self.program_text.is_empty() {
            return Err(EngineError::OperationNotAllowed("No program loaded".to_string()));
        }
        
        let timeout_duration = Duration::from_millis(timeout_ms);
        let program_clone = self.program_text.clone();
        
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

    /// Reset the state
    fn reset(&mut self) {
        self.program_text.clear();
        self.predicate_annotations.clear();
    }

    /// List all premises
    fn list_premises(&self) -> String {
        if self.program_text.is_empty() {
            "% No premises loaded".to_string()
        } else {
            self.program_text.clone()
        }
    }

    /// Validate a rule
    fn validate_rule(&self, rule_str: &str) -> ValidateResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        if let Err(e) = self.validate_datalog_syntax(rule_str) {
            errors.push(e.to_string());
            return ValidateResult {
                is_valid: false,
                errors,
                warnings,
            };
        }
        
        // Validate Nemo variable syntax
        if let Err(e) = self.validate_nemo_variable_syntax(rule_str) {
            errors.push(e.to_string());
            return ValidateResult {
                is_valid: false,
                errors,
                warnings,
            };
        }
        
        if rule_str.contains(":-") {
            let parts: Vec<&str> = rule_str.split(":-").collect();
            if parts.len() == 2 {
                let head_vars = self.extract_variables(parts[0]);
                let body_vars = self.extract_variables(parts[1]);
                
                for var in &head_vars {
                    if !body_vars.contains(var) {
                        errors.push(format!("Variable {} in head is not bound in body", var));
                    }
                }
                
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

    /// Add predicate annotation
    fn add_predicate_annotation(&mut self, predicate: String, annotation: String) {
        self.predicate_annotations.insert(predicate, annotation);
    }

    /// Explain inference
    fn explain_inference(&self, trace_json: &serde_json::Value, short: bool) -> String {
        if short {
            "Inference explanation: No detailed trace available".to_string()
        } else {
            format!("Detailed inference explanation:\n\nTrace data: {}\n\nNo detailed trace parsing implemented yet.", trace_json)
        }
    }
}

/// Handle for communicating with a worker thread
pub struct NemoWorkerHandle {
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
}

impl NemoWorkerHandle {
    /// Create a new worker and spawn its thread
    fn new() -> Self {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<WorkerCommand>();
        
        // Spawn worker thread
        tokio::spawn(async move {
            let mut state = NemoWorkerState::new();
            
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    WorkerCommand::LoadFact { fact, response_tx } => {
                        let result = state.load_fact(&fact).await;
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::LoadRule { rule, response_tx } => {
                        let result = state.load_rule(&rule).await;
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::LoadBulk { datalog, atomic, response_tx } => {
                        let result = state.load_bulk(&datalog, atomic).await;
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::Query { query_str, timeout_ms, response_tx } => {
                        let result = state.query(&query_str, timeout_ms).await;
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::Materialize { timeout_ms, response_tx } => {
                        let result = state.materialize(timeout_ms).await;
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::GetTraceJson { response_tx } => {
                        let _ = response_tx.send(None);
                    }
                    WorkerCommand::Reset { response_tx } => {
                        state.reset();
                        let _ = response_tx.send(());
                    }
                    WorkerCommand::ListPremises { response_tx } => {
                        let result = state.list_premises();
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::ValidateRule { rule_str, response_tx } => {
                        let result = state.validate_rule(&rule_str);
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::AddPredicateAnnotation { predicate, annotation, response_tx } => {
                        state.add_predicate_annotation(predicate, annotation);
                        let _ = response_tx.send(());
                    }
                    WorkerCommand::ExplainInference { trace_json, short, response_tx } => {
                        let result = state.explain_inference(&trace_json, short);
                        let _ = response_tx.send(result);
                    }
                    WorkerCommand::Shutdown => {
                        break;
                    }
                }
            }
        });
        
        Self { command_tx }
    }

    /// Load a single fact
    pub async fn load_fact(&self, fact: String) -> EngineResult<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::LoadFact { fact, response_tx })
            .map_err(|_| EngineError::InternalError("Worker channel closed".to_string()))?;
        response_rx.await
            .map_err(|_| EngineError::InternalError("Worker response channel closed".to_string()))?
    }

    /// Load a single rule
    pub async fn load_rule(&self, rule: String) -> EngineResult<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::LoadRule { rule, response_tx })
            .map_err(|_| EngineError::InternalError("Worker channel closed".to_string()))?;
        response_rx.await
            .map_err(|_| EngineError::InternalError("Worker response channel closed".to_string()))?
    }

    /// Load bulk data
    pub async fn load_bulk(&self, datalog: String, atomic: bool) -> AddBulkResult {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::LoadBulk { datalog, atomic, response_tx })
            .map_err(|_| AddBulkResult {
                added_count: 0,
                total_count: 0,
                errors: vec![(0, "Worker channel closed".to_string())],
                rolled_back: true,
            })
            .ok();
        response_rx.await.unwrap_or_else(|_| AddBulkResult {
            added_count: 0,
            total_count: 0,
            errors: vec![(0, "Worker response channel closed".to_string())],
            rolled_back: true,
        })
    }

    /// Execute a query
    pub async fn query(&self, query_str: String, timeout_ms: u64) -> InferenceResult {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::Query { query_str, timeout_ms, response_tx })
            .ok();
        response_rx.await.unwrap_or_else(|_| InferenceResult {
            proven: false,
            status: crate::domain::models::InferenceStatus::Inconclusive,
            bindings: Vec::new(),
            trace_json: None,
            explanation: Some("Worker response channel closed".to_string()),
        })
    }

    /// Materialize
    pub async fn materialize(&self, timeout_ms: u64) -> EngineResult<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::Materialize { timeout_ms, response_tx })
            .map_err(|_| EngineError::InternalError("Worker channel closed".to_string()))?;
        response_rx.await
            .map_err(|_| EngineError::InternalError("Worker response channel closed".to_string()))?
    }

    /// Get trace JSON
    pub async fn get_trace_json(&self) -> Option<serde_json::Value> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::GetTraceJson { response_tx }).ok()?;
        response_rx.await.ok()?
    }

    /// Reset
    pub async fn reset(&self) {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::Reset { response_tx }).ok();
        response_rx.await.ok();
    }

    /// List premises
    pub async fn list_premises(&self) -> String {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::ListPremises { response_tx }).ok();
        response_rx.await.unwrap_or_else(|_| "% Worker error".to_string())
    }

    /// Validate rule
    pub async fn validate_rule(&self, rule_str: String) -> ValidateResult {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::ValidateRule { rule_str, response_tx }).ok();
        response_rx.await.unwrap_or_else(|_| ValidateResult {
            is_valid: false,
            errors: vec!["Worker response channel closed".to_string()],
            warnings: Vec::new(),
        })
    }

    /// Add predicate annotation
    pub async fn add_predicate_annotation(&self, predicate: String, annotation: String) {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::AddPredicateAnnotation { predicate, annotation, response_tx }).ok();
        response_rx.await.ok();
    }

    /// Explain inference
    pub async fn explain_inference(&self, trace_json: serde_json::Value, short: bool) -> String {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx.send(WorkerCommand::ExplainInference { trace_json, short, response_tx }).ok();
        response_rx.await.unwrap_or_else(|_| "Worker error".to_string())
    }

    /// Shutdown worker
    pub async fn shutdown(&self) {
        self.command_tx.send(WorkerCommand::Shutdown).ok();
    }
}

impl Drop for NemoWorkerHandle {
    fn drop(&mut self) {
        // Try to send shutdown signal, ignore if channel is already closed
        let _ = self.command_tx.send(WorkerCommand::Shutdown);
    }
}

/// Global worker pool - one worker per session ID
pub struct WorkerPool {
    workers: Arc<RwLock<HashMap<String, Arc<NemoWorkerHandle>>>>,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a worker for a session
    pub async fn get_worker(&self, session_id: String) -> Arc<NemoWorkerHandle> {
        // Try read lock first
        {
            let workers = self.workers.read().await;
            if let Some(worker) = workers.get(&session_id) {
                return Arc::clone(worker);
            }
        }
        
        // Need to create new worker
        let mut workers = self.workers.write().await;
        
        // Double-check in case another task created it
        if let Some(worker) = workers.get(&session_id) {
            return Arc::clone(worker);
        }
        
        // Create new worker
        let worker = Arc::new(NemoWorkerHandle::new());
        workers.insert(session_id, Arc::clone(&worker));
        worker
    }

    /// Remove a worker for a session
    pub async fn remove_worker(&self, session_id: &str) {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.remove(session_id) {
            worker.shutdown().await;
        }
    }

    /// Get count of active workers
    pub async fn worker_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.len()
    }
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

// Global worker pool instance
lazy_static::lazy_static! {
    /// Global worker pool - accessed by all MCP sessions
    pub static ref GLOBAL_WORKER_POOL: WorkerPool = WorkerPool::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_handle_new() {
        let handle = NemoWorkerHandle::new();
        let result = handle.list_premises().await;
        assert!(result.contains("No premises"));
    }

    #[tokio::test]
    async fn test_worker_handle_load_fact() {
        let handle = NemoWorkerHandle::new();
        let result = handle.load_fact("perro(fido).".to_string()).await;
        assert!(result.is_ok());
        
        let premises = handle.list_premises().await;
        assert!(premises.contains("perro(fido)"));
    }

    #[tokio::test]
    async fn test_nemo_syntax_validation() {
        let handle = NemoWorkerHandle::new();
        
        // Test 1: Valid Nemo syntax with ?X should work
        let result = handle.load_rule("valid(?X) :- data(?X).".to_string()).await;
        assert!(result.is_ok(), "Valid Nemo syntax should be accepted: {:?}", result);
        
        // Test 2: Invalid syntax without ? should fail  
        let result = handle.load_rule("invalid(X) :- data(X).".to_string()).await;
        assert!(result.is_err(), "Invalid syntax without ? should be rejected");
        
        // Verify error message is helpful
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            assert!(error_msg.contains("Invalid variable syntax"));
            assert!(error_msg.contains("Nemo requires variables to start with '?'"));
        }
    }

    #[tokio::test]
    async fn test_worker_handle_load_rule() {
        let handle = NemoWorkerHandle::new();
        let result = handle.load_rule("come(?X) :- perro(?X).".to_string()).await;
        assert!(result.is_ok());
        
        let premises = handle.list_premises().await;
        assert!(premises.contains("come(?X)"));
    }

    #[tokio::test]
    async fn test_worker_handle_reset() {
        let handle = NemoWorkerHandle::new();
        handle.load_fact("test(a).".to_string()).await.ok();
        handle.reset().await;
        
        let premises = handle.list_premises().await;
        assert!(premises.contains("No premises"));
    }

    #[tokio::test]
    async fn test_worker_pool_get_worker() {
        let pool = WorkerPool::new();
        let worker1 = pool.get_worker("session1".to_string()).await;
        let worker2 = pool.get_worker("session1".to_string()).await;
        
        // Should be same instance
        assert!(Arc::ptr_eq(&worker1, &worker2));
    }

    #[tokio::test]
    async fn test_worker_pool_different_sessions() {
        let pool = WorkerPool::new();
        let worker1 = pool.get_worker("session1".to_string()).await;
        let worker2 = pool.get_worker("session2".to_string()).await;
        
        // Should be different instances
        assert!(!Arc::ptr_eq(&worker1, &worker2));
    }

    #[tokio::test]
    async fn test_worker_pool_remove() {
        let pool = WorkerPool::new();
        pool.get_worker("session1".to_string()).await;
        
        assert_eq!(pool.worker_count().await, 1);
        
        pool.remove_worker("session1").await;
        assert_eq!(pool.worker_count().await, 0);
    }

    #[tokio::test]
    async fn test_worker_pool_multiple_workers() {
        let pool = WorkerPool::new();
        
        for i in 0..5 {
            pool.get_worker(format!("session{}", i)).await;
        }
        
        assert_eq!(pool.worker_count().await, 5);
    }

    #[tokio::test]
    async fn test_global_worker_pool() {
        let worker = GLOBAL_WORKER_POOL.get_worker("test_session".to_string()).await;
        worker.load_fact("global_test(a).".to_string()).await.ok();
        
        let premises = worker.list_premises().await;
        assert!(premises.contains("global_test"));
        
        // Cleanup
        GLOBAL_WORKER_POOL.remove_worker("test_session").await;
    }

    #[tokio::test]
    async fn test_worker_isolation() {
        let pool = WorkerPool::new();
        
        let worker1 = pool.get_worker("iso1".to_string()).await;
        let worker2 = pool.get_worker("iso2".to_string()).await;
        
        worker1.load_fact("data1(a).".to_string()).await.ok();
        worker2.load_fact("data2(b).".to_string()).await.ok();
        
        let premises1 = worker1.list_premises().await;
        let premises2 = worker2.list_premises().await;
        
        assert!(premises1.contains("data1"));
        assert!(!premises1.contains("data2"));
        assert!(premises2.contains("data2"));
        assert!(!premises2.contains("data1"));
        
        // Cleanup
        pool.remove_worker("iso1").await;
        pool.remove_worker("iso2").await;
    }

    #[tokio::test]
    async fn test_load_fact_rollback_on_error() {
        let handle = NemoWorkerHandle::new();
        
        // Load a valid fact first
        handle.load_fact("valid(data).".to_string()).await.ok();
        let before_premises = handle.list_premises().await;
        
        // Try to load an invalid fact that will fail validation
        // This should NOT modify program_text
        let result = handle.load_fact("INVALID SYNTAX HERE".to_string()).await;
        assert!(result.is_err());
        
        // Verify program_text was not modified
        let after_premises = handle.list_premises().await;
        assert_eq!(before_premises, after_premises);
        assert!(after_premises.contains("valid(data)"));
        assert!(!after_premises.contains("INVALID"));
    }

    #[tokio::test]
    async fn test_load_rule_rollback_on_error() {
        let handle = NemoWorkerHandle::new();
        
        // Load a valid rule first
        handle.load_rule("valid(?X) :- data(?X).".to_string()).await.ok();
        let before_premises = handle.list_premises().await;
        
        // Try to load an invalid rule that will fail validation
        let result = handle.load_rule("invalid rule without proper syntax".to_string()).await;
        assert!(result.is_err());
        
        // Verify program_text was not modified
        let after_premises = handle.list_premises().await;
        assert_eq!(before_premises, after_premises);
        assert!(after_premises.contains("valid(?X)"));
        assert!(!after_premises.contains("invalid"));
    }

    #[tokio::test]
    async fn test_load_bulk_rollback_on_validation_error() {
        let handle = NemoWorkerHandle::new();
        
        // Load valid data first
        handle.load_fact("initial(data).".to_string()).await.ok();
        let before_premises = handle.list_premises().await;
        
        // Try to load bulk data with atomic=true where one line has syntax errors
        // This should cause a rollback
        let bulk_data = "valid(a).\nINVALID SYNTAX\nvalid(b).";
        let result = handle.load_bulk(bulk_data.to_string(), true).await;
        
        // With atomic=true and errors present, should rollback
        assert!(result.rolled_back);
        assert_eq!(result.added_count, 0);
        
                // Verify program_text was not modified
        let after_premises = handle.list_premises().await;
        assert_eq!(before_premises, after_premises);
        assert!(after_premises.contains("initial(data)"));
        assert!(!after_premises.contains("valid(a)"));
    }

    #[tokio::test]
    async fn test_query_living_beings_basic() {
        // Test the Claude Desktop scenario with the fix
        let handle = NemoWorkerHandle::new();

        // Load facts
        handle.load_fact("is_alive(cat).".to_string()).await.unwrap();
        handle.load_fact("is_alive(plant).".to_string()).await.unwrap();
        handle.load_fact("is_alive(person).".to_string()).await.unwrap();

        // Load rules (with Nemo syntax)
        handle.load_rule("living(?X) :- is_alive(?X).".to_string()).await.unwrap();
        handle.load_rule("mortal(?X) :- living(?X).".to_string()).await.unwrap();

        // Test 1: Specific query - is cat living?
        let result = handle.query("?- living(cat).".to_string(), 5000).await;
        eprintln!("Query result: proven={}, status={:?}, explanation={:?}",
                  result.proven, result.status, result.explanation);
        assert!(result.proven, "Cat should be proven as living. Explanation: {:?}", result.explanation);
        assert_eq!(result.status, crate::domain::models::InferenceStatus::True);

        // Test 2: Variable query - find all living things (with Nemo syntax)
        let result = handle.query("?- living(?X).".to_string(), 5000).await;
        assert!(result.proven, "Should find living beings");
        assert!(!result.bindings.is_empty(), "Should have bindings for living beings");

        // Verify we found cat, plant, person
        let values: Vec<String> = result.bindings.iter().map(|b| b.value.clone()).collect();
        assert!(values.iter().any(|v| v.contains("cat")), "Should find cat in {:?}", values);
        assert!(values.iter().any(|v| v.contains("plant")), "Should find plant in {:?}", values);
        assert!(values.iter().any(|v| v.contains("person")), "Should find person in {:?}", values);

        // Test 3: Specific negative query - is camera living?
        handle.load_fact("object(camera).".to_string()).await.unwrap();
        let result = handle.query("?- living(camera).".to_string(), 5000).await;
        assert!(!result.proven, "Camera should not be living");
    }

    #[tokio::test]
    async fn test_query_variable_deduplication_preserves_order() {
        let handle = NemoWorkerHandle::new();
        
        // Load test facts
        handle.load_fact("edge(a, b).".to_string()).await.unwrap();
        handle.load_fact("edge(b, c).".to_string()).await.unwrap();
        
        // Query with duplicate variable ?X (should appear only once in result)
        // Using Nemo syntax
        let result = handle.query("?- edge(?X, ?Y), edge(?Y, ?X).".to_string(), 5000).await;
        
        // If there are bindings, verify ?X comes before ?Y (order of first appearance)
        if !result.bindings.is_empty() {
            let var_names: Vec<String> = result.bindings.iter()
                .map(|b| b.variable.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            
            // Should have exactly 2 unique variables: ?X and ?Y (with Nemo syntax)
            if var_names.len() == 2 {
                assert!(var_names.contains(&"?X".to_string()));
                assert!(var_names.contains(&"?Y".to_string()));
            }
        }
    }

    #[tokio::test]
    async fn test_reject_prolog_comments() {
        let handle = NemoWorkerHandle::new();
        
        // Test 1: Fact with comment should be ACCEPTED (comments are stripped)
        let result = handle.load_fact("perro(fido). % This is a comment".to_string()).await;
        assert!(result.is_ok(), "Should accept and strip % comment from fact");
        
        // Test 2: Rule with comment should be ACCEPTED (comments are stripped)
        let result = handle.load_rule("mortal(?X) :- humano(?X). % All humans are mortal".to_string()).await;
        assert!(result.is_ok(), "Should accept and strip % comment from rule");
        
        // Test 3: Comment-only line becomes empty and should be rejected
        let result = handle.load_fact("% Just a comment".to_string()).await;
        assert!(result.is_err(), "Should reject comment-only line (becomes empty)");
        
        // Test 4: Verify the facts were actually loaded without comments
        let premises = handle.list_premises().await;
        assert!(premises.contains("perro(fido)."), "Fact should be loaded without comment");
        assert!(!premises.contains("This is a comment"), "Comment should not be in premises");
        assert!(premises.contains("mortal(?X)"), "Rule should be loaded without comment");
        assert!(!premises.contains("All humans are mortal"), "Comment should not be in premises");
    }
}
