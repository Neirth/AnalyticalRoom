use crate::domain::models::types::*;
use crate::domain::services::tree_engine_service::TreeEngineService;
use rmcp::{ handler::server::tool::{ToolRouter}, handler::server::wrapper::Parameters, model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo}, schemars::JsonSchema, tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateTreeRequest {
    pub premise: String,
    pub complexity: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddLeafRequest {
    pub premise: String,
    pub reasoning: String,
    pub probability: f64,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExpandLeafRequest {
    pub node_id: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PruneTreeRequest {
    pub aggressiveness: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportPathsRequest {
    pub narrative_style: String,
    pub insights: Vec<String>,
    pub confidence_assessment: f64,
}

/// TreeEngineServer provides an MCP (Model Context Protocol) interface for the probability tree engine.
///
/// This server acts as the main entry point for MCP clients to interact with the TreeEngineService.
/// It implements the MCP tool protocol, providing a set of tools that clients can invoke to:
/// - Create and manipulate probability trees
/// - Add and expand nodes
/// - Perform tree analysis and validation
/// - Export analysis results
/// - Inspect tree structure and statistics
///
/// Each server instance maintains its own isolated TreeEngineService with an independent
/// in-memory database, ensuring complete session isolation between different MCP clients.
///
/// # Authentication
///
/// The server includes a dummy authentication system for MCP client compatibility. This system:
/// - Always allows access (dummy authentication for compatibility)
/// - Real security comes from MCP session isolation via mcp-session-id headers
/// - Maintains token/session tracking for logging and debugging purposes
/// - Follows OAuth-like patterns for client compatibility without real security enforcement
///
/// # Architecture
/// - Uses `OnceCell` for lazy initialization of the TreeEngineService
/// - Each server instance gets its own SurrealDB in-memory database
/// - Thread-safe access through Arc<Mutex<TreeEngineService>>
/// - Implements MCP tool router pattern for method dispatch
/// - Includes dummy authentication system for MCP client compatibility
pub struct TreeEngineServer {
    /// Lazy-initialized tree engine service with isolated database
    service: OnceCell<Arc<Mutex<TreeEngineService>>>,
    /// MCP tool router for handling method dispatch
    tool_router: ToolRouter<TreeEngineServer>,
}

impl TreeEngineServer {
    /// Creates a new TreeEngineServer instance ready for MCP client connections.
    ///
    /// The server is created in an uninitialized state - the underlying TreeEngineService
    /// and database connection are created lazily on first use to optimize resource usage
    /// and ensure proper async initialization.
    ///
    /// # Returns
    /// A new TreeEngineServer instance ready to handle MCP tool requests
    ///
    /// # Example
    /// ```rust,no_run
    /// use deep_analytics::controllers::mcp_controller::TreeEngineServer;
    ///
    /// let server = TreeEngineServer::new();
    /// // Server is ready to handle MCP protocol requests
    /// ```
    pub fn new() -> TreeEngineServer {
        TreeEngineServer {
            service: OnceCell::new(),
            tool_router: Self::tool_router(),
        }
    }

    /// Gets or initializes the TreeEngineService with an isolated database connection.
    ///
    /// This method uses lazy initialization to create the TreeEngineService and its
    /// associated SurrealDB in-memory database only when first needed. Each server
    /// instance gets its own completely isolated database.
    ///
    /// # Returns
    /// A reference to the shared TreeEngineService wrapped in Arc<Mutex<>> for thread safety
    ///
    /// # Database Configuration
    /// - Uses SurrealDB in-memory engine for fast, isolated storage
    /// - Namespace: "analytics"
    /// - Database: "trees"
    /// - No persistence - data is session-specific
    async fn get_service(&self) -> &Arc<Mutex<TreeEngineService>> {
        self.service.get_or_init(|| async {
            let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
            db.use_ns("analytics").use_db("trees").await.unwrap();

            let service = TreeEngineService::new(Arc::new(db));
            Arc::new(Mutex::new(service))
        }).await
    }
}

#[tool_router]
impl TreeEngineServer {
    /// MCP Tool: Creates a new probability tree with the specified premise and complexity.
    ///
    /// This tool initializes a new probability tree analysis session by creating a root node
    /// with the provided premise and setting the complexity level for the analysis engine.
    /// Any existing tree data in the current session is cleared before creating the new tree.
    ///
    /// # MCP Tool Parameters
    /// - `premise` (string): The main question or statement to analyze (minimum 10 characters)
    /// - `complexity` (u8): Analysis complexity level from 1-10 affecting tree behavior
    ///
    /// # Returns
    /// - Success: "Successfully created probability tree with ID: {node_id}"
    /// - Error: "Failed to create tree: {error_description}"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "create_tree",
    ///     "arguments": {
    ///       "premise": "Should we expand into the European market?",
    ///       "complexity": 7
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Create a new probability tree with the given premise and complexity level")]
    async fn create_tree(&self, Parameters(request): Parameters<CreateTreeRequest>) -> String {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        match service.create_tree(request.premise, request.complexity).await {
            Ok(tree_id) => format!("Successfully created probability tree with ID: {}", tree_id),
            Err(e) => format!("Failed to create tree: {}", e),
        }
    }

    /// MCP Tool: Adds a new leaf node to the probability tree at the current cursor position.
    ///
    /// This tool extends the probability tree by adding a new child node at the cursor position.
    /// The cursor is automatically set by create_tree (to root) and expand_leaf (to expanded node).
    /// This provides seamless workflow where you expand a node and then add children to it.
    ///
    /// # MCP Tool Parameters
    /// - `premise` (string): The premise/statement for this probability branch (required)
    /// - `reasoning` (string): Detailed reasoning supporting this branch (required)
    /// - `probability` (f64): Probability value between 0.0 and 1.0 (inclusive)
    /// - `confidence` (u8): Confidence level from 1-10 indicating assessment certainty
    ///
    /// # Returns
    /// - Success: "Successfully added leaf node with ID: {node_id}"
    /// - Error: Various error messages for validation failures or system errors
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "add_leaf",
    ///     "arguments": {
    ///       "premise": "Market research shows positive demand",
    ///       "reasoning": "Survey of 1000 potential customers shows 75% interest",
    ///       "probability": 0.75,
    ///       "confidence": 8
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Add a new leaf node to the probability tree at the current expanded leaf")]
    async fn add_leaf(&self, Parameters(request): Parameters<AddLeafRequest>) -> String {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        match service.add_leaf(
            request.premise,
            request.reasoning,
            request.probability,
            request.confidence,
        ).await {
            Ok(node_id) => format!("Successfully added leaf node with ID: {}", node_id),
            Err(e) => format!("Failed to add leaf: {}", e),
        }
    }

    /// MCP Tool: Expands a leaf node by updating its reasoning and potentially generating new branches.
    ///
    /// This tool transforms a leaf node into a branch node by applying new reasoning
    /// and potentially creating child nodes based on the tree's complexity configuration.
    /// The expansion follows the established probability tree analysis patterns.
    ///
    /// # MCP Tool Parameters
    /// - `node_id` (string): ID of the leaf node to expand (must be a valid leaf node)
    /// - `rationale` (string): New reasoning/rationale for the expanded analysis
    ///
    /// # Returns
    /// - Success: "Successfully expanded leaf. Created {count} new nodes"
    /// - Error: "Failed to expand leaf: {error_description}"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "expand_leaf",
    ///     "arguments": {
    ///       "node_id": "node:xyz789",
    ///       "rationale": "Further analysis reveals multiple subcategories requiring separate evaluation"
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Expand a leaf node by generating new child branches")]
    async fn expand_leaf(&self, Parameters(request): Parameters<ExpandLeafRequest>) -> String {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        match service.expand_leaf(request.node_id, request.rationale).await {
            Ok(expanded_nodes) => format!("Successfully expanded leaf. Created {} new nodes", expanded_nodes.len()),
            Err(e) => format!("Failed to expand leaf: {}", e),
        }
    }

    /// MCP Tool: Prunes the probability tree by removing low-probability branches.
    ///
    /// This tool optimizes the tree structure by removing branches with probabilities
    /// below a calculated threshold based on the aggressiveness parameter. It helps
    /// focus analysis on the most probable scenarios while maintaining tree coherence.
    ///
    /// # MCP Tool Parameters
    /// - `aggressiveness` (optional f64): Pruning aggressiveness level 0.0-1.0 (defaults to 0.5)
    ///   - 0.0 = Very conservative (removes only extremely low probability branches)
    ///   - 0.5 = Balanced pruning (default)
    ///   - 1.0 = Aggressive pruning (removes more branches, keeps only highest probabilities)
    ///
    /// # Returns
    /// - Success: "Pruned {removed} nodes, preserved {preserved} nodes with aggressiveness level {level}"
    /// - Error: "Failed to prune tree: {error_description}"
    ///
    /// # Pruning Logic
    /// - Calculates threshold based on aggressiveness and tree statistics
    /// - Preserves critical path nodes regardless of probability
    /// - Maintains tree structural integrity
    /// - Updates parent-child relationships after pruning
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "prune_tree",
    ///     "arguments": {
    ///       "aggressiveness": 0.7
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Prune the probability tree by removing low-probability branches")]
    async fn prune_tree(&self,  Parameters(request): Parameters<PruneTreeRequest>) -> String {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        let aggressiveness = request.aggressiveness.unwrap_or(0.5);

        match service.prune_tree(aggressiveness).await {
            Ok(result) => format!("Pruned {} nodes, preserved {} nodes with aggressiveness level {:.2}",
                result.statistics.removed_count,
                result.statistics.preserved_count,
                result.statistics.aggressiveness_level),
            Err(e) => format!("Failed to prune tree: {}", e),
        }
    }

    /// MCP Tool: Exports surviving probability paths with comprehensive analysis and insights.
    ///
    /// This tool generates a detailed analysis report of all viable probability paths
    /// in the tree, combining them with user-provided insights and presenting the
    /// results in the specified narrative style. It's the primary output tool for
    /// probability tree analysis.
    ///
    /// # MCP Tool Parameters
    /// - `narrative_style` (string): Presentation style for the analysis report
    ///   - "Analytical" = Data-focused, technical presentation (default)
    ///   - "Strategic" = Business-oriented, decision-focused format
    ///   - "Storytelling" = Narrative-driven, engaging presentation
    /// - `insights` (array of strings): User insights to integrate (minimum 3 required)
    /// - `confidence_assessment` (f64): Overall confidence in analysis (0.0-1.0)
    ///
    /// # Returns
    /// - Success: "Exported {count} surviving paths with {insights} insights (confidence: {level})"
    /// - Error: "Failed to export paths: {error_description}"
    ///
    /// # Analysis Includes
    /// - All viable probability paths from root to leaves
    /// - Integrated reasoning chains for each path
    /// - Statistical summaries and confidence assessments
    /// - User insights woven into the narrative
    /// - Formatted according to specified style
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "export_paths",
    ///     "arguments": {
    ///       "narrative_style": "Strategic",
    ///       "insights": [
    ///         "Market conditions strongly favor this decision",
    ///         "Risk tolerance should be considered carefully",
    ///         "Timeline constraints may impact feasibility"
    ///       ],
    ///       "confidence_assessment": 0.85
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "Export surviving probability paths with analysis and insights")]
    async fn export_paths(&self,  Parameters(request): Parameters<ExportPathsRequest>) -> String {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        let style = match request.narrative_style.as_str() {
            "Strategic" => NarrativeStyle::Strategic,
            "Storytelling" => NarrativeStyle::Storytelling,
            _ => NarrativeStyle::Analytical,
        };

        match service.export_paths(style, request.insights, request.confidence_assessment).await {
            Ok(result) => {
                // Return comprehensive analysis result using Display trait for rich formatting
                result.to_string()
            },
            Err(e) => format!("Failed to export paths: {}", e),
        }
    }

    /// MCP Tool: Inspects the current state and structure of the probability tree.
    ///
    /// This tool provides a comprehensive analysis of the current tree structure,
    /// including statistical summaries, node counts, depth analysis, and probability
    /// assessments. It's essential for monitoring tree development and understanding
    /// the current state of analysis.
    ///
    /// # MCP Tool Parameters
    /// None - this tool takes no parameters and analyzes the current tree state.
    ///
    /// # Returns
    /// - Success: Formatted tree analysis report including:
    ///   - Total number of nodes in the tree
    ///   - Number of active probability paths
    ///   - Maximum depth reached in the tree
    ///   - Average probability across all nodes
    ///   - Complexity score based on tree structure
    /// - Error: "Failed to inspect tree: {error_description}"
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "inspect_tree",
    ///     "arguments": {}
    ///   }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```text
    /// Tree Analysis:
    /// - Total nodes: 5
    /// - Active paths: 3
    /// - Max depth: 2
    /// - Avg probability: 0.72
    /// - Complexity score: 1.85
    /// ```
    #[tool(description = "Inspect the current state and structure of the probability tree")]
    async fn inspect_tree(&self) -> String {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        match service.inspect_tree().await {
            Ok(visualization) => visualization.to_string(),
            Err(e) => format!("Failed to inspect tree: {}", e),
        }
    }

    /// MCP Tool: Validates the logical coherence and consistency of the probability tree.
    ///
    /// This tool performs comprehensive logical analysis to detect contradictions,
    /// inconsistencies, and coherence violations within the tree structure. It helps
    /// ensure that the probability assessments and reasoning chains are logically sound.
    ///
    /// # MCP Tool Parameters
    /// None - this tool analyzes the entire current tree structure.
    ///
    /// # Returns
    /// - Success: "Coherence validation: {PASSED/FAILED} (coherent: {bool}, {count} contradictions found, {count} nodes eliminated)"
    /// - Error: "Failed to validate coherence: {error_description}"
    ///
    /// # Analysis Performed
    /// - Logical consistency between parent-child relationships
    /// - Probability value coherence across branches
    /// - Reasoning chain validation
    /// - Contradiction detection and resolution suggestions
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "validate_coherence",
    ///     "arguments": {}
    ///   }
    /// }
    /// ```
    #[tool(description = "Validate logical coherence of the probability tree")]
    async fn validate_coherence(&self) -> String {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        match service.validate_coherence().await {
            Ok(result) => format!("Coherence validation: {} (coherent: {}, {} contradictions found, {} nodes eliminated)",
                if result.is_coherent { "PASSED" } else { "FAILED" },
                result.is_coherent,
                result.contradictions.len(),
                result.eliminated_nodes.len()),
            Err(e) => format!("Failed to validate coherence: {}", e),
        }
    }

    /// MCP Tool: Analyzes and reports the probability validation status of the tree.
    ///
    /// This tool provides detailed analysis of probability values throughout the tree,
    /// identifying violations, inconsistencies, and providing suggestions for maintaining
    /// mathematical coherence in probability assessments.
    ///
    /// # MCP Tool Parameters
    /// None - analyzes all probability values in the current tree.
    ///
    /// # Returns
    /// - Success: "Probability status: {VALID/INVALID} (valid: {bool}, {count} violations, {count} suggestions)"
    /// - Error: "Failed to get probability status: {error_description}"
    ///
    /// # Validation Checks
    /// - Probability range validation [0.0, 1.0]
    /// - Confidence level validation [1, 10]
    /// - Sibling probability sum validation
    /// - Parent-child probability consistency
    /// - Minimum threshold compliance
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "probability_status",
    ///     "arguments": {}
    ///   }
    /// }
    /// ```
    #[tool(description = "Get probability analysis status for tree validation")]
    async fn probability_status(&self) -> String {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        match service.probability_status().await {
            Ok(result) => format!("Probability status: {} (valid: {}, {} violations, {} suggestions)",
                if result.is_valid { "VALID" } else { "INVALID" },
                result.is_valid,
                result.violations.len(),
                result.suggestions.len()),
            Err(e) => format!("Failed to get probability status: {}", e),
        }
    }
}

#[tool_handler]
impl ServerHandler for TreeEngineServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Deep Analytics Tree Engine MCP Server\n\n\
                This server provides probabilistic decision tree analysis capabilities.\n\n\
                Available tools:\n\
                - create_tree: Create a new probability tree\n\
                - add_leaf: Add leaf nodes to the tree\n\
                - expand_leaf: Expand nodes with new branches\n\
                - prune_tree: Remove low-probability branches\n\
                - export_paths: Export analysis results\n\
                - inspect_tree: View tree structure\n\
                - validate_coherence: Check logical consistency\n\
                - probability_status: Get validation status\n\n\
                Use these tools to build, analyze, and export probability trees for complex decision-making scenarios."
                    .to_string(),
            ),
        }
    }
}
