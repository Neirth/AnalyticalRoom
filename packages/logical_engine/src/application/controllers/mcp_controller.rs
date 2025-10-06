use crate::domain::services::logical_inference_engine::LogicalInferenceEngine;
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    pub query: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateRuleRequest {
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddBulkRequest {
    pub datalog: String,
    pub atomic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExplainInferenceRequest {
    pub trace_json: serde_json::Value,
    pub short: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPremisesRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResetRequest {}

/// LogicalInferenceServer provides an MCP (Model Context Protocol) interface for the logical inference engine.
///
/// This server acts as the main entry point for MCP clients to interact with the LogicalInferenceEngine.
/// It implements the MCP tool protocol, providing a set of tools that clients can invoke to:
/// - Load facts and rules into the knowledge base
/// - Execute logical queries
/// - Validate rules
/// - Get explanations of inferences
/// - Manage the knowledge base
///
/// Each server instance maintains its own isolated LogicalInferenceEngine with an independent
/// knowledge base, ensuring complete session isolation between different MCP clients.
pub struct LogicalInferenceServer {
    service: OnceCell<Arc<Mutex<LogicalInferenceEngine>>>,
    tool_router: ToolRouter<LogicalInferenceServer>,
}

impl LogicalInferenceServer {
    /// Create a new LogicalInferenceServer instance
    /// 
    /// Each instance is isolated and maintains its own knowledge base
    pub fn new() -> Self {
        Self {
            service: OnceCell::new(),
            tool_router: Self::tool_router(),
        }
    }

    /// Get or create the service instance
    async fn get_service(&self) -> &Arc<Mutex<LogicalInferenceEngine>> {
        self.service
            .get_or_init(|| async {
                let engine = LogicalInferenceEngine::new();
                Arc::new(Mutex::new(engine))
            })
            .await
    }
}

impl Default for LogicalInferenceServer {
    fn default() -> Self {
        Self::new()
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
                "# Logical Inference MCP Server\n\n\
                This server provides Datalog-based logical inference using the Nemo reasoning engine.\n\n\
                ## PRIMARY PURPOSE\n\
                Build and query logical knowledge bases using Datalog syntax to perform automated reasoning\n\
                and derive conclusions from facts and rules.\n\n\
                ## WORKFLOW\n\n\
                ### Phase 1: BUILD KNOWLEDGE BASE\n\
                1. **add_bulk(datalog, atomic)** - Add facts and rules\n\
                   - Use Datalog syntax (e.g., 'perro(fido).' or 'come(X) :- perro(X).')\n\
                   - atomic=true: all-or-nothing, atomic=false: best-effort\n\
                2. **validate_rule(rule)** - Check rule syntax and semantics\n\
                   - Detects unbound variables and other common errors\n\n\
                ### Phase 2: QUERY AND REASON\n\
                1. **query(query, timeout_ms)** - Execute queries\n\
                   - Query format: '?- predicate(args).'\n\
                   - Returns bindings and proof traces\n\
                2. **explain_inference(trace_json, short)** - Get natural language explanation\n\
                   - Converts proof traces to human-readable text\n\n\
                ### Phase 3: INSPECTION\n\
                1. **list_premises()** - View all loaded facts and rules\n\
                2. **reset()** - Clear the knowledge base\n\n\
                ## DATALOG SYNTAX\n\
                - Facts: 'predicate(constant).'\n\
                - Rules: 'head(X) :- body1(X), body2(X).'\n\
                - Queries: '?- predicate(X).'\n\
                - Variables start with uppercase (X, Y, Z)\n\
                - Constants are lowercase or quoted\n\n\
                ## IMPORTANT NOTES\n\
                - Each session has isolated knowledge base\n\
                - Nemo engine provides sound and complete inference\n\
                - Supports stratified negation and recursion detection\n\
                "
                .to_string(),
            ),
        }
    }
}

#[tool_router]
impl LogicalInferenceServer {
    /// Execute a logical query against the knowledge base
    /// 
    /// Query the loaded facts and rules to determine if a proposition can be proven.
    /// 
    /// # Arguments
    /// - `query` (string): A Datalog query starting with ?- (e.g., "?- perro(X).")
    /// - `timeout_ms` (number, optional): Maximum time to wait for query in milliseconds (default: 5000)
    /// 
    /// # Returns
    /// - InferenceResult with proven status, bindings, and trace information
    /// 
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "query",
    ///     "arguments": {
    ///       "query": "?- come(X).",
    ///       "timeout_ms": 3000
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Execute a logical query against the knowledge base. Query the loaded facts and rules to determine if a proposition can be proven. Query format: ?- predicate(args).")]
    async fn query(&self, Parameters(request): Parameters<QueryRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;
        
        let timeout_ms = request.timeout_ms.unwrap_or(5000);
        let result = service.query(&request.query, timeout_ms).await;
        
        match serde_json::to_string_pretty(&result) {
            Ok(json) => Ok(json),
            Err(e) => Ok(format!("Query executed but serialization failed: {}", e)),
        }
    }

    /// Validate a Datalog rule for syntax and semantic issues
    /// 
    /// Check if a rule is syntactically valid and doesn't have common issues
    /// like unbound variables.
    /// 
    /// # Arguments
    /// - `rule` (string): The Datalog rule to validate
    /// 
    /// # Returns
    /// - ValidateResult with validation status, errors, and warnings
    /// 
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "validate_rule",
    ///     "arguments": {
    ///       "rule": "come(X) :- perro(X), existe(X)."
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Validate a Datalog rule for syntax and semantic issues. Check if a rule is syntactically valid and doesn't have common issues like unbound variables.")]
    async fn validate_rule(&self, Parameters(request): Parameters<ValidateRuleRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;
        
        let result = service.validate_rule(&request.rule);
        
        match serde_json::to_string_pretty(&result) {
            Ok(json) => Ok(json),
            Err(e) => Ok(format!("Validation completed but serialization failed: {}", e)),
        }
    }

    /// Add multiple facts and/or rules to the knowledge base
    /// 
    /// Load multiple Datalog statements in bulk, with optional atomic behavior.
    /// 
    /// # Arguments
    /// - `datalog` (string): Multiple Datalog statements separated by newlines
    /// - `atomic` (boolean): If true, all statements must be valid or none are applied
    /// 
    /// # Returns
    /// - AddBulkResult with details about which statements were added
    /// 
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "add_bulk",
    ///     "arguments": {
    ///       "datalog": "perro(fido).\nexiste(fido).\ncome(X) :- perro(X), existe(X).",
    ///       "atomic": true
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Add multiple facts and/or rules to the knowledge base. Load multiple Datalog statements in bulk, with optional atomic behavior. If atomic=true, all statements must be valid or none are applied.")]
    async fn add_bulk(&self, Parameters(request): Parameters<AddBulkRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;
        
        let result = service.load_bulk(&request.datalog, request.atomic).await;
        
        match serde_json::to_string_pretty(&result) {
            Ok(json) => Ok(json),
            Err(e) => Ok(format!("Bulk add completed but serialization failed: {}", e)),
        }
    }

    /// Get a human-readable explanation of an inference
    /// 
    /// Convert a trace JSON from Nemo into natural language explanation.
    /// 
    /// # Arguments
    /// - `trace_json` (object): The trace JSON from a query result
    /// - `short` (boolean, optional): Whether to return a short summary (default: false)
    /// 
    /// # Returns
    /// - String with human-readable explanation
    /// 
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "explain_inference",
    ///     "arguments": {
    ///       "trace_json": {},
    ///       "short": false
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Get a human-readable explanation of an inference. Convert a trace JSON from Nemo into natural language explanation.")]
    async fn explain_inference(&self, Parameters(request): Parameters<ExplainInferenceRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;
        
        let short = request.short.unwrap_or(false);
        let explanation = service.explain_inference(&request.trace_json, short);
        
        Ok(explanation)
    }

    /// List all current premises (facts and rules) in the knowledge base
    /// 
    /// Returns all loaded facts and rules in Datalog format.
    /// 
    /// # Returns
    /// - String with all premises in Datalog format
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
    #[tool(description = "List all current premises (facts and rules) in the knowledge base")]
    async fn list_premises(&self, Parameters(_request): Parameters<ListPremisesRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let service = service.lock().await;
        
        Ok(service.list_premises())
    }

    /// Reset the knowledge base
    /// 
    /// Clear all facts, rules, and reset the inference engine.
    /// 
    /// # Returns
    /// - Confirmation message
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
    #[tool(description = "Reset the knowledge base, clearing all facts and rules")]
    async fn reset(&self, Parameters(_request): Parameters<ResetRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;
        
        service.reset();
        Ok("Knowledge base reset successfully".to_string())
    }
}
