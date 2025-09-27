use crate::domain::models::types::*;
use crate::domain::services::tree_engine_service::TreeEngineService;
use rmcp::{ handler::server::{tool::ToolRouter, wrapper::Parameters}, model::{ErrorCode, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo}, schemars::JsonSchema, tool, tool_handler, tool_router, ErrorData, ServerHandler};
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateTreeRequest {
    pub premise: String,
    pub complexity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddLeafRequest {
    pub premise: String,
    pub reasoning: String,
    pub probability: f64,
    pub confidence: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InspectTreeRequest{}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateCoherenceRequest{}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProbabilityStatusRequest{}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NavigateToRequest {
    pub node_id: String,
    pub justification: String,
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
    /// - `complexity` (i64): Analysis complexity level from 1-10 affecting tree behavior
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
    #[tool(description = "TREE INITIALIZATION: Create a new probability tree with a root premise and complexity level (1-10). This is the mandatory first step that clears any existing tree and sets the cursor at the root for adding initial child branches. Use complexity 1-3 for simple analysis, 4-7 for balanced analysis, 8-10 for complex multi-layered analysis. After creation, use add_leaf to add initial branches to the root.")]
    async fn create_tree(&self, Parameters(request): Parameters<CreateTreeRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        match service.create_tree(request.premise, request.complexity).await {
            Ok(tree_id) => Ok(format!("Successfully created probability tree with ID: {}", tree_id)),
            Err(e) => Err(ErrorData::new(ErrorCode::RESOURCE_NOT_FOUND, format!("Failed to create tree: {}", e), None)),
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
    /// - `confidence` (i64): Confidence level from 1-10 indicating assessment certainty
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
    #[tool(description = "BRANCH CREATION: Add a new child node to the current cursor position in the tree. This requires a premise (the branch statement), detailed reasoning (explanation/evidence), probability (0.0-1.0), and confidence level (1-10). The cursor is automatically positioned by create_tree (at root) or expand_leaf (at expanded node). Use this after create_tree to add root's children, or after expand_leaf to add children to the expanded node.")]
    async fn add_leaf(&self, Parameters(request): Parameters<AddLeafRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        match service.add_leaf(
            request.premise,
            request.reasoning,
            request.probability,
            request.confidence,
        ).await {
            Ok(node_id) => Ok(format!("Successfully added leaf node with ID: {}", node_id)),
            Err(e) => Err(ErrorData::new(ErrorCode::RESOURCE_NOT_FOUND, format!("Failed to add leaf: {}", e), None)),
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
    #[tool(description = "CURSOR POSITIONING: Expand a specific leaf node to prepare it for adding children. This moves the internal cursor to the specified node_id and updates its reasoning. After expansion, use add_leaf to add child nodes to this expanded node. Essential for building tree depth - always expand a node before adding its children. Provide the exact node_id from previous tool responses and detailed rationale for the expansion.")]
    async fn expand_leaf(&self, Parameters(request): Parameters<ExpandLeafRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        let node_id = request.node_id.clone();
        match service.expand_leaf(request.node_id, request.rationale).await {
            Ok(_result) => Ok(format!("Successfully expanded leaf. Now working in {}.", node_id)),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to expand leaf: {}", e), None)),
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
    #[tool(description = "TREE OPTIMIZATION: Remove low-probability branches to focus on viable scenarios. Aggressiveness 0.0-1.0 controls how many branches to remove (0.0=conservative, 0.5=balanced, 1.0=aggressive). Use this after building the full tree but before final analysis to eliminate noise and focus on meaningful probability paths. Maintains structural integrity and updates all relationships.")]
    async fn prune_tree(&self, Parameters(request): Parameters<PruneTreeRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        let aggressiveness = request.aggressiveness.unwrap_or(0.5);

        match service.prune_tree(aggressiveness).await {
            Ok(result) => Ok(format!("Pruned {} nodes, preserved {} nodes with aggressiveness level {:.2}",
                result.statistics.removed_count,
                result.statistics.preserved_count,
                result.statistics.aggressiveness_level)),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to prune tree: {}", e), None)),
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
    #[tool(description = "FINAL ANALYSIS OUTPUT: Generate comprehensive analysis report of all viable probability paths with integrated insights. Choose narrative style: 'Analytical' (technical/data-focused), 'Strategic' (business/decision-focused), or 'Storytelling' (engaging/narrative). Provide minimum 3 user insights to integrate and overall confidence assessment (0.0-1.0). This is typically the final step after tree building, pruning, and validation.")]
    async fn export_paths(&self, Parameters(request): Parameters<ExportPathsRequest>) -> Result<String, ErrorData> {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        let style = match request.narrative_style.as_str() {
            "Strategic" => NarrativeStyle::Strategic,
            "Storytelling" => NarrativeStyle::Storytelling,
            _ => NarrativeStyle::Analytical,
        };

        match service.export_paths(style, request.insights, request.confidence_assessment).await {
            Ok(result) => Ok(result.to_string()),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to export paths: {}", e), None)),
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
    #[tool(description = "TREE ANALYSIS: Get comprehensive overview of current tree structure including node counts, depth analysis, probability statistics, and complexity metrics. Use this to understand tree development progress, identify structural patterns, and guide further expansion decisions. Essential for monitoring tree health during construction and before major operations like pruning or export.")]
    async fn inspect_tree(&self, Parameters(_request): Parameters<InspectTreeRequest>) -> Result<String, ErrorData> {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        match service.inspect_tree().await {
            Ok(visualization) => Ok(visualization.to_string()),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to inspect tree: {}", e), None)),
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
    #[tool(description = "LOGICAL VALIDATION: Perform comprehensive logical consistency analysis to detect contradictions, reasoning conflicts, and coherence violations. Identifies problematic nodes and provides elimination suggestions. Use this during tree development to ensure logical soundness and before final analysis to guarantee reasoning integrity. Reports pass/fail status with detailed findings.")]
    async fn validate_coherence(&self, Parameters(_request): Parameters<ValidateCoherenceRequest>) -> Result<String, ErrorData> {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        match service.validate_coherence().await {
            Ok(result) => Ok(format!("Coherence validation: {} (coherent: {}, {} contradictions found, {} nodes eliminated)",
                if result.is_coherent { "PASSED" } else { "FAILED" },
                result.is_coherent,
                result.contradictions.len(),
                result.eliminated_nodes.len())),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to validate coherence: {}", e), None)),
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
    #[tool(description = "MATHEMATICAL VALIDATION: Analyze probability values throughout the tree for mathematical consistency. Validates probability ranges [0.0-1.0], confidence levels [1-10], sibling probability relationships, and parent-child consistency. Use this to ensure mathematical soundness before pruning or export. Provides detailed violation reports and correction suggestions.")]
    async fn probability_status(&self, Parameters(_request): Parameters<ProbabilityStatusRequest>) -> Result<String, ErrorData> {
        let service_arc = self.get_service().await;
        let service = service_arc.lock().await;

        match service.probability_status().await {
            Ok(result) => Ok(format!("Probability status: {} (valid: {}, {} violations, {} suggestions)",
                if result.is_valid { "VALID" } else { "INVALID" },
                result.is_valid,
                result.violations.len(),
                result.suggestions.len())),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to get probability status: {}", e), None)),
        }
    }

    /// MCP Tool: Navigates the cursor to a specific node in the probability tree.
    ///
    /// This tool allows explicit navigation to any node in the tree by setting the internal
    /// cursor position. This is useful for manually controlling where new child nodes will
    /// be added via add_leaf, or for positioning yourself at specific points in the tree
    /// for contextual operations.
    ///
    /// # MCP Tool Parameters
    /// - `node_id` (string): ID of the node to navigate to (must be a valid existing node)
    /// - `justification` (string): Reason for navigating to this node (for logging/debugging)
    ///
    /// # Returns
    /// - Success: "Successfully navigated to node {node_id}. Current position: {node_details}"
    /// - Error: "Failed to navigate: {error_description}"
    ///
    /// # Navigation Context
    /// - After navigation, add_leaf will add children to the target node
    /// - This gives manual control over cursor positioning beyond expand_leaf
    /// - Useful for building non-linear tree structures
    /// - All subsequent contextual operations use this node as reference
    ///
    /// # Example MCP Request
    /// ```json
    /// {
    ///   "method": "tools/call",
    ///   "params": {
    ///     "name": "navigate_to",
    ///     "arguments": {
    ///       "node_id": "node:xyz789",
    ///       "justification": "Want to add more children to this specific branch"
    ///     }
    ///   }
    /// }
    /// ```
    #[tool(description = "MANUAL CURSOR CONTROL: Explicitly set the cursor to any existing node for manual control over where add_leaf will place new children. Useful for non-linear tree building, returning to previous branches, or precise cursor positioning beyond automatic expand_leaf behavior. Provide exact node_id and justification. After navigation, add_leaf adds children to the target node.")]
    async fn navigate_to(&self, Parameters(request): Parameters<NavigateToRequest>) -> Result<String, ErrorData> {
        let service = self.get_service().await;
        let mut service = service.lock().await;

        match service.navigate_to(request.node_id.clone()).await {
            Ok(_) => Ok(format!("Successfully navigated to node {}. Ready to add children to this node.", request.node_id)),
            Err(e) => Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("Failed to navigate: {}", e), None)),
        }
    }
}

#[tool_handler]
impl ServerHandler for TreeEngineServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "# Deep Analytics Tree Engine MCP Server\n\n\
                This server provides advanced probabilistic decision tree analysis for complex decision-making scenarios.\n\n\
                ## PRIMARY PURPOSE\n\
                Build and analyze probability trees to evaluate complex decisions by systematically breaking them down into\n\
                logical branches with quantified probabilities, detailed reasoning, and confidence assessments.\n\n\
                ## MANDATORY WORKFLOW SEQUENCE (MUST follow in order):\n\n\
                ### Phase 1: INITIALIZATION (Required First Step)\n\
                1. **create_tree(premise, complexity)** - ALWAYS START HERE\n\
                   - Creates root node and positions cursor at root\n\
                   - Complexity 1-3: Simple binary decisions\n\
                   - Complexity 4-7: Multi-factor analysis (most common)\n\
                   - Complexity 8-10: Complex multi-layered decisions\n\
                   - This CLEARS any existing tree state\n\n\
                ### Phase 2: BUILDING (Core Tree Construction)\n\
                2. **add_leaf(premise, reasoning, probability, confidence)** - Add root's children\n\
                   - IMMEDIATELY after create_tree, add 2-4 initial branches to root\n\
                   - Each branch represents a major pathway/outcome\n\
                   - Probability: 0.0-1.0 (siblings should roughly sum to 1.0)\n\
                   - Confidence: 1-10 (higher = more certain about assessment)\n\
                   - Reasoning must be detailed and specific\n\n\
                3. **expand_leaf(node_id, rationale)** - Prepare node for children\n\
                   - Moves cursor to specific node for deeper analysis\n\
                   - MANDATORY before adding children to any non-root node\n\
                   - Use exact node_id from previous responses\n\
                   - Provide detailed rationale for why this branch needs expansion\n\n\
                4. **REPEAT**: expand_leaf → add_leaf cycles for each level\n\
                   - Build incrementally: root children → expand one → add its children → repeat\n\
                   - Typical depth: 2-4 levels depending on complexity\n\n\
                ### Phase 3: VALIDATION (Quality Assurance)\n\
                5. **inspect_tree()** - Monitor construction progress\n\
                   - Use frequently during building to track structure\n\
                   - Shows node counts, depth, probability distribution\n\n\
                6. **validate_coherence()** - Check logical consistency\n\
                   - Identifies contradictions and reasoning conflicts\n\
                   - MANDATORY before pruning or final analysis\n\n\
                7. **probability_status()** - Validate mathematical consistency\n\
                   - Checks probability ranges and relationships\n\
                   - MANDATORY before pruning or final analysis\n\n\
                ### Phase 4: OPTIMIZATION (Refinement)\n\
                8. **prune_tree(aggressiveness)** - Remove weak branches\n\
                   - 0.0-0.3: Conservative (keep most branches)\n\
                   - 0.4-0.6: Balanced (recommended)\n\
                   - 0.7-1.0: Aggressive (keep only strongest paths)\n\
                   - Use ONLY after validation passes\n\n\
                ### Phase 5: OUTPUT (Final Analysis)\n\
                9. **export_paths(style, insights, confidence)** - Generate final report\n\
                   - 'Analytical': Technical, data-driven presentation\n\
                   - 'Strategic': Business-focused, decision-oriented\n\
                   - 'Storytelling': Narrative, engaging format\n\
                   - Minimum 3 user insights required\n\
                   - Overall confidence: 0.0-1.0\n\n\
                ## CRITICAL RULES (Violations cause errors):\n\
                ⚠️  NEVER skip create_tree - it's mandatory first step\n\
                ⚠️  NEVER add children without expand_leaf (except for root's children)\n\
                ⚠️  ALWAYS save node_ids from responses for expand_leaf\n\
                ⚠️  NEVER prune before validation (coherence + probability checks)\n\
                ⚠️  ALWAYS validate before export\n\n\
                ## CURSOR SYSTEM EXPLANATION:\n\
                - Cursor determines WHERE add_leaf places new nodes\n\
                - create_tree sets cursor to root\n\
                - expand_leaf moves cursor to specified node\n\
                - navigate_to manually positions cursor (advanced use)\n\
                - add_leaf always adds children to current cursor position\n\n\
                ## COMPLETE EXAMPLE SEQUENCE:\n\
                ```\n\
                # 1. Initialize\n\
                create_tree('Should we expand to European market?', 6)\n\
                \n\
                # 2. Add root's children (major pathways)\n\
                add_leaf('Market research positive', 'Customer surveys show 78% interest', 0.7, 8)\n\
                add_leaf('Market research negative', 'Low engagement in focus groups', 0.3, 6)\n\
                \n\
                # 3. Expand first branch for deeper analysis\n\
                expand_leaf('node:abc123', 'Positive research needs breakdown by market segment')\n\
                \n\
                # 4. Add children to expanded node\n\
                add_leaf('B2B segment viable', 'Enterprise customers show strong demand', 0.85, 9)\n\
                add_leaf('B2C segment uncertain', 'Consumer adoption patterns unclear', 0.4, 5)\n\
                \n\
                # 5. Validate construction\n\
                inspect_tree()\n\
                validate_coherence()\n\
                probability_status()\n\
                \n\
                # 6. Optimize\n\
                prune_tree(0.5)\n\
                \n\
                # 7. Generate final analysis\n\
                export_paths('Strategic', ['Risk tolerance is moderate', 'Timeline is 18 months', 'Budget constraints exist'], 0.8)\n\
                ```\n\n\
                ## WHEN TO USE EACH TOOL:\n\
                - **create_tree**: Always first, when starting any new analysis\n\
                - **add_leaf**: After create_tree (for root children) or expand_leaf (for node children)\n\
                - **expand_leaf**: When you want to analyze a branch deeper (add children to it)\n\
                - **inspect_tree**: Frequently during building to monitor progress\n\
                - **validate_coherence**: Before optimization, to check logical consistency\n\
                - **probability_status**: Before optimization, to check mathematical validity\n\
                - **prune_tree**: After validation, to remove weak branches\n\
                - **export_paths**: Final step, to generate analysis report\n\
                - **navigate_to**: Advanced cursor control for non-linear building"
                    .to_string(),
            ),
        }
    }
}
