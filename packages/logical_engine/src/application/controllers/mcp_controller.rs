//! MCP Controller for Logical Inference Engine
//!
//! This module provides the MCP (Model Context Protocol) interface for the Nemo-based
//! logical inference engine. It exposes Datalog reasoning capabilities through standardized
//! MCP tools.
//!
//! # Architecture
//! - Each MCP session gets its own isolated Nemo worker thread
//! - Workers are managed by the global worker pool
//! - Session lifecycle is managed automatically via mcp-session-id headers
//! - All operations run in dedicated worker threads for thread safety
//!
//! # Session Management
//! - Sessions are created automatically on first request
//! - Each session maintains its own knowledge base (facts and rules)
//! - Sessions are cleaned up when the MCP connection closes
//! - Worker threads are terminated gracefully on session cleanup

use crate::domain::services::LogicalInferenceService;
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

// ============================================================================
// MCP Tool Request Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoadFactRequest {
    /// Datalog fact to load (e.g., "perro(fido).")
    pub fact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoadRuleRequest {
    /// Datalog rule to load (e.g., "come(?X) :- perro(?X), existe(?X).")
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoadBulkRequest {
    /// Multiple Datalog statements separated by newlines
    pub datalog: String,
    /// If true, all statements must be valid or none are applied (default: false)
    pub atomic: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    /// Datalog query to execute (e.g., "?- come(?X).")
    pub query: String,
    /// Timeout in milliseconds (default: 5000ms)
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MaterializeRequest {
    /// Timeout in milliseconds (default: 10000ms)
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetTraceJsonRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResetRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPremisesRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateRuleRequest {
    /// Rule to validate for syntax and semantic issues
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPredicateAnnotationRequest {
    /// Predicate name (e.g., "perro")
    pub predicate: String,
    /// Human-readable description (e.g., "is a dog")
    pub annotation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExplainInferenceRequest {
    /// Trace JSON from a previous query
    pub trace_json: serde_json::Value,
    /// If true, return short summary; if false, return detailed explanation
    pub short: Option<bool>,
}

// ============================================================================
// Logical Inference Server - MCP Entry Point
// ============================================================================

/// LogicalInferenceServer provides an MCP interface for Datalog reasoning with Nemo.
///
/// This server acts as the main entry point for MCP clients to interact with the
/// Nemo logical inference engine. It implements the MCP tool protocol, providing
/// a set of tools that clients can invoke to:
/// - Load facts and rules into the knowledge base
/// - Execute logical queries with unification
/// - Materialize all derivable facts
/// - Validate Datalog syntax and semantics
/// - Explain inference traces in natural language
/// - Reset and inspect the knowledge base state
///
/// Each server instance maintains its own isolated LogicalInferenceService with
/// a dedicated Nemo worker thread, ensuring complete session isolation between
/// different MCP clients.
///
/// # Session Isolation
/// - Each MCP session (identified by mcp-session-id header) gets its own worker
/// - Workers run in dedicated threads for Nemo compatibility
/// - Knowledge bases are completely isolated between sessions
/// - Workers are cleaned up automatically when sessions end
///
/// # Thread Safety
/// - Nemo engine is NOT Send/Sync, so it runs in dedicated worker threads
/// - Commands are sent to workers via async channels
/// - Responses are returned via oneshot channels
/// - Worker pool is globally shared and thread-safe
pub struct LogicalInferenceServer {
    /// Lazy-initialized logical inference service with isolated worker
    service: OnceCell<Arc<Mutex<LogicalInferenceService>>>,
    /// MCP tool router for handling method dispatch
    tool_router: ToolRouter<LogicalInferenceServer>,
    /// Session ID for this server instance
    session_id: String,
}

impl LogicalInferenceServer {
    /// Creates a new LogicalInferenceServer instance for an MCP client connection.
    ///
    /// The server is created with a unique session ID. The underlying worker thread
    /// and Nemo engine are created lazily on first use to optimize resource usage.
    ///
    /// # Returns
    /// A new LogicalInferenceServer instance ready to handle MCP tool requests
    ///
    /// # Example
    /// ```rust,no_run
    /// use logical_engine::application::controllers::mcp_controller::LogicalInferenceServer;
    ///
    /// let server = LogicalInferenceServer::new();
    /// // Server is ready to handle MCP protocol requests
    /// ```
    pub fn new() -> LogicalInferenceServer {
        let session_id = Uuid::new_v4().to_string();
        LogicalInferenceServer {
            service: OnceCell::new(),
            tool_router: Self::tool_router(),
            session_id,
        }
    }

    /// Gets or initializes the LogicalInferenceService with an isolated worker.
    ///
    /// This method uses lazy initialization to create the LogicalInferenceService
    /// and its associated worker thread only when first needed. Each server
    /// instance gets its own completely isolated worker.
    ///
    /// # Returns
    /// A reference to the shared LogicalInferenceService wrapped in Arc<Mutex<>>
    async fn get_service(&self) -> &Arc<Mutex<LogicalInferenceService>> {
        self.service.get_or_init(|| async {
            let service = LogicalInferenceService::new(self.session_id.clone());
            Arc::new(Mutex::new(service))
        }).await
    }
}

#[tool_router]
impl LogicalInferenceServer {
    /// MCP Tool: Load a single fact into the knowledge base.
    ///
    /// This tool adds a Datalog fact to the current session's knowledge base.
    /// Facts are atomic statements that are always true (e.g., "perro(fido)." states
    /// that fido is a dog).
    ///
    /// # MCP Tool Parameters
    /// - `fact` (string): A Datalog fact in the form "predicate(args)." (required)
    ///
    /// # Returns
    /// - Success: "Successfully loaded fact: {fact}"
    /// - Error: "Failed to load fact: {error_description}"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "load_fact",
    ///     "arguments": {
    ///       "fact": "perro(fido)."
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Datalog Syntax
    /// - Fact format: `predicate(term1, term2, ..., termN).`
    /// - Predicate names must start with lowercase letter
    /// - Terms can be constants (lowercase) or numbers
    /// - Must end with a period (.)
    #[tool(description = "LOAD FACT: Add a single Datalog fact to the knowledge base. Facts are atomic statements like 'perro(fido).' or 'edad(juan, 30).'. **BEST PRACTICE**: Load generic rules FIRST to define your framework, THEN load facts as instances. Predicate names must start with lowercase. Terms can be constants (lowercase) or numbers. Must end with a period. Use this to build your knowledge base with known truths after establishing the logical framework.")]
    async fn load_fact(&self, Parameters(request): Parameters<LoadFactRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        match service.load_fact(request.fact.clone()).await {
            Ok(_) => Ok(format!("Successfully loaded fact: {}", request.fact)),
            Err(e) => Ok(format!("Failed to load fact: {}", e)),
        }
    }

    /// MCP Tool: Load a single rule into the knowledge base.
    ///
    /// This tool adds a Datalog rule to the current session's knowledge base.
    /// Rules define logical implications (e.g., "mortal(?X) :- humano(?X)." states
    /// that all humans are mortal).
    ///
    /// # MCP Tool Parameters
    /// - `rule` (string): A Datalog rule in the form "head :- body." (required)
    ///
    /// # Returns
    /// - Success: "Successfully loaded rule: {rule}"
    /// - Error: "Failed to load rule: {error_description}"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "load_rule",
    ///     "arguments": {
    ///       "rule": "mortal(?X) :- humano(?X)."
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Datalog Syntax
    /// - Rule format: `head(?X) :- body1(?X), body2(?X), ..., bodyN(?X).`
    /// - Head is what will be derived (conclusion)
    /// - Body contains conditions (premises), separated by commas (AND)
    /// - Negation: Stratified negation using tilde `~` before predicates (e.g., `~puede_volar(?X)`). The negated predicate must not depend (directly or transitively) on the rule head to ensure stratification. Negation is only allowed in rule bodies, not in heads or facts.
    /// - Disjunction (OR): Create multiple rules with same head predicate
    /// - Variables (uppercase with ? prefix) in head must appear in positive body literals (not only in negated literals)
    /// - Must end with a period (.)
    #[tool(description = "LOAD RULE: Add a Datalog rule to the knowledge base. Rules define logical implications like 'mortal(?X) :- humano(?X).' (all humans are mortal). **BEST PRACTICE**: Load rules FIRST to define your domain framework, THEN load facts. IMPORTANT: Variables MUST use Nemo syntax with '?' prefix (e.g., ?X, ?Y, ?Z). Format: head :- body1, body2, ... Variables in head must appear in positive body literals. Logical operators: AND (comma), OR (multiple rules), NOT (stratified negation with tilde ~). Negation syntax: `~predicate(?X)` means 'predicate(?X) is not derivable'. Negation must be stratified (no cyclic dependencies through negation). Examples: 'puede_conducir(?X) :- persona(?X), tiene_licencia(?X).' (AND), 'no_volador(?X) :- pajaro(?X), ~puede_volar(?X).' (stratified negation). Use this to define logical relationships and derivation rules.")]
    async fn load_rule(&self, Parameters(request): Parameters<LoadRuleRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        match service.load_rule(request.rule.clone()).await {
            Ok(_) => Ok(format!("Successfully loaded rule: {}", request.rule)),
            Err(e) => Ok(format!("Failed to load rule: {}", e)),
        }
    }

    /// MCP Tool: Load multiple facts and/or rules in bulk.
    ///
    /// This tool allows loading multiple Datalog statements at once, with optional
    /// atomic transaction semantics (all-or-nothing).
    ///
    /// # MCP Tool Parameters
    /// - `datalog` (string): Multiple Datalog statements separated by newlines (required)
    /// - `atomic` (bool): If true, all must be valid or none are applied (default: false)
    ///
    /// # Returns
    /// - Success: Detailed report of added statements, errors, and rollback status
    /// - Error: Not applicable - always returns a result report
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "load_bulk",
    ///     "arguments": {
    ///       "datalog": "perro(fido).\nperro(rex).\nexiste(fido).\nexiste(rex).",
    ///       "atomic": true
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Bulk Loading Features
    /// - Lines starting with `%` are treated as comments and ignored
    /// - Empty lines are ignored
    /// - Atomic mode: if any statement fails, none are applied
    /// - Non-atomic mode: valid statements are applied, invalid ones are reported
    /// - Returns detailed error information per line
    #[tool(description = "LOAD BULK: Load multiple Datalog statements at once (facts and/or rules). **BEST PRACTICE**: Define rules first (generic framework), then facts (specific instances). IMPORTANT: Variables MUST use Nemo syntax with '?' prefix (e.g., ?X, ?Y, ?Z). Comments: Use '% comment' on any line - they will be automatically removed. Separate statements with newlines. Empty lines are ignored. Set atomic=true for all-or-nothing (if any fails, none apply). Set atomic=false to apply valid statements and report errors for invalid ones. Returns detailed report with added count, errors per line, and rollback status. Efficient for loading large knowledge bases with clear separation between framework (rules) and data (facts).")]
    async fn load_bulk(&self, Parameters(request): Parameters<LoadBulkRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        let atomic = request.atomic.unwrap_or(false);
        let result = service.load_bulk(request.datalog, atomic).await;

        Ok(format!(
            "Bulk load result:\n\
            - Added: {}/{} statements\n\
            - Errors: {}\n\
            - Rolled back: {}\n\
            {}",
            result.added_count,
            result.total_count,
            result.errors.len(),
            result.rolled_back,
            if result.errors.is_empty() {
                "All statements loaded successfully".to_string()
            } else {
                format!(
                    "Errors:\n{}",
                    result.errors.iter()
                        .map(|(line, err)| format!("  Line {}: {}", line, err))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        ))
    }

    /// MCP Tool: Execute a logical query against the knowledge base.
    ///
    /// This tool executes a Datalog query and returns whether it can be proven
    /// true based on the current knowledge base (facts + rules). Supports variable
    /// unification and pattern matching.
    ///
    /// # MCP Tool Parameters
    /// - `query` (string): Datalog query in the form "?- predicate(args)." (required)
    /// - `timeout_ms` (u64): Query timeout in milliseconds (default: 5000ms)
    ///
    /// # Returns
    /// - Success: Detailed inference result with proof status, bindings, and explanation
    /// - Error: Not applicable - always returns an inference result
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "query",
    ///     "arguments": {
    ///       "query": "?- mortal(socrates).",
    ///       "timeout_ms": 5000
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Query Syntax
    /// - Query format: `?- predicate(term1, term2, ...).`
    /// - Must start with `?-` (query prefix)
    /// - Variables (uppercase) will be unified with matching values
    /// - Constants match literally
    /// - Supports conjunction: `?- p1(X), p2(X).` (both must be true)
    /// - Supports negation: `?- p1(X), ~p2(X).` (p1 true AND p2 false)
    /// - Must end with a period (.)
    ///
    /// # Inference Status
    /// - TRUE: Query proven true from knowledge base
    /// - FALSE: Query proven false (explicit negation)
    /// - INCONCLUSIVE: Cannot determine (missing facts/rules or timeout)
    /// - CANNOT_DEMONSTRATE: Loop detected or infinite recursion
    #[tool(description = "QUERY: Execute a logical query against the knowledge base. IMPORTANT: Variables MUST use Nemo syntax with '?' prefix (e.g., ?X, ?Y, ?Z). Format: '?- predicate(args).' with variables or constants. Supports conjunction (AND with comma) and stratified negation (NOT with tilde ~). Negation syntax: `~predicate(?X)` means 'predicate(?X) is not derivable'. Negation must be stratified (no cyclic dependencies). Examples: '?- mortal(?X).' (find all mortals), '?- humano(?X), ~mortal(?X).' (find humans that are not provably mortal - requires stratification). Returns proof status (TRUE/FALSE/INCONCLUSIVE/CANCELLED/TIMEOUT), variable bindings if applicable, and optional trace. IMPORTANT: If query returns NO RESULTS, this means the premise is either FALSE (can be proven false) or INCONCLUSIVE (cannot be proven with current knowledge). The premise might not exist in the knowledge base, required facts may be missing, or it may be logically false. Always check the explanation field for details.")]
    async fn query(&self, Parameters(request): Parameters<QueryRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        let timeout_ms = request.timeout_ms.unwrap_or(5000);
        let result = service.query(request.query.clone(), timeout_ms).await;

        Ok(format!(
            "Query result for '{}':\n\
            - Status: {:?}\n\
            - Proven: {}\n\
            - Bindings: {}\n\
            - Explanation: {}",
            request.query,
            result.status,
            result.proven,
            if result.bindings.is_empty() {
                "None".to_string()
            } else {
                result.bindings.iter()
                    .map(|b| format!("{} = {}", b.variable, b.value))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            result.explanation.unwrap_or_else(|| "No explanation available".to_string())
        ))
    }

    /// MCP Tool: Materialize all derivable facts from the current knowledge base.
    ///
    /// This tool runs the Nemo inference engine to compute the closure of all facts
    /// that can be logically derived from the current facts and rules. This is useful
    /// for precomputing all consequences before querying.
    ///
    /// # MCP Tool Parameters
    /// - `timeout_ms` (u64): Materialization timeout in milliseconds (default: 10000ms)
    ///
    /// # Returns
    /// - Success: "Successfully materialized knowledge base"
    /// - Error: "Failed to materialize: {error_description}"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "materialize",
    ///     "arguments": {
    ///       "timeout_ms": 10000
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # When to Use
    /// - Before running many queries on a static knowledge base
    /// - To precompute all logical consequences
    /// - For performance optimization with complex rule sets
    /// - Warning: Can be expensive for large or recursive rule sets
    #[tool(description = "MATERIALIZE: Run the inference engine to compute all facts derivable from current rules and facts. This precomputes the closure of the knowledge base. Use before running many queries for better performance. Can timeout on very large or complex recursive rule sets. Default timeout: 10000ms. Returns success/failure status.")]
    async fn materialize(&self, Parameters(request): Parameters<MaterializeRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        let timeout_ms = request.timeout_ms.unwrap_or(10000);
        match service.materialize(timeout_ms).await {
            Ok(_) => Ok("Successfully materialized knowledge base".to_string()),
            Err(e) => Ok(format!("Failed to materialize: {}", e)),
        }
    }

    /// MCP Tool: Get the JSON trace from the last query execution.
    ///
    /// This tool retrieves the internal trace data from the most recent query,
    /// if available. Traces contain detailed information about the inference steps.
    ///
    /// # MCP Tool Parameters
    /// None - retrieves trace from the last query execution
    ///
    /// # Returns
    /// - Success: JSON trace data if available
    /// - Success: "No trace available" if no trace exists
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "get_trace_json",
    ///     "arguments": {}
    ///   }
    /// }
    /// ```
    #[tool(description = "GET TRACE JSON: Retrieve the JSON trace from the last query execution. Traces contain detailed information about inference steps, rule applications, and unification. Returns trace JSON if available, or 'No trace available' message. Use this for debugging complex queries or understanding how conclusions were reached.")]
    async fn get_trace_json(&self, Parameters(_request): Parameters<GetTraceJsonRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        match service.get_trace_json().await {
            Some(trace) => Ok(serde_json::to_string_pretty(&trace).unwrap_or_else(|_| "Invalid JSON".to_string())),
            None => Ok("No trace available".to_string()),
        }
    }

    /// MCP Tool: Reset the entire knowledge base.
    ///
    /// This tool clears all facts, rules, and predicate annotations from the current
    /// session's knowledge base. The knowledge base returns to its initial empty state.
    ///
    /// # MCP Tool Parameters
    /// None - resets the entire knowledge base
    ///
    /// # Returns
    /// - Success: "Knowledge base reset successfully"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "reset",
    ///     "arguments": {}
    ///   }
    /// }
    /// ```
    ///
    /// # Warning
    /// This operation is irreversible. All loaded facts and rules will be lost.
    #[tool(description = "RESET: Clear all facts, rules, and annotations from the knowledge base. Returns to initial empty state. This operation is irreversible - all loaded knowledge will be lost. Use this to start fresh or between unrelated reasoning tasks.")]
    async fn reset(&self, Parameters(_request): Parameters<ResetRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        service.reset().await;
        Ok("Knowledge base reset successfully".to_string())
    }

    /// MCP Tool: List all premises (facts and rules) currently in the knowledge base.
    ///
    /// This tool returns a complete listing of all facts and rules that have been
    /// loaded into the current session's knowledge base.
    ///
    /// # MCP Tool Parameters
    /// None - lists all premises in the knowledge base
    ///
    /// # Returns
    /// - Success: Complete Datalog program text with all facts and rules
    /// - Success: "% No premises loaded" if knowledge base is empty
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "list_premises",
    ///     "arguments": {}
    ///   }
    /// }
    /// ```
    #[tool(description = "LIST PREMISES: Show all facts and rules currently in the knowledge base. Returns the complete Datalog program text. Use this to inspect current state, verify what has been loaded, or debug unexpected query results. Returns '% No premises loaded' if knowledge base is empty.")]
    async fn list_premises(&self, Parameters(_request): Parameters<ListPremisesRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        let premises = service.list_premises().await;
        Ok(premises)
    }

    /// MCP Tool: Validate a Datalog rule for syntax and semantic correctness.
    ///
    /// This tool checks a rule for syntax errors, unbound variables, and other
    /// common issues without actually loading it into the knowledge base.
    ///
    /// # MCP Tool Parameters
    /// - `rule` (string): Datalog rule to validate (required)
    ///
    /// # Returns
    /// - Success: Validation result with errors and warnings
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "validate_rule",
    ///     "arguments": {
    ///       "rule": "mortal(?X, ?Y) :- humano(?X)."
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Validation Checks
    /// - Syntax correctness (predicate format, operator usage)
    /// - Unbound variables in head (all head vars must appear in body)
    /// - Empty rule bodies (warning)
    /// - Invalid predicate names or structure
    #[tool(description = "VALIDATE RULE: Check a Datalog rule for syntax and semantic errors without loading it. Validates: syntax correctness, unbound variables (head vars must appear in body), empty bodies, invalid predicates. Returns validation status, list of errors, and list of warnings. Use this before load_rule to catch issues early.")]
    async fn validate_rule(&self, Parameters(request): Parameters<ValidateRuleRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        let result = service.validate_rule(request.rule.clone()).await;

        Ok(format!(
            "Validation result for '{}':\n\
            - Valid: {}\n\
            - Errors: {}\n\
            - Warnings: {}",
            request.rule,
            result.is_valid,
            if result.errors.is_empty() {
                "None".to_string()
            } else {
                result.errors.join(", ")
            },
            if result.warnings.is_empty() {
                "None".to_string()
            } else {
                result.warnings.join(", ")
            }
        ))
    }

    /// MCP Tool: Add a human-readable annotation for a predicate.
    ///
    /// This tool associates a natural language description with a predicate name,
    /// which can be used for generating human-readable explanations.
    ///
    /// # MCP Tool Parameters
    /// - `predicate` (string): Predicate name (e.g., "perro") (required)
    /// - `annotation` (string): Human-readable description (e.g., "is a dog") (required)
    ///
    /// # Returns
    /// - Success: "Successfully added annotation for predicate '{predicate}'"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "add_predicate_annotation",
    ///     "arguments": {
    ///       "predicate": "perro",
    ///       "annotation": "is a dog"
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "ADD PREDICATE ANNOTATION: Associate a human-readable description with a predicate name. For example, annotate 'perro' as 'is a dog'. These annotations are used when generating natural language explanations via explain_inference. Use this to make inference traces more understandable for end users.")]
    async fn add_predicate_annotation(&self, Parameters(request): Parameters<AddPredicateAnnotationRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        service.add_predicate_annotation(request.predicate.clone(), request.annotation.clone()).await;
        Ok(format!("Successfully added annotation for predicate '{}'", request.predicate))
    }

    /// MCP Tool: Generate a natural language explanation of an inference trace.
    ///
    /// This tool converts a JSON inference trace into a human-readable explanation,
    /// using predicate annotations if available.
    ///
    /// # MCP Tool Parameters
    /// - `trace_json` (JSON): Inference trace from a previous query (required)
    /// - `short` (bool): If true, return brief summary; if false, return detailed explanation (default: false)
    ///
    /// # Returns
    /// - Success: Natural language explanation of the inference
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "explain_inference",
    ///     "arguments": {
    ///       "trace_json": {"steps": [...], "rules": [...]},
    ///       "short": false
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "EXPLAIN INFERENCE: Convert a JSON inference trace into human-readable natural language explanation. Use predicate annotations (from add_predicate_annotation) to make explanations more understandable. Set short=true for brief summary, short=false for detailed step-by-step explanation. Input trace_json from previous query's trace. Useful for explaining 'why' a conclusion was reached.")]
    async fn explain_inference(&self, Parameters(request): Parameters<ExplainInferenceRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;

        let short = request.short.unwrap_or(false);
        let explanation = service.explain_inference(request.trace_json, short).await;
        Ok(explanation)
    }
}

#[tool_handler]
impl ServerHandler for LogicalInferenceServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "# Logical Inference Engine MCP Server\n\n\
                This server provides Datalog-based logical reasoning and inference capabilities using the Nemo engine.\n\n\
                ## PRIMARY PURPOSE\n\
                Enable logical reasoning, knowledge representation, and automated inference using Datalog - a declarative\n\
                logic programming language. Build knowledge bases with facts and rules, then query for logical consequences.\n\n\
                ## CORE CONCEPTS\n\n\
                ### Facts\n\
                Facts are atomic statements that are always true:\n\
                - Format: `predicate(term1, term2, ..., termN).`\n\
                - Example: `perro(fido).` (fido is a dog)\n\
                - Example: `edad(juan, 30).` (juan's age is 30)\n\
                - Predicate names start with lowercase\n\
                - Terms can be constants (lowercase) or numbers\n\n\
                ### Rules\n\
                Rules define logical implications (if-then relationships):\n\
                - Format: `head(?X) :- body1(?X), body2(?X).`\n\
                - Example: `mortal(?X) :- humano(?X).` (all humans are mortal)\n\
                - Head is the conclusion (what is derived)\n\
                - Body contains premises (conditions), joined by comma (AND)\n\
                - Variables (uppercase with ? prefix) in head MUST appear in body\n\n\
                #### Logical Operators\n\
                - **Conjunction (AND)**: Use comma `,` to combine conditions\n\
                  - `puede_conducir(?X) :- persona(?X), mayor_edad(?X), tiene_licencia(?X).`\n\
                  - All conditions must be true\n\
                - **Disjunction (OR)**: Define multiple rules with same head predicate\n\
                  - `mamifero(?X) :- perro(?X).`\n\
                  - `mamifero(?X) :- gato(?X).`\n\
                  - Any rule can make the head true\n\
                - **Negation (NOT)**: Use tilde `~` before a predicate\n\
                  - `no_volador(?X) :- pajaro(?X), ~puede_volar(?X).`\n\
                  - Negation requires closed-world assumption (what's not proven is false)\n\n\
                ### Queries\n\
                Queries test whether something can be proven:\n\
                - Format: `?- predicate(args).`\n\
                - Example: `?- mortal(socrates).` (is Socrates mortal?)\n\
                - Example: `?- perro(?X).` (what are all the dogs?)\n\
                - Variables (uppercase with ? prefix) get unified with matching values\n\n\
                ## TYPICAL WORKFLOW\n\n\
                **IMPORTANT**: Follow this recommended order for best results:\n\
                1. Define the formal framework (generic rules and structure)\n\
                2. Load specific facts (concrete instances)\n\
                3. Execute queries\n\n\
                ### 1. Define the Formal Framework First\n\
                ```\n\
                # STEP 1: Define generic rules WITHOUT specific facts\n\
                # This establishes the logical framework of your domain\n\
                \n\
                # Define what it means to be mortal\n\
                load_rule('mortal(?X) :- humano(?X).')\n\
                \n\
                # Define what it means to be a philosopher\n\
                load_rule('filosofo(?X) :- humano(?X), sabio(?X).')\n\
                \n\
                # Define transitive relationships\n\
                load_rule('ancestro(?X, ?Y) :- padre(?X, ?Y).')\n\
                load_rule('ancestro(?X, ?Z) :- padre(?X, ?Y), ancestro(?Y, ?Z).')\n\
                ```\n\n\
                ### 2. Load Specific Facts\n\
                ```\n\
                # STEP 2: Now add concrete instances that use the framework\n\
                \n\
                # Load facts about specific individuals\n\
                load_fact('humano(socrates).')\n\
                load_fact('humano(platon).')\n\
                load_fact('humano(aristoteles).')\n\
                load_fact('sabio(socrates).')\n\
                load_fact('sabio(platon).')\n\
                ```\n\n\
                ### 3. Query the Knowledge Base\n\
                ```\n\
                # STEP 3: Ask questions using the framework and facts\n\
                \n\
                # Ask specific questions\n\
                query('?- mortal(socrates).') → TRUE (all humans are mortal)\n\
                query('?- filosofo(socrates).') → TRUE (Socrates is human and wise)\n\
                query('?- perro(socrates).') → FALSE/INCONCLUSIVE (not a dog)\n\
                \n\
                # Find all solutions with variables\n\
                query('?- humano(?X).') → ?X = socrates, platon, aristoteles\n\
                query('?- filosofo(?X).') → ?X = socrates, platon\n\
                ```\n\n\
                ### 4. Validate and Inspect\n\
                ```\n\
                # Check rule before loading\n\
                validate_rule('ancestro(?X, ?Z) :- padre(?X, ?Y), ancestro(?Y, ?Z).')\n\
                \n\
                # See what's loaded\n\
                list_premises()\n\
                ```\n\n\
                ### 5. Optimize and Explain\n\
                ```\n\
                # Precompute all derivable facts\n\
                materialize()\n\
                \n\
                # Get human-readable explanations\n\
                add_predicate_annotation('humano', 'is human')\n\
                add_predicate_annotation('mortal', 'is mortal')\n\
                explain_inference(trace, short=false)\n\
                ```\n\n\
                ## RECOMMENDED WORKFLOW PRINCIPLE\n\n\
                **Framework-First Approach**: Always define the generic logical framework (rules and relationships) \n\
                BEFORE loading specific instances (facts). This approach:\n\
                - Creates a reusable knowledge structure\n\
                - Makes the domain model explicit and clear\n\
                - Separates domain logic from data\n\
                - Enables better validation and understanding\n\
                - Follows declarative programming best practices\n\n\
                **Anti-Pattern to Avoid**: Do NOT mix ad-hoc fact definitions with implicit rules. \n\
                Instead of loading facts without first establishing the framework, always:\n\
                1. Define generic predicates and their relationships (rules)\n\
                2. Then instantiate specific cases (facts)\n\
                3. Finally query the structured knowledge base\n\n\
                ## TOOLS REFERENCE\n\n\
                ### Knowledge Base Construction\n\
                - **load_fact**: Add a single fact\n\
                - **load_rule**: Add a single rule\n\
                - **load_bulk**: Load many facts/rules at once (with atomic option)\n\
                - **reset**: Clear entire knowledge base\n\n\
                ### Querying and Reasoning\n\
                - **query**: Execute logical query with unification\n\
                - **materialize**: Precompute all derivable facts\n\n\
                ### Validation and Inspection\n\
                - **validate_rule**: Check rule syntax/semantics before loading\n\
                - **list_premises**: Show all loaded facts and rules\n\
                - **get_trace_json**: Get detailed trace from last query\n\n\
                ### Explanation and Documentation\n\
                - **add_predicate_annotation**: Add human-readable predicate descriptions\n\
                - **explain_inference**: Convert trace to natural language\n\n\
                ## ADVANCED FEATURES\n\n\
                ### Recursive Rules\n\
                Datalog supports recursion for transitive relationships.\n\
                **Framework-First Example**:\n\
                ```\n\
                % Step 1: Define generic recursive framework\n\
                ancestro(?X, ?Y) :- padre(?X, ?Y).\n\
                ancestro(?X, ?Z) :- padre(?X, ?Y), ancestro(?Y, ?Z).\n\
                \n\
                % Step 2: Add specific facts\n\
                padre(juan, maria).\n\
                padre(maria, pedro).\n\
                \n\
                % Step 3: Query\n\
                ?- ancestro(juan, pedro). % TRUE (juan -> maria -> pedro)\n\
                ```\n\n\
                ### Complex Queries\n\
                Combine multiple conditions:\n\
                ```\n\
                ?- humano(?X), mortal(?X), sabio(?X).\n\
                ```\n\n\
                ### Negation in Queries\n\
                Find things that DON'T satisfy a condition:\n\
                ```\n\
                % Find motor vehicles without insurance\n\
                ?- vehiculo_motor(?X), ~seguro_pagado(?X).\n\
                ```\n\n\
                ### Bulk Loading with Comments\n\
                ```\n\
                % RECOMMENDED: Define framework (rules) first, then facts\n\
                load_bulk(\n\
                  '% Domain Framework - Generic Rules\\n\
                   mortal(?X) :- humano(?X).\\n\
                   filosofo(?X) :- humano(?X), sabio(?X).\\n\
                   % Specific Instances - Facts\\n\
                   humano(socrates).\\n\
                   humano(platon).\\n\
                   sabio(socrates).\\n\
                   sabio(platon).',\n\
                  atomic=true\n\
                )\n\
                ```\n\n\
                ## BEST PRACTICES\n\n\
                1. **Framework-First**: ALWAYS define generic rules BEFORE loading specific facts\n\
                   - Define the domain structure with rules first\n\
                   - Then instantiate with concrete facts\n\
                   - This separates logic from data and creates reusable models\n\
                2. **Validate First**: Use validate_rule before load_rule to catch errors early\n\
                3. **Use Annotations**: Add predicate_annotation for better explanations\n\
                4. **Atomic Bulk Loads**: Use atomic=true when loading related facts/rules\n\
                5. **List Premises**: Regularly check what's loaded with list_premises\n\
                6. **Materialize for Performance**: Run materialize before many queries\n\
                7. **Avoid Ad-hoc Attributes**: Don't create situation-specific predicates; use generic framework\n\n\
                ## COMMON PATTERNS\n\n\
                ### Transitive Relationships\n\
                ```\n\
                ancestro(?X, ?Y) :- padre(?X, ?Y).\n\
                ancestro(?X, ?Z) :- padre(?X, ?Y), ancestro(?Y, ?Z).\n\
                ```\n\n\
                ### Classification Hierarchies (Disjunction/OR)\n\
                ```\n\
                animal(?X) :- perro(?X).\n\
                animal(?X) :- gato(?X).\n\
                mamifero(?X) :- animal(?X), sangre_caliente(?X).\n\
                ```\n\n\
                ### Negation (NOT) - Closed World Assumption\n\
                ```\n\
                % Birds that cannot fly\n\
                no_volador(?X) :- pajaro(?X), ~puede_volar(?X).\n\
                \n\
                % Vehicles without insurance\n\
                sin_seguro(?X) :- vehiculo(?X), ~seguro_pagado(?X).\n\
                ```\n\n\
                ### Conjunction (AND) - Multiple Conditions\n\
                ```\n\
                % Can drive: must be person AND adult AND have license\n\
                puede_conducir(?X) :- persona(?X), mayor_edad(?X), tiene_licencia(?X).\n\
                \n\
                % Philosopher: must be human AND wise\n\
                filosofo(?X) :- humano(?X), sabio(?X).\n\
                ```\n\n\
                ### Complex Combinations (AND + OR + NOT)\n\
                ```\n\
                % Motor vehicle (car OR motorcycle)\n\
                vehiculo_motor(X) :- coche(X).\n\
                vehiculo_motor(X) :- moto(X).\n\
                \n\
                % Can drive legally (has motor AND has insurance AND NOT bicycle)\n\
                puede_conducir_legal(X) :- vehiculo_motor(X), seguro_pagado(X), ~bicicleta(X).\n\
                \n\
                % Eco-friendly (bicycle OR NOT has motor)\n\
                eco_amigable(?X) :- bicicleta(?X).\n\
                eco_amigable(?X) :- ~motor(?X).\n\
                ```\n\n\
                ### Conditional Properties\n\
                ```\n\
                puede_volar(?X) :- pajaro(?X), no_pinguino(?X).\n\
                puede_nadar(?X) :- pez(?X).\n\
                puede_nadar(?X) :- mamifero(?X), acuatico(?X).\n\
                ```\n\n\
                ## ERROR HANDLING\n\n\
                All tools return detailed error messages for:\n\
                - Syntax errors in Datalog statements\n\
                - Unbound variables in rules\n\
                - Timeout errors in materialization/queries\n\
                - Invalid predicate names or structures\n\n\
                ## SESSION ISOLATION\n\n\
                Each MCP session gets:\n\
                - Its own isolated knowledge base\n\
                - Its own dedicated Nemo worker thread\n\
                - Complete independence from other sessions\n\
                - Automatic cleanup when session ends\n\n\
                ## PERFORMANCE NOTES\n\n\
                - Facts and rules are stored in memory only (no persistence)\n\
                - Materialization can be expensive for large recursive rule sets\n\
                - Queries have default 5s timeout, materialization has 10s timeout\n\
                - Use bulk loading for better performance when adding many statements\n\n\
                ## LIMITATIONS\n\n\
                - Negation: Only stratified negation is supported (negated predicates must not depend, directly or transitively, on the rule head). Non-stratified negation will cause validation errors.\n\
                - Variables: Variables in rule heads must appear in at least one positive (non-negated) body literal.\n\
                - Arithmetic: Limited arithmetic operations in current version\n\
                - Persistence: No persistence between sessions (in-memory only)\n\
                - Recursion: Timeout required for unbounded recursion"
                    .to_string(),
            ),
        }
    }
}

impl Default for LogicalInferenceServer {
    fn default() -> Self {
        Self::new()
    }
}
