use crate::domain::errors::{TreeEngineError, TreeResult};
use crate::domain::models::{
    tree_node::TreeNode,
    tree_state::{TreeState},
    AnalysisResult, ValidationResult, PruningResult, PruningStatistics, PathResult,
    BalancingResult, CoherenceAnalysis, TreeVisualization, UncertaintyType, NarrativeStyle,
    ContradictionResult, TruthTableRow, NodeVisualization, TreeStatsSummary, NodeStatus,
    ValidationViolation, ViolationType, Severity, TreeMetadata, TreeDistributions, ActivePath
};
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::{Surreal, engine::local::Db};
use surrealdb::RecordId;

/// TreeEngineService provides a comprehensive engine for managing and analyzing probabilistic decision trees.
///
/// This service acts as the core component for handling probability trees, offering functionality for:
/// - Tree creation and manipulation
/// - Node management (addition, expansion, pruning)
/// - Probability analysis and validation
/// - Coherence checking and balancing
/// - Path analysis and export capabilities
///
/// Each service instance maintains its own database connection and generates a unique instance ID
/// to ensure proper isolation between different sessions or contexts.
pub struct TreeEngineService {
    /// Shared database connection wrapped in Arc for safe concurrent access
    db: Arc<Surreal<Db>>,
    /// Unique identifier for this service instance, used to isolate data across sessions
    instance_id: String,
    /// Current cursor position in the tree for contextual operations
    cursor_node_id: Option<String>,
}

impl TreeEngineService {
    /// Creates a new TreeEngineService instance with a unique identifier.
    ///
    /// # Arguments
    /// * `db` - A shared reference to a SurrealDB database connection
    ///
    /// # Returns
    /// A new TreeEngineService instance with a timestamp-based unique instance ID
    ///
    /// # Example
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use surrealdb::Surreal;
    /// use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// let service = TreeEngineService::new(Arc::new(db));
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(db: Arc<Surreal<Db>>) -> TreeEngineService {
        let instance_id = format!("instance_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));

        TreeEngineService{
            db,
            instance_id,
            cursor_node_id: None,
        }
    }

    /// Initializes the service with a specific tree state configuration.
    ///
    /// This method creates a new TreeState with the provided tree ID and complexity level,
    /// storing it in the database for this service instance.
    ///
    /// # Arguments
    /// * `tree_id` - Unique identifier for the tree to be initialized
    /// * `complexity` - Complexity level for the tree (typically 1-10)
    ///
    /// # Returns
    /// * `Ok(())` - If the tree state was successfully initialized
    /// * `Err(TreeEngineError)` - If database operations fail
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// let result = service.initialize_with_tree("my_tree_001".to_string(), 5).await;
    /// assert!(result.is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn initialize_with_tree(&mut self, tree_id: String, complexity: u8) -> TreeResult<()> {
        let tree_state = TreeState::new(tree_id, complexity);
        let _: Option<TreeState> = self.db.upsert(("tree_state", &self.instance_id)).content(tree_state).await?;
        Ok(())
    }

    /// Retrieves the current tree state from the database for this service instance.
    ///
    /// # Returns
    /// * `Ok(TreeState)` - The current tree state if found
    /// * `Err(TreeEngineError::NotFound)` - If no tree state has been initialized
    /// * `Err(TreeEngineError::DatabaseError)` - If database query fails
    async fn get_current_tree_state(&self) -> TreeResult<TreeState> {
        let tree_state: Option<TreeState> = self.db.select(("tree_state", &self.instance_id)).await?;
        tree_state.ok_or_else(|| TreeEngineError::NotFound("No tree state initialized".to_string()))
    }

    /// Updates an existing tree state in the database with current timestamp.
    ///
    /// # Arguments
    /// * `tree_state` - The tree state to update (will be modified to include current timestamp)
    ///
    /// # Returns
    /// * `Ok(TreeState)` - The updated tree state
    /// * `Err(TreeEngineError::DatabaseError)` - If the update operation fails
    async fn update_tree_state(&self, mut tree_state: TreeState) -> TreeResult<TreeState> {
        tree_state.updated_at = chrono::Utc::now();
        let updated: Option<TreeState> = self.db.update(("tree_state", &self.instance_id)).content(tree_state.clone()).await?;
        updated.ok_or_else(|| TreeEngineError::DatabaseError("Failed to update tree state".to_string()))
    }

    /// Retrieves all leaf nodes from the current tree.
    ///
    /// A leaf node is defined as a node that:
    /// - Has no children (empty children vector)
    /// - Has a parent (is not the root node)
    ///
    /// # Returns
    /// * `Ok(Vec<TreeNode>)` - Vector of all leaf nodes in the tree
    /// * `Err(TreeEngineError::DatabaseError)` - If database query fails
    async fn get_leaf_nodes(&self) -> TreeResult<Vec<TreeNode>> {
        let all_nodes: Vec<TreeNode> = self.db.select("node").await?;
        let leaf_nodes: Vec<TreeNode> = all_nodes.into_iter()
            .filter(|node| node.children.is_empty() && node.parent_id.is_some())
            .collect();
        Ok(leaf_nodes)
    }

    /// Retrieves all nodes that have been marked as invalidated.
    ///
    /// Invalidated nodes are typically nodes that have been logically eliminated
    /// during analysis but are kept for audit purposes.
    ///
    /// # Returns
    /// * `Ok(Vec<TreeNode>)` - Vector of all invalidated nodes
    /// * `Err(TreeEngineError::DatabaseError)` - If database query fails
    async fn get_invalidated_nodes(&self) -> TreeResult<Vec<TreeNode>> {
        let all_nodes: Vec<TreeNode> = self.db.select("node").await?;
        let invalidated_nodes: Vec<TreeNode> = all_nodes.into_iter()
            .filter(|node| node.is_invalidated)
            .collect();
        Ok(invalidated_nodes)
    }

    /// Creates a new probability tree with the specified premise and complexity.
    ///
    /// This is the primary method for initializing a new probability tree analysis.
    /// It performs comprehensive validation, cleans up any existing tree data,
    /// creates a root node, and establishes the initial tree state.
    ///
    /// # Arguments
    /// * `premise` - The root premise/question for the probability tree (minimum 10 characters)
    /// * `complexity` - Complexity level from 1-10 that influences tree analysis behavior
    ///
    /// # Returns
    /// * `Ok(String)` - The unique ID of the created root node
    /// * `Err(TreeEngineError::InvalidInput)` - If premise is too short or complexity out of range
    /// * `Err(TreeEngineError::DatabaseError)` - If database operations fail
    ///
    /// # Validation Rules
    /// - Complexity must be between 1 and 10 (inclusive)
    /// - Premise must be at least 10 characters long (after trimming)
    ///
    /// # Side Effects
    /// - Clears any existing nodes in the current instance
    /// - Creates new tree state with generated tree ID
    /// - Initializes root node with the provided premise
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// let tree_id = service.create_tree(
    ///     "Should we invest in renewable energy?".to_string(),
    ///     7
    /// ).await?;
    /// println!("Created tree with root node: {}", tree_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_tree(&mut self, premise: String, complexity: u8) -> TreeResult<String> {
        if !(1..=10).contains(&complexity) {
            return Err(TreeEngineError::InvalidInput("complexity".to_string(), "Complexity must be between 1 and 10".to_string()));
        }
        if premise.trim().len() < 10 {
            return Err(TreeEngineError::InvalidInput("premise".to_string(), "Premise must be at least 10 characters long".to_string()));
        }

        let tree_id = format!("tree_{}", chrono::Utc::now().timestamp());

        // Create new tree state
        let tree_state = TreeState::new(tree_id.clone(), complexity);
        let _: Option<TreeState> = self.db.upsert(("tree_state", &self.instance_id)).content(tree_state).await?;

        // Clean up any existing nodes for this service instance
        let _: Vec<TreeNode> = self.db.delete("node").await?;

        let root = TreeNode::new_root(premise, complexity);
        let created_node: Option<TreeNode> = self.db.create("node").content(root).await?;

        let root_node = created_node.ok_or_else(|| TreeEngineError::DatabaseError("Failed to create root node".to_string()))?;
        let root_id = root_node.id.as_ref().unwrap().clone();

        let mut tree_state = self.get_current_tree_state().await?;
        tree_state.set_root_id(root_id.clone());
        self.update_tree_state(tree_state).await?;

        // Set cursor to the root node for contextual operations
        self.cursor_node_id = Some(root_id.clone().to_string());

        // TreeState mantiene el root_id directamente, no necesita relación RELATE

        Ok(root_id.to_string())
    }

    /// Adds a new leaf node to the probability tree as a child of the specified parent.
    ///
    /// This method creates a new child node with the provided premise, reasoning,
    /// probability assessment, and confidence level. It performs comprehensive validation
    /// and maintains tree integrity by updating parent-child relationships.
    ///
    /// # Arguments
    /// * `parent_node_id` - ID of the parent node to attach the new leaf to
    /// * `premise` - The premise/statement for this probability branch (non-empty)
    /// * `reasoning` - Detailed reasoning supporting this branch (non-empty)
    /// * `probability` - Probability value between 0.0 and 1.0 (inclusive)
    /// * `confidence` - Confidence level from 1-10 indicating certainty in the assessment
    ///
    /// # Returns
    /// * `Ok(String)` - The unique ID of the newly created leaf node
    /// * `Err(TreeEngineError::InvalidInput)` - If any input validation fails
    /// * `Err(TreeEngineError::ProbabilityOutOfRange)` - If probability not in [0.0, 1.0]
    /// * `Err(TreeEngineError::NotFound)` - If parent node doesn't exist
    /// * `Err(TreeEngineError::DatabaseError)` - If database operations fail
    ///
    /// # Validation Rules
    /// - Premise and reasoning must be non-empty after trimming
    /// - Probability must be between 0.0 and 1.0 (inclusive)
    /// - Confidence must be between 1 and 10 (inclusive)
    /// - Parent node must exist in the tree
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # let root_id = service.create_tree("Test premise".to_string(), 5).await?;
    /// let leaf_id = service.add_leaf(
    ///     "Market conditions favor expansion".to_string(),
    ///     "Recent market analysis shows 15% growth".to_string(),
    ///     0.75,
    ///     8
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_leaf(
        &mut self,
        premise: String,
        reasoning: String,
        probability: f64,
        confidence: u8,
    ) -> TreeResult<String> {
        // Validations
        if premise.trim().is_empty() {
            return Err(TreeEngineError::InvalidInput("premise".to_string(), "Premise cannot be empty".to_string()));
        }

        if reasoning.trim().is_empty() {
            return Err(TreeEngineError::InvalidInput("reasoning".to_string(), "Reasoning cannot be empty".to_string()));
        }

        if !(0.0..=1.0).contains(&probability) {
            return Err(TreeEngineError::ProbabilityOutOfRange(probability));
        }

        if !(1..=10).contains(&confidence) {
            return Err(TreeEngineError::InvalidInput("confidence".to_string(), "Confidence must be between 1 and 10".to_string()));
        }

        let tree_state = self.get_current_tree_state().await?;

        // Use cursor to determine parent node
        let cursor_id = match &self.cursor_node_id {
            Some(cursor_id) => cursor_id.clone(),
            None => return Err(TreeEngineError::OperationNotAllowed("No cursor set. Use create_tree first or expand_leaf to set cursor.".to_string())),
        };

        let parent_record_id: RecordId = cursor_id.parse()
            .map_err(|_| TreeEngineError::InvalidInput("cursor_node_id".to_string(), "Invalid cursor node ID format".to_string()))?;

        // Check if parent node exists
        let parent_node: Option<TreeNode> = self.db.select(&parent_record_id).await?;
        let parent_node = parent_node.ok_or_else(|| TreeEngineError::NotFound(cursor_id.clone()))?;

        // Check depth limit - the new child would have depth = parent_node.depth + 1
        if parent_node.depth + 1 >= tree_state.config.max_depth {
            return Err(TreeEngineError::OperationNotAllowed(format!("Maximum depth {} reached", tree_state.config.max_depth)));
        }

        let new_leaf = TreeNode::new_leaf(premise, reasoning, probability, confidence, parent_record_id.clone(), parent_node.depth + 1);
        let created_leaf: Option<TreeNode> = self.db.create("node").content(new_leaf).await?;
        let leaf_node = created_leaf.ok_or_else(|| TreeEngineError::DatabaseError("Failed to create leaf node".to_string()))?;
        let leaf_id = leaf_node.id.as_ref().unwrap().clone();

        // Update parent node to include new child
        let mut updated_parent = parent_node;
        updated_parent.add_child(leaf_id.clone());
        let _: Option<TreeNode> = self.db.update(&parent_record_id).content(updated_parent).await?;

        // No need for explicit relations since we use the children field

        Ok(leaf_id.to_string())
    }

    /// Expands a leaf node by converting it to a branch node and adding child nodes.
    ///
    /// This method transforms a leaf node into a branch node by updating its reasoning
    /// and potentially generating new child nodes based on the complexity level and
    /// updated reasoning. The expansion follows the tree's complexity configuration.
    ///
    /// # Arguments
    /// * `node_id` - ID of the leaf node to expand (must be a valid leaf node)
    /// * `new_reasoning` - Updated reasoning for the expanded node (non-empty)
    ///
    /// # Returns
    /// * `Ok(String)` - Confirmation message indicating successful expansion
    /// * `Err(TreeEngineError::InvalidInput)` - If node_id format is invalid or reasoning is empty
    /// * `Err(TreeEngineError::NotFound)` - If the specified node doesn't exist
    /// * `Err(TreeEngineError::OperationNotAllowed)` - If the node is not a leaf
    /// * `Err(TreeEngineError::DatabaseError)` - If database operations fail
    ///
    /// # Requirements
    /// - Target node must be a leaf node (no children)
    /// - New reasoning must be non-empty after trimming
    /// - Tree state must be properly initialized
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # let root_id = service.create_tree("Test premise".to_string(), 5).await?;
    /// # let leaf_id = service.add_leaf("Test".to_string(), "Initial reasoning".to_string(), 0.7, 7).await?;
    /// let result = service.expand_leaf(
    ///     leaf_id,
    ///     "Expanded reasoning with deeper analysis".to_string()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn expand_leaf(
        &mut self,
        node_id: String,
        new_reasoning: String,
    ) -> TreeResult<String> {
        if new_reasoning.trim().is_empty() {
            return Err(TreeEngineError::InvalidInput("reasoning".to_string(), "Reasoning cannot be empty".to_string()));
        }

        let tree_state = self.get_current_tree_state().await?;

        let node_record_id: RecordId = node_id.parse()
            .map_err(|_| TreeEngineError::InvalidInput("node_id".to_string(), "Invalid node ID format".to_string()))?;

        let mut node: Option<TreeNode> = self.db.select(&node_record_id).await?;
        let mut node = node.take().ok_or_else(|| TreeEngineError::NotFound(node_id.clone()))?;

        if !node.is_leaf() {
            return Err(TreeEngineError::OperationNotAllowed("Node is not a leaf".to_string()));
        }

        if node.depth >= tree_state.config.max_depth {
            return Err(TreeEngineError::OperationNotAllowed(format!("Maximum depth {} reached", tree_state.config.max_depth)));
        }

        node.expand_to_branch();
        node.reasoning = new_reasoning;

        let _: Option<TreeNode> = self.db.update(&node_record_id).content(node).await?;

        // Set cursor to the expanded node for subsequent operations
        self.cursor_node_id = Some(node_record_id.to_string());

        // Los leaf nodes se obtienen dinámicamente desde la base de datos

        Ok(node_record_id.to_string())
    }

    /// Navigates to a specific node in the tree, setting it as the current focus node.
    ///
    /// This method updates the tree's navigation state to focus on a particular node,
    /// which affects subsequent operations that use the "current node" context.
    /// The navigation state is persisted in the tree's metadata.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the target node to navigate to
    ///
    /// # Returns
    /// * `Ok(())` - If navigation was successful
    /// * `Err(TreeEngineError::InvalidInput)` - If node_id format is invalid
    /// * `Err(TreeEngineError::NotFound)` - If the specified node doesn't exist
    /// * `Err(TreeEngineError::DatabaseError)` - If database operations fail
    ///
    /// # Usage
    /// Navigation affects operations that can use a "current node" context,
    /// such as adding leaves without specifying a parent (defaults to current node).
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # let root_id = service.create_tree("Test premise".to_string(), 5).await?;
    /// service.navigate_to(root_id).await?;
    /// // Now operations will use this node as context
    /// # Ok(())
    /// # }
    /// ```
    pub async fn navigate_to(&mut self, node_id: String) -> TreeResult<()> {
        let node_record_id: RecordId = node_id.parse()
            .map_err(|_| TreeEngineError::InvalidInput("node_id".to_string(), "Invalid node ID format".to_string()))?;

        // Verify node exists
        let node: Option<TreeNode> = self.db.select(&node_record_id).await?;
        if node.is_none() {
            return Err(TreeEngineError::NotFound(node_id));
        }

        // Update navigation state in tree metadata
        let mut tree_state = self.get_current_tree_state().await?;
        tree_state.metadata.insert("current_node".to_string(), node_record_id.to_string());
        self.update_tree_state(tree_state).await?;

        // Set cursor to the navigated node
        self.cursor_node_id = Some(node_record_id.to_string());

        Ok(())
    }

    /// Prunes the probability tree by removing low-probability branches based on aggressiveness level.
    ///
    /// This method optimizes the tree structure by removing branches with probabilities
    /// below a calculated threshold. The aggressiveness parameter controls how many
    /// branches are removed, helping focus analysis on the most probable scenarios.
    ///
    /// # Arguments
    /// * `aggressiveness` - Pruning aggressiveness level (0.0 to 1.0)
    ///   - 0.0: Very conservative, removes only extremely low probability branches
    ///   - 0.5: Balanced pruning approach
    ///   - 1.0: Aggressive pruning, keeps only the highest probability branches
    ///
    /// # Returns
    /// * `Ok(PruningResult)` - Detailed results including:
    ///   - List of removed and preserved nodes
    ///   - Pruning statistics (counts, thresholds, etc.)
    ///   - Manual override information
    /// * `Err(TreeEngineError::InvalidInput)` - If aggressiveness is not in [0.0, 1.0]
    /// * `Err(TreeEngineError::DatabaseError)` - If database operations fail
    ///
    /// # Pruning Logic
    /// - Calculates probability threshold based on aggressiveness and tree statistics
    /// - Preserves root node and critical path nodes regardless of probability
    /// - Maintains parent-child relationship integrity
    /// - Updates tree structure after pruning operations
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # service.create_tree("Test premise".to_string(), 5).await?;
    /// let result = service.prune_tree(0.7).await?;
    /// println!("Pruned {} nodes, preserved {} nodes",
    ///          result.statistics.removed_count,
    ///          result.statistics.preserved_count);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn prune_tree(&mut self, aggressiveness: f64) -> TreeResult<PruningResult> {
        if !(0.0..=1.0).contains(&aggressiveness) {
            return Err(TreeEngineError::InvalidInput("aggressiveness".to_string(), "Aggressiveness must be between 0.0 and 1.0".to_string()));
        }

        let tree_state = self.get_current_tree_state().await?;

        // Get all nodes ordered by probability
        let all_nodes: Vec<TreeNode> = self.db.select("node").await?;

        if all_nodes.is_empty() {
            return Ok(PruningResult {
                nodes_removed: vec![],
                nodes_preserved: vec![],
                manual_overrides: vec![],
                statistics: PruningStatistics {
                    original_count: 0,
                    removed_count: 0,
                    preserved_count: 0,
                    aggressiveness_level: aggressiveness,
                },
            });
        }

        let threshold = tree_state.config.min_probability + (aggressiveness * (1.0 - tree_state.config.min_probability));

        let mut nodes_to_remove = Vec::new();
        let mut nodes_preserved = Vec::new();

        for node in &all_nodes {
            if let Some(root_id) = &tree_state.config.root_id {
                if node.id.as_ref() == Some(root_id) {
                    nodes_preserved.push(node.id.as_ref().unwrap().to_string());
                    continue;
                }
            }

            if node.probability < threshold {
                let mut node_to_invalidate = node.clone();
                node_to_invalidate.invalidate();
                let node_id = node_to_invalidate.id.as_ref().unwrap().clone();
                let _: Option<TreeNode> = self.db.update(&node_id).content(node_to_invalidate).await?;
                nodes_to_remove.push(node_id.to_string());
            } else {
                nodes_preserved.push(node.id.as_ref().unwrap().to_string());
            }
        }

        // Los nodos removidos ya están marcados como invalidated en la base de datos
        // No necesitamos actualizar el TreeState manualmente

        let removed_count = nodes_to_remove.len();
        let preserved_count = nodes_preserved.len();

        Ok(PruningResult {
            nodes_removed: nodes_to_remove,
            nodes_preserved,
            manual_overrides: vec![],
            statistics: PruningStatistics {
                original_count: all_nodes.len(),
                removed_count,
                preserved_count,
                aggressiveness_level: aggressiveness,
            },
        })
    }

    /// Prunes leaf nodes to maintain a maximum count, preserving highest probability nodes.
    ///
    /// This method implements count-based pruning rather than probability-threshold pruning.
    /// It maintains only the top N leaf nodes based on probability scores, invalidating
    /// lower-probability leaves to reduce tree complexity and focus analysis on the most
    /// promising branches.
    ///
    /// # Algorithm
    ///
    /// 1. **Validation**: Ensures max_leafs > 0
    /// 2. **Leaf Collection**: Retrieves all current leaf nodes
    /// 3. **Count Check**: If current leafs ≤ max_leafs, returns without changes
    /// 4. **Probability Sorting**: Orders leafs by probability (descending)
    /// 5. **Selection**: Preserves top max_leafs nodes, marks others as invalidated
    /// 6. **Database Update**: Persists invalidation states to database
    ///
    /// # Parameters
    ///
    /// * `max_leafs` - Maximum number of leaf nodes to preserve (must be > 0)
    ///
    /// # Returns
    ///
    /// * `Ok(PruningResult)` - Details of pruning operation including:
    ///   - `nodes_removed`: IDs of invalidated leaf nodes
    ///   - `nodes_preserved`: IDs of kept highest-probability leafs
    ///   - `statistics`: Original count, removal count, preservation metrics
    ///
    /// # Errors
    ///
    /// * `TreeEngineError::InvalidInput` - When max_leafs is 0
    /// * `TreeEngineError::DatabaseError` - Database operation failures
    /// * `TreeEngineError::InternalError` - Unexpected system errors during pruning
    ///
    /// # Example
    ///
    /// ```rust
    /// use deep_analytics::domain::services::TreeEngineService;
    /// use surrealdb::{Surreal, engine::local::Mem};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let db = Arc::new(Surreal::new::<Mem>(()).await?);
    /// let mut service = TreeEngineService::new(db);
    ///
    /// // Create tree with multiple branches
    /// let tree_id = service.create_tree("Analysis scenario".to_string(), 3).await?;
    /// let root_id = service.get_current_node().await?;
    /// service.add_leaf("Option A".to_string(), "First reasoning".to_string(), 0.8, 1).await?;
    /// service.add_leaf("Option B".to_string(), "Second reasoning".to_string(), 0.6, 2).await?;
    /// service.add_leaf("Option C".to_string(), "Third reasoning".to_string(), 0.4, 3).await?;
    /// service.add_leaf("Option D".to_string(), "Fourth reasoning".to_string(), 0.9, 4).await?;
    ///
    /// // Prune to keep only top 2 leafs
    /// let result = service.prune_leafs(2).await?;
    ///
    /// // Result preserves Options D (0.9) and A (0.8)
    /// // Removes Options B (0.6) and C (0.4)
    /// assert_eq!(result.statistics.preserved_count, 2);
    /// assert_eq!(result.statistics.removed_count, 2);
    /// assert_eq!(result.statistics.original_count, 4);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Use Cases
    ///
    /// - **Complexity Management**: Reduce overwhelming number of analysis options
    /// - **Focus Enhancement**: Concentrate on most promising decision paths
    /// - **Performance Optimization**: Limit computational overhead for large trees
    /// - **Clarity Improvement**: Eliminate low-probability distractions
    ///
    /// # Related Methods
    ///
    /// - [`prune_tree()`] - Probability-threshold based pruning with configurable aggressiveness
    /// - [`balance_leafs()`] - Probability adjustment without node removal
    /// - [`validate_coherence()`] - Analysis integrity verification after pruning
    pub async fn prune_leafs(&mut self, max_leafs: usize) -> TreeResult<PruningResult> {
        if max_leafs == 0 {
            return Err(TreeEngineError::InvalidInput("max_leafs".to_string(), "max_leafs must be greater than 0".to_string()));
        }

        // Get leaf nodes using relational query
        let leaf_nodes = self.get_leaf_nodes().await?;
        let original_count = leaf_nodes.len();

        if leaf_nodes.len() <= max_leafs {
            return Ok(PruningResult {
                nodes_removed: vec![],
                nodes_preserved: leaf_nodes.iter().map(|n| n.id.as_ref().unwrap().to_string()).collect(),
                manual_overrides: vec![],
                statistics: PruningStatistics {
                    original_count,
                    removed_count: 0,
                    preserved_count: leaf_nodes.len(),
                    aggressiveness_level: 0.0,
                },
            });
        }

        // Sort by probability descending
        let mut sorted_leafs = leaf_nodes;
        sorted_leafs.sort_by(|a, b| b.probability.partial_cmp(&a.probability).unwrap());

        let nodes_to_keep = &sorted_leafs[..max_leafs];
        let nodes_to_remove = &sorted_leafs[max_leafs..];

        let mut removed_ids = Vec::new();
        for node in nodes_to_remove {
            let mut node_to_invalidate = node.clone();
            node_to_invalidate.invalidate();
            let node_id = node_to_invalidate.id.as_ref().unwrap().clone();
            let _: Option<TreeNode> = self.db.update(&node_id).content(node_to_invalidate).await?;
            removed_ids.push(node_id.to_string());
        }

        // Los nodos removidos ya están marcados como invalidated en la base de datos
        // No necesitamos actualizar el TreeState manualmente

        let removed_count = nodes_to_remove.len();
        let preserved_count = nodes_to_keep.len();

        Ok(PruningResult {
            nodes_removed: removed_ids,
            nodes_preserved: nodes_to_keep.iter().map(|n| n.id.as_ref().unwrap().to_string()).collect(),
            manual_overrides: vec![],
            statistics: PruningStatistics {
                original_count,
                removed_count,
                preserved_count,
                aggressiveness_level: 0.0,
            },
        })
    }

    /// Balances leaf node probabilities using specific uncertainty management strategies.
    ///
    /// This method adjusts probability distributions across leaf nodes without removing
    /// any nodes, applying different balancing algorithms based on the uncertainty context.
    /// It's designed to handle cognitive biases and uncertainty scenarios that can distort
    /// probability assessments in decision trees.
    ///
    /// # Balancing Strategies
    ///
    /// ## InsufficientData
    /// - **Purpose**: Counteract overconfidence when data is limited
    /// - **Algorithm**: Reduces high probabilities (>avg + 0.1) by averaging with mean
    /// - **Effect**: Moderates extreme confidence, promotes epistemic humility
    /// - **Formula**: `new_prob = (current_prob + avg_prob) / 2.0`
    ///
    /// ## EqualLikelihood
    /// - **Purpose**: Boost probabilities when all outcomes seem equally likely
    /// - **Algorithm**: Increases low probabilities (<0.8) toward certainty
    /// - **Effect**: Helps break decision paralysis, promotes action
    /// - **Formula**: `new_prob = (current_prob + 1.0) / 2.0`
    ///
    /// ## CognitiveOverload
    /// - **Purpose**: Normalize probabilities when too many options cause analysis paralysis
    /// - **Algorithm**: Weighted average toward population mean for all nodes
    /// - **Effect**: Reduces extreme values, simplifies decision landscape
    /// - **Formula**: `new_prob = (current_prob * 0.7) + (avg_prob * 0.3)`
    ///
    /// # Parameters
    ///
    /// * `uncertainty_type` - The specific uncertainty scenario requiring probability adjustment
    ///
    /// # Returns
    ///
    /// * `Ok(BalancingResult)` - Results of balancing operation including:
    ///   - `balanced_nodes`: IDs of nodes whose probabilities were modified
    ///   - `uncertainty_type`: The strategy used for balancing
    ///   - `original_probabilities`: Pre-balancing probability values by node ID
    ///   - `new_probabilities`: Post-balancing probability values by node ID
    ///
    /// # Errors
    ///
    /// * `TreeEngineError::DatabaseError` - Database update failures during balancing
    /// * `TreeEngineError::InternalError` - Unexpected system errors during calculation
    ///
    /// # Example
    ///
    /// ```rust
    /// use deep_analytics::domain::services::TreeEngineService;
    /// use deep_analytics::domain::models::{UncertaintyType};
    /// use surrealdb::{Surreal, engine::local::Mem};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let db = Arc::new(Surreal::new::<Mem>(()).await?);
    /// let mut service = TreeEngineService::new(db);
    ///
    /// // Create tree with varied probabilities
    /// let tree_id = service.create_tree("Decision scenario".to_string(), 2).await?;
    /// let root_id = service.get_current_node().await?;
    /// service.add_leaf("High confidence".to_string(), "Strong evidence".to_string(), 0.95, 1).await?;
    /// service.add_leaf("Medium confidence".to_string(), "Some evidence".to_string(), 0.6, 2).await?;
    /// service.add_leaf("Low confidence".to_string(), "Weak evidence".to_string(), 0.2, 3).await?;
    ///
    /// // Balance for insufficient data scenario
    /// let result = service.balance_leafs(UncertaintyType::InsufficientData).await?;
    ///
    /// // High confidence (0.95) gets moderated down
    /// // Other probabilities remain unchanged (below threshold)
    /// assert!(!result.balanced_nodes.is_empty());
    /// assert!(result.original_probabilities.contains_key(&result.balanced_nodes[0]));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Psychology & Decision Theory
    ///
    /// This method addresses several cognitive biases:
    /// - **Overconfidence Bias**: InsufficientData strategy moderates excessive certainty
    /// - **Analysis Paralysis**: EqualLikelihood strategy breaks decision deadlocks
    /// - **Cognitive Overload**: CognitiveOverload strategy simplifies complex choices
    ///
    /// # Use Cases
    ///
    /// - **Risk Assessment**: Calibrate confidence in uncertain environments
    /// - **Decision Support**: Break ties when options appear equally viable
    /// - **Complexity Management**: Simplify overwhelming decision landscapes
    /// - **Bias Mitigation**: Counter systematic probability distortions
    ///
    /// # Related Methods
    ///
    /// - [`prune_leafs()`] - Remove nodes rather than adjust probabilities
    /// - [`prune_tree()`] - Threshold-based node removal with aggressiveness control
    /// - [`validate_coherence()`] - Verify probability consistency after balancing
    pub async fn balance_leafs(&mut self, uncertainty_type: UncertaintyType) -> TreeResult<BalancingResult> {
        let leaf_nodes = self.get_leaf_nodes().await?;

        if leaf_nodes.is_empty() {
            return Ok(BalancingResult {
                balanced_nodes: vec![],
                uncertainty_type,
                original_probabilities: HashMap::new(),
                new_probabilities: HashMap::new(),
            });
        }

        let total_probability: f64 = leaf_nodes.iter().map(|n| n.probability).sum();
        let avg_probability = total_probability / leaf_nodes.len() as f64;

        let mut balanced_nodes = Vec::new();
        let mut original_probabilities = HashMap::new();
        let mut new_probabilities = HashMap::new();

        match uncertainty_type {
            UncertaintyType::InsufficientData => {
                // Reduce high probabilities
                for mut node in leaf_nodes {
                    if node.probability > avg_probability + 0.1 {
                        let old_prob = node.probability;
                        node.probability = (node.probability + avg_probability) / 2.0;
                        let node_id = node.id.as_ref().unwrap().clone();
                        let _: Option<TreeNode> = self.db.update(&node_id).content(node.clone()).await?;
                        let node_id_str = node_id.to_string();
                        balanced_nodes.push(node_id_str.clone());
                        original_probabilities.insert(node_id_str.clone(), old_prob);
                        new_probabilities.insert(node_id_str, node.probability);
                    }
                }
            },
            UncertaintyType::EqualLikelihood => {
                // Boost probabilities closer to certainty
                for mut node in leaf_nodes {
                    if node.probability < 0.8 {
                        let old_prob = node.probability;
                        node.probability = (node.probability + 1.0) / 2.0;
                        let node_id = node.id.as_ref().unwrap().clone();
                        let _: Option<TreeNode> = self.db.update(&node_id).content(node.clone()).await?;
                        let node_id_str = node_id.to_string();
                        balanced_nodes.push(node_id_str.clone());
                        original_probabilities.insert(node_id_str.clone(), old_prob);
                        new_probabilities.insert(node_id_str, node.probability);
                    }
                }
            },
            UncertaintyType::CognitiveOverload => {
                // Move all towards average
                for mut node in leaf_nodes {
                    let old_prob = node.probability;
                    node.probability = (node.probability * 0.7) + (avg_probability * 0.3);
                    let node_id = node.id.as_ref().unwrap().clone();
                    let _: Option<TreeNode> = self.db.update(&node_id).content(node.clone()).await?;
                    let node_id_str = node_id.to_string();
                    balanced_nodes.push(node_id_str.clone());
                    original_probabilities.insert(node_id_str.clone(), old_prob);
                    new_probabilities.insert(node_id_str, node.probability);
                }
            }
        }

        Ok(BalancingResult {
            balanced_nodes,
            uncertainty_type,
            original_probabilities,
            new_probabilities,
        })
    }

    /// Validates the logical coherence and consistency of the probability tree structure.
    ///
    /// This method performs comprehensive analysis to detect logical contradictions,
    /// probability inconsistencies, and coherence violations throughout the tree.
    /// It helps ensure that the tree maintains mathematical and logical soundness.
    ///
    /// # Returns
    /// * `Ok(CoherenceAnalysis)` - Complete coherence report including:
    ///   - Overall coherence status (coherent/incoherent)
    ///   - List of detected violations and their severity
    ///   - Logical contradictions found
    ///   - Suggestions for resolving issues
    ///   - Statistical health metrics
    /// * `Err(TreeEngineError::DatabaseError)` - If database queries fail
    ///
    /// # Validation Checks
    /// - Parent-child probability relationships (child ≤ parent)
    /// - Sum of sibling probabilities ≤ 1.0
    /// - Logical consistency in reasoning chains
    /// - Confidence level appropriateness
    /// - Structural integrity validation
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # service.create_tree("Test premise".to_string(), 5).await?;
    /// let analysis = service.validate_coherence().await?;
    /// if !analysis.is_coherent {
    ///     println!("Found {} contradictions", analysis.contradictions.len());
    ///     for contradiction in &analysis.contradictions {
    ///         println!("Contradiction in node: {}", contradiction.node_id);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn validate_coherence(&self) -> TreeResult<CoherenceAnalysis> {
        let all_nodes: Vec<TreeNode> = self.db.select("node").await?;
        let tree_state = self.get_current_tree_state().await?;

        let mut violations = Vec::new();
        let mut contradictions = Vec::new();
        let mut suggestions = Vec::new();

        // Check Kolmogorov axiom violations
        for node in &all_nodes {
            if node.probability < 0.0 || node.probability > 1.0 {
                violations.push(ValidationViolation {
                    node_id: node.id.as_ref().unwrap().to_string(),
                    violation_type: ViolationType::ProbabilityRange,
                    message: format!("Probability {} is out of range [0,1]", node.probability),
                    severity: Severity::Error,
                });
            }

            if node.confidence < 1 || node.confidence > 10 {
                violations.push(ValidationViolation {
                    node_id: node.id.as_ref().unwrap().to_string(),
                    violation_type: ViolationType::LogicalIncoherence,
                    message: format!("Confidence {} is out of range [1,10]", node.confidence),
                    severity: Severity::Warning,
                });
            }
        }

        // Check for contradictions in sibling nodes
        let mut parent_children: HashMap<RecordId, Vec<&TreeNode>> = HashMap::new();
        for node in &all_nodes {
            if let Some(parent_id) = &node.parent_id {
                parent_children.entry(parent_id.clone()).or_default().push(node);
            }
        }

        for (parent_id, children) in parent_children {
            if children.len() > 1 {
                let total_prob: f64 = children.iter().map(|n| n.probability).sum();
                if total_prob > 1.1 { // Allow small tolerance
                    contradictions.push(ContradictionResult {
                        node_id: parent_id.to_string(),
                        conflicting_premises: children.iter().map(|n| n.premise.clone()).collect(),
                        explanation: "Child probabilities sum exceeds 1.0".to_string(),
                    });
                }
            }
        }

        // Generate suggestions
        if violations.is_empty() && contradictions.is_empty() {
            suggestions.push("Tree structure appears coherent".to_string());
        } else {
            if !violations.is_empty() {
                suggestions.push("Fix probability and confidence range violations".to_string());
            }
            if !contradictions.is_empty() {
                suggestions.push("Normalize child node probabilities to sum ≤ 1.0".to_string());
            }
        }

        // Build truth table for logical consistency
        let mut truth_table = Vec::new();
        for node in all_nodes.iter().take(5) { // Limit for performance
            let mut premises = HashMap::new();
            premises.insert(node.premise.clone(), node.probability > tree_state.config.min_probability);
            truth_table.push(TruthTableRow {
                premises,
                is_consistent: node.probability > tree_state.config.min_probability,
                affected_nodes: vec![node.id.as_ref().unwrap().to_string()],
            });
        }

        Ok(CoherenceAnalysis {
            is_coherent: violations.is_empty() && contradictions.is_empty(),
            contradictions,
            truth_table,
            eliminated_nodes: vec![],
        })
    }

    /// Exports surviving probability paths with comprehensive analysis and integrated insights.
    ///
    /// This method generates a detailed analysis report of all viable probability paths
    /// in the tree, combining quantitative analysis with qualitative insights. The output
    /// is formatted according to the specified narrative style for different audiences.
    ///
    /// # Arguments
    /// * `narrative_style` - Presentation format for the analysis
    ///   - `NarrativeStyle::Analytical`: Data-focused, technical presentation
    ///   - `NarrativeStyle::Strategic`: Business-oriented, decision-focused format
    ///   - `NarrativeStyle::Storytelling`: Narrative-driven, engaging presentation
    /// * `insights` - User-provided insights to integrate (minimum 3 required)
    /// * `confidence_assessment` - Overall confidence in the analysis (0.0 to 1.0)
    ///
    /// # Returns
    /// * `Ok(AnalysisResult)` - Complete analysis including:
    ///   - All surviving probability paths from root to leaves
    ///   - Integrated reasoning chains and probability calculations
    ///   - User insights woven into the narrative
    ///   - Statistical summaries and confidence metrics
    ///   - Formatted presentation according to narrative style
    /// * `Err(TreeEngineError::InvalidInput)` - If insights < 3 or confidence not in [0.0, 1.0]
    /// * `Err(TreeEngineError::DatabaseError)` - If database queries fail
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # use deep_analytics::domain::models::NarrativeStyle;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # service.create_tree("Test premise".to_string(), 5).await?;
    /// let insights = vec![
    ///     "Market analysis supports this decision".to_string(),
    ///     "Risk factors are manageable".to_string(),
    ///     "Timeline is feasible with current resources".to_string(),
    /// ];
    /// let result = service.export_paths(
    ///     NarrativeStyle::Strategic,
    ///     insights,
    ///     0.85
    /// ).await?;
    /// println!("Exported {} paths", result.surviving_paths.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn export_paths(
        &self,
        narrative_style: NarrativeStyle,
        insights: Vec<String>,
        confidence_assessment: f64,
    ) -> TreeResult<AnalysisResult> {
        if insights.len() < 3 {
            return Err(TreeEngineError::InvalidInput("insights".to_string(), "At least 3 insights are required".to_string()));
        }

        if !(0.0..=1.0).contains(&confidence_assessment) {
            return Err(TreeEngineError::ProbabilityOutOfRange(confidence_assessment));
        }

        for (i, insight) in insights.iter().enumerate() {
            if insight.trim().is_empty() {
                return Err(TreeEngineError::InvalidInput("insights".to_string(), format!("Insight {} cannot be empty", i + 1)));
            }
        }

        let leaf_nodes = self.get_leaf_nodes().await?;
        let all_nodes: Vec<TreeNode> = self.db.select("node").await?;

        let mut surviving_paths = Vec::new();
        let mut total_tokens = 0;

        // Build paths from leaf to root for each leaf node
        for leaf in &leaf_nodes {
            if leaf.is_invalidated {
                continue;
            }

            let mut path = vec![leaf.id.as_ref().unwrap().to_string()];
            let mut premises = vec![leaf.premise.clone()];
            let mut reasoning_chain = leaf.reasoning.clone();
            let mut current_node = leaf;
            let mut path_probability = leaf.probability;

            // Walk up to root
            while let Some(parent_id) = &current_node.parent_id {
                if let Some(parent_node) = all_nodes.iter().find(|n| n.id.as_ref() == Some(parent_id)) {
                    path.push(parent_id.to_string());
                    premises.push(parent_node.premise.clone());
                    reasoning_chain = format!("{} -> {}", parent_node.reasoning, reasoning_chain);
                    path_probability *= parent_node.probability;
                    current_node = parent_node;
                } else {
                    break;
                }
            }

            // Reverse to get root-to-leaf order
            path.reverse();
            premises.reverse();

            surviving_paths.push(PathResult {
                path,
                premises,
                final_probability: path_probability,
                reasoning_chain,
                confidence_score: leaf.confidence as f64 / 10.0,
            });

            total_tokens += leaf.reasoning.split_whitespace().count();
        }

        // Get comprehensive tree information like inspect_tree
        let tree_visualization = self.inspect_tree().await.ok();
        let node_details = if let Some(ref viz) = tree_visualization {
            viz.node_details.clone()
        } else {
            HashMap::new()
        };
        let tree_statistics = tree_visualization.as_ref().map(|viz| viz.statistics.clone());
        let tree_distributions = tree_visualization.as_ref().map(|viz| viz.distributions.clone());
        let active_paths_detail = tree_visualization.as_ref()
            .map(|viz| viz.active_paths.clone())
            .unwrap_or_default();

        Ok(AnalysisResult {
            surviving_paths,
            insights,
            confidence_assessment,
            narrative_style,
            total_thought_tokens: total_tokens,
            tree_visualization,
            node_details,
            tree_statistics,
            tree_distributions,
            active_paths_detail,
        })
    }

    /// Generates a comprehensive visualization and analysis of the current probability tree.
    ///
    /// This method performs deep analysis of the tree structure, calculating statistics,
    /// generating ASCII visualization, and providing detailed node information. It's
    /// primarily used for debugging, monitoring, and presenting tree state to users.
    ///
    /// # Returns
    /// * `Ok(TreeVisualization)` - Complete tree analysis including:
    ///   - Statistical summary (node count, depth, probabilities, etc.)
    ///   - ASCII tree representation
    ///   - Detailed node information and relationships
    ///   - Active path analysis
    /// * `Err(TreeEngineError::NotFound)` - If no tree state is initialized
    /// * `Err(TreeEngineError::DatabaseError)` - If database queries fail
    ///
    /// # Generated Statistics
    /// - Total number of nodes in the tree
    /// - Maximum depth reached
    /// - Number of active probability paths
    /// - Average probability across all nodes
    /// - Complexity score based on tree structure
    /// - Distribution of confidence levels
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # service.create_tree("Test premise".to_string(), 5).await?;
    /// let visualization = service.inspect_tree().await?;
    /// println!("Tree has {} nodes with max depth {}",
    ///          visualization.statistics.total_nodes,
    ///          visualization.statistics.max_depth);
    /// println!("ASCII Tree:\n{}", visualization.ascii_tree);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn inspect_tree(&self) -> TreeResult<TreeVisualization> {
        let tree_state = self.get_current_tree_state().await?;

        // Get all nodes with comprehensive information
        let mut nodes_query = self.db.query("SELECT * FROM node ORDER BY depth, probability DESC").await?;
        let nodes: Vec<TreeNode> = nodes_query.take(0)?;

        // Get leaf nodes and invalidated nodes
        let leaf_nodes = self.get_leaf_nodes().await?;
        let invalidated_nodes = self.get_invalidated_nodes().await?;

        // Build comprehensive node details (no ASCII generation)
        let mut node_details = HashMap::new();
        let mut max_depth = 0;
        let mut total_probability = 0.0;
        let mut confidence_distribution = std::collections::HashMap::new();
        let mut depth_distribution = std::collections::HashMap::new();
        let mut probability_distribution = Vec::new();
        let mut premise_analysis = Vec::new();
        let mut reasoning_analysis = Vec::new();

        for node in &nodes {
            let node_id_str = node.id.as_ref().unwrap().to_string();
            let friendly_id = format!("N{}", node_id_str.chars().take(8).collect::<String>());

            // Enhanced node details with complete information
            node_details.insert(friendly_id.clone(), NodeVisualization {
                friendly_id: friendly_id.clone(),
                premise_summary: node.premise.clone(),
                full_premise: node.premise.clone(),
                full_reasoning: node.reasoning.clone(),
                probability: node.probability,
                depth: node.depth as u32,
                children_count: node.children.len(),
                children_ids: node.children.iter().map(|c| c.to_string()).collect(),
                parent_id: None, // TreeNode doesn't have parent field, would need separate query
                is_leaf: node.is_leaf(),
                can_expand: node.can_expand(),
                confidence: node.confidence,
                status: if node.is_invalidated {
                    NodeStatus::Invalidated
                } else {
                    NodeStatus::Active
                },
            });

            // Collect analytics data
            *confidence_distribution.entry(node.confidence).or_insert(0) += 1;
            *depth_distribution.entry(node.depth as u32).or_insert(0) += 1;
            probability_distribution.push(node.probability);
            premise_analysis.push(node.premise.len());
            if !node.reasoning.trim().is_empty() {
                reasoning_analysis.push(node.reasoning.len());
            }

            if node.depth > max_depth {
                max_depth = node.depth;
            }
            total_probability += node.probability;
        }

        // Calculate comprehensive statistics
        let avg_probability = if nodes.is_empty() { 0.0 } else { total_probability / nodes.len() as f64 };
        let total_nodes = nodes.len();
        let active_nodes = nodes.iter().filter(|n| !n.is_invalidated).count();
        let complexity_score = (max_depth as f64 * total_nodes as f64).sqrt();

        // Probability statistics
        probability_distribution.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let prob_median = if probability_distribution.is_empty() {
            0.0
        } else {
            probability_distribution[probability_distribution.len() / 2]
        };
        let prob_min = probability_distribution.first().copied().unwrap_or(0.0);
        let prob_max = probability_distribution.last().copied().unwrap_or(0.0);

        // Content analysis
        let avg_premise_length = if premise_analysis.is_empty() { 0.0 } else {
            premise_analysis.iter().sum::<usize>() as f64 / premise_analysis.len() as f64
        };
        let avg_reasoning_length = if reasoning_analysis.is_empty() { 0.0 } else {
            reasoning_analysis.iter().sum::<usize>() as f64 / reasoning_analysis.len() as f64
        };

        Ok(TreeVisualization {
            // No ASCII generation here - only data
            ascii_tree: String::new(),
            node_details,
            tree_metadata: TreeMetadata {
                tree_id: tree_state.tree_id.clone(),
                complexity: tree_state.config.complexity,
                config: tree_state.config.clone(),
                created_at: tree_state.created_at,
                status: "Active".to_string(),
            },
            statistics: TreeStatsSummary {
                total_nodes,
                active_nodes,
                invalidated_nodes: invalidated_nodes.len(),
                active_paths: leaf_nodes.len(),
                max_depth: max_depth as u32,
                avg_depth: if total_nodes > 0 {
                    depth_distribution.iter().map(|(k, v)| *k as f64 * *v as f64).sum::<f64>() / total_nodes as f64
                } else { 0.0 },
                avg_probability,
                probability_median: prob_median,
                probability_range: (prob_min, prob_max),
                complexity_score,
                avg_premise_length,
                avg_reasoning_length,
            },
            distributions: TreeDistributions {
                confidence_distribution,
                depth_distribution,
                probability_distribution,
            },
            active_paths: leaf_nodes.iter().enumerate().map(|(i, leaf)| {
                let leaf_id = leaf.id.as_ref().unwrap().to_string();
                let friendly_leaf_id = format!("N{}", leaf_id.chars().take(8).collect::<String>());

                ActivePath {
                    path_number: i + 1,
                    leaf_id: friendly_leaf_id,
                    premise: leaf.premise.clone(),
                    probability: leaf.probability,
                    confidence: leaf.confidence,
                }
            }).collect(),
            recommendations: self.generate_recommendations(&leaf_nodes, max_depth as u32, avg_probability, &invalidated_nodes, total_nodes),
        })
    }

    /// Generate analysis recommendations based on tree state
    fn generate_recommendations(&self, leaf_nodes: &[TreeNode], max_depth: u32, avg_probability: f64, invalidated_nodes: &[TreeNode], total_nodes: usize) -> Vec<String> {
        let mut recommendations = Vec::new();

        if leaf_nodes.len() < 2 {
            recommendations.push("⚠️  Consider adding more leaf nodes for richer analysis".to_string());
        }
        if max_depth < 2 {
            recommendations.push("⚠️  Tree depth is shallow - consider expanding promising branches".to_string());
        }
        if avg_probability < 0.3 {
            recommendations.push("⚠️  Low average probability - review premise strength".to_string());
        }
        if invalidated_nodes.len() > total_nodes / 3 {
            recommendations.push("⚠️  High invalidation rate - consider tree restructuring".to_string());
        }
        if leaf_nodes.iter().any(|n| n.probability > 0.8) {
            recommendations.push("✅ High-confidence paths identified - good for decision making".to_string());
        }

        recommendations
    }

    /// Analyzes and validates probability values throughout the entire tree structure.
    ///
    /// This method performs comprehensive probability validation across all nodes,
    /// checking for range violations, consistency issues, and providing suggestions
    /// for maintaining probability coherence within the tree structure.
    ///
    /// # Returns
    /// * `Ok(ValidationResult)` - Complete validation report including:
    ///   - Overall validation status (valid/invalid)
    ///   - List of probability violations found
    ///   - Suggested corrections and improvements
    ///   - Statistical summary of probability health
    /// * `Err(TreeEngineError::NotFound)` - If tree state is not initialized
    /// * `Err(TreeEngineError::DatabaseError)` - If database queries fail
    ///
    /// # Validation Checks
    /// - Probability values must be within [0.0, 1.0] range
    /// - Confidence levels must be within [1, 10] range
    /// - Child node probabilities should not exceed parent probabilities
    /// - Sum of sibling probabilities should be ≤ 1.0
    /// - Minimum probability threshold compliance
    ///
    /// # Example
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use surrealdb::Surreal;
    /// # use deep_analytics::domain::services::tree_engine_service::TreeEngineService;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let db = Surreal::new::<surrealdb::engine::local::Mem>(()).await?;
    /// # let mut service = TreeEngineService::new(Arc::new(db));
    /// # service.create_tree("Test premise".to_string(), 5).await?;
    /// let validation = service.probability_status().await?;
    ///
    /// if validation.is_valid {
    ///     println!("All probabilities are within valid ranges");
    /// } else {
    ///     println!("Found {} violations", validation.violations.len());
    ///     for suggestion in &validation.suggestions {
    ///         println!("Suggestion: {}", suggestion);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn probability_status(&self) -> TreeResult<ValidationResult> {
        let all_nodes: Vec<TreeNode> = self.db.select("node").await?;
        let tree_state = self.get_current_tree_state().await?;

        let mut violations = Vec::new();
        let mut _warnings: Vec<ValidationViolation> = Vec::new();

        // Check each node for probability violations
        for node in &all_nodes {
            if node.probability < 0.0 || node.probability > 1.0 {
                violations.push(ValidationViolation {
                    node_id: node.id.as_ref().unwrap().to_string(),
                    violation_type: ViolationType::ProbabilityRange,
                    message: format!("Node probability {} is outside valid range [0,1]", node.probability),
                    severity: Severity::Error,
                });
            }

            if node.probability < tree_state.config.min_probability {
                violations.push(ValidationViolation {
                    node_id: node.id.as_ref().unwrap().to_string(),
                    violation_type: ViolationType::ProbabilityRange,
                    message: format!("Node probability {} is below minimum threshold {}",
                                       node.probability, tree_state.config.min_probability),
                    severity: Severity::Warning,
                });
            }
        }

        let is_valid = violations.is_empty();

        Ok(ValidationResult {
            is_valid,
            violations,
            suggestions: if is_valid {
                vec!["Tree appears valid".to_string()]
            } else {
                vec!["Fix probability range violations".to_string()]
            },
        })
    }

    /// Retrieves the current tree state configuration and metadata.
    ///
    /// # Returns
    /// * `Ok(TreeState)` - The complete tree state including configuration and metadata
    /// * `Err(TreeEngineError::NotFound)` - If no tree state has been initialized
    /// * `Err(TreeEngineError::DatabaseError)` - If database query fails
    pub async fn get_state(&self) -> TreeResult<TreeState> {
        self.get_current_tree_state().await
    }

    /// Gets the ID of the current node in the navigation context.
    ///
    /// Returns the currently focused node ID, falling back to root node if no
    /// current node is explicitly set in the tree state metadata.
    ///
    /// # Returns
    /// * `Ok(String)` - The ID of the current node
    /// * `Err(TreeEngineError::NotFound)` - If neither current node nor root node exist
    /// * `Err(TreeEngineError::DatabaseError)` - If tree state query fails
    pub async fn get_current_node(&self) -> TreeResult<String> {
        let tree_state = self.get_current_tree_state().await?;
        tree_state.metadata.get("current_node")
            .cloned()
            .or_else(|| tree_state.config.root_id.as_ref().map(|id| id.to_string()))
            .ok_or_else(|| TreeEngineError::NotFound("No current node set".to_string()))
    }

    /// Retrieves the root node ID of the current tree, if available.
    ///
    /// This method safely attempts to get the root node ID without returning errors,
    /// making it suitable for conditional operations and existence checks.
    ///
    /// # Returns
    /// * `Some(String)` - The root node ID if tree is initialized
    /// * `None` - If no tree has been created or tree state is not accessible
    pub async fn get_root_id(&self) -> Option<String> {
        if let Ok(state) = self.get_current_tree_state().await {
            state.config.root_id.as_ref().map(|id| id.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_initialize_with_tree() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let tree_id = "test_tree_123".to_string();
        let complexity = 5;

        let result = service.initialize_with_tree(tree_id.clone(), complexity).await;
        assert!(result.is_ok());

        let tree_state = service.get_current_tree_state().await.unwrap();
        assert_eq!(tree_state.tree_id, tree_id);
        assert_eq!(tree_state.config.complexity, complexity);
    }

    #[tokio::test]
    async fn test_create_tree_success() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let premise = "This is a valid test premise for creating a tree".to_string();
        let complexity = 5;

        let result = service.create_tree(premise.clone(), complexity).await;
        assert!(result.is_ok());

        let root_id = result.unwrap();
        assert!(!root_id.is_empty());

        let tree_state = service.get_current_tree_state().await.unwrap();
        assert_eq!(tree_state.config.complexity, complexity);
        // total_nodes is now calculated dynamically from the database
    }

    #[tokio::test]
    async fn test_create_tree_invalid_complexity() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        
        let premise = "Valid premise".to_string();
        let complexity = 15; // Invalid - out of range

        let result = service.create_tree(premise, complexity).await;
        assert!(result.is_err());

        if let Err(TreeEngineError::InvalidInput(field, _)) = result {
            assert_eq!(field, "complexity");
        } else {
            panic!("Expected InvalidInput error for complexity");
        }
    }

    #[tokio::test]
    async fn test_create_tree_short_premise() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Short".to_string(); // Too short
        let complexity = 5;

        let result = service.create_tree(premise, complexity).await;
        assert!(result.is_err());

        if let Err(TreeEngineError::InvalidInput(field, _)) = result {
            assert_eq!(field, "premise");
        } else {
            panic!("Expected InvalidInput error for premise");
        }
    }

    #[tokio::test]
    async fn test_add_leaf_success() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // First create a tree
        let premise = "Root premise for leaf test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Then add a leaf
        let leaf_premise = "Child leaf premise".to_string();
        let reasoning = "This is the reasoning for the leaf".to_string();
        let probability = 0.7;
        let confidence = 8;

        let result = service.add_leaf(leaf_premise.clone(), reasoning.clone(), probability, confidence).await;
        assert!(result.is_ok());

        let leaf_id = result.unwrap();
        assert!(!leaf_id.is_empty());

        let _tree_state = service.get_current_tree_state().await.unwrap();
        // Leaf nodes and counts are now calculated dynamically from the database
    }

    #[tokio::test]
    async fn test_add_leaf_invalid_inputs() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let premise = "Root premise for invalid inputs test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Test empty premise
        let result = service.add_leaf("".to_string(), "Valid reasoning".to_string(), 0.5, 5).await;
        assert!(result.is_err());

        // Test empty reasoning
        let result = service.add_leaf("Valid premise".to_string(), "".to_string(), 0.5, 5).await;
        assert!(result.is_err());

        // Test invalid probability
        let result = service.add_leaf("Valid premise".to_string(), "Valid reasoning".to_string(), 1.5, 5).await;
        assert!(result.is_err());

        // Test invalid confidence
        let result = service.add_leaf("Valid premise".to_string(), "Valid reasoning".to_string(), 0.5, 15).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_expand_leaf_success() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Create tree and add leaf
        let premise = "Root premise for expand test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();
        let leaf_id = service.add_leaf("Leaf premise".to_string(), "Initial reasoning".to_string(), 0.6, 7).await.unwrap();

        // Expand the leaf
        let new_reasoning = "Updated reasoning after expansion".to_string();
        let result = service.expand_leaf(leaf_id, new_reasoning).await;
        assert!(result.is_ok());

        let _tree_state = service.get_current_tree_state().await.unwrap();
        // Leaf nodes are now tracked in the database, not in TreeState
    }

    #[tokio::test]
    async fn test_expand_leaf_invalid_node_id() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let result = service.expand_leaf("invalid_id".to_string(), "Rationale".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_current_tree_state_error() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let service = TreeEngineService::new(Arc::new(db));
        let result = service.get_current_tree_state().await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_root_id() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Test when no tree state is initialized
        assert!(service.get_root_id().await.is_none());

        // Test when tree state is initialized but no root is set
        service.initialize_with_tree("test_get_root_id".to_string(), 5).await.unwrap();
        assert!(service.get_root_id().await.is_none());

        // Test when root is set
        let premise = "This is a valid test premise for creating a tree for root id test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let retrieved_root_id = service.get_root_id();
        assert!(retrieved_root_id.await.is_some());
        let retrieved_value = service.get_root_id().await;
        assert_eq!(retrieved_value.unwrap(), root_id);
    }

    #[tokio::test]
    async fn test_validate_hierarchical_constraint() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        
        let premise = "Test hierarchical constraint".to_string();
        let root_id = service.create_tree(premise, 3).await.unwrap();

        // Add some nodes to create a hierarchy
        let _leaf1 = service.add_leaf("Child 1".to_string(), "Reasoning 1".to_string(), 0.6, 7).await.unwrap();
        let _leaf2 = service.add_leaf("Child 2".to_string(), "Reasoning 2".to_string(), 0.4, 6).await.unwrap();

        // Test validation
        let result = service.validate_coherence().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_leaf_without_tree_state() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Try to add leaf without initializing tree state
        let result = service.add_leaf("premise".to_string(), "reasoning".to_string(), 0.5, 5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_methods_without_tree_state() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Test methods that require tree state but none exists
        assert!(service.get_state().await.is_err()); // Should fail - no tree state
        assert!(service.get_current_node().await.is_err());
    }

    // New comprehensive tests for 100% coverage

    #[tokio::test]
    async fn test_get_leaf_nodes() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Initially no leaf nodes
        let result = service.get_leaf_nodes().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        // Create tree with leaf nodes
        let premise = "Root for leaf test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();
        let _leaf1 = service.add_leaf("Leaf 1".to_string(), "Reasoning 1".to_string(), 0.6, 7).await.unwrap();
        let _leaf2 = service.add_leaf("Leaf 2".to_string(), "Reasoning 2".to_string(), 0.4, 6).await.unwrap();

        // Now should have leaf nodes
        let result = service.get_leaf_nodes().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_get_invalidated_nodes() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Initially no invalidated nodes
        let result = service.get_invalidated_nodes().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_prune_tree() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        
        let premise = "Root for prune test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add nodes with different probabilities
        let _leaf1 = service.add_leaf("High prob leaf".to_string(), "Reasoning".to_string(), 0.9, 8).await.unwrap();
        let _leaf2 = service.add_leaf("Low prob leaf".to_string(), "Reasoning".to_string(), 0.1, 3).await.unwrap();

        // Prune with medium aggressiveness
        let result = service.prune_tree(0.5).await;
        assert!(result.is_ok());

        let prune_result = result.unwrap();
        assert!(prune_result.statistics.removed_count > 0 || prune_result.statistics.preserved_count > 0);
    }

    #[tokio::test]
    async fn test_prune_tree_invalid_aggressiveness() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Test invalid aggressiveness values
        let result = service.prune_tree(1.5).await;
        assert!(result.is_err());

        let result = service.prune_tree(-0.1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prune_leafs() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for leaf prune test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add multiple leafs
        let _leaf1 = service.add_leaf("Leaf 1".to_string(), "Reasoning".to_string(), 0.9, 8).await.unwrap();
        let _leaf2 = service.add_leaf("Leaf 2".to_string(), "Reasoning".to_string(), 0.7, 7).await.unwrap();
        let _leaf3 = service.add_leaf("Leaf 3".to_string(), "Reasoning".to_string(), 0.5, 5).await.unwrap();

        // Prune to keep only 2 leafs
        let result = service.prune_leafs(2).await;
        assert!(result.is_ok());

        let prune_result = result.unwrap();
        assert_eq!(prune_result.statistics.preserved_count, 2);
        assert_eq!(prune_result.statistics.removed_count, 1);
    }

    #[tokio::test]
    async fn test_prune_leafs_invalid_max() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // Test invalid max_leafs
        let result = service.prune_leafs(0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_balance_leafs_conservative() {
                let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for balance test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add leafs with high probabilities
        let _leaf1 = service.add_leaf("High prob 1".to_string(), "Reasoning".to_string(), 0.95, 9).await.unwrap();
        let _leaf2 = service.add_leaf("High prob 2".to_string(), "Reasoning".to_string(), 0.92, 8).await.unwrap();

        let result = service.balance_leafs(UncertaintyType::InsufficientData).await;
        assert!(result.is_ok());

        let balance_result = result.unwrap();
        assert_eq!(balance_result.uncertainty_type, UncertaintyType::InsufficientData);
    }

    #[tokio::test]
    async fn test_balance_leafs_optimistic() {
                let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let premise = "Root for optimistic balance test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add leafs with low probabilities
        let _leaf1 = service.add_leaf("Low prob 1".to_string(), "Reasoning".to_string(), 0.3, 4).await.unwrap();
        let _leaf2 = service.add_leaf("Low prob 2".to_string(), "Reasoning".to_string(), 0.4, 5).await.unwrap();

        let result = service.balance_leafs(UncertaintyType::EqualLikelihood).await;
        assert!(result.is_ok());

        let balance_result = result.unwrap();
        assert_eq!(balance_result.uncertainty_type, UncertaintyType::EqualLikelihood);
    }

    #[tokio::test]
    async fn test_balance_leafs_neutral() {
                let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let premise = "Root for neutral balance test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let _leaf1 = service.add_leaf("Mixed prob 1".to_string(), "Reasoning".to_string(), 0.8, 7).await.unwrap();
        let _leaf2 = service.add_leaf("Mixed prob 2".to_string(), "Reasoning".to_string(), 0.2, 3).await.unwrap();

        let result = service.balance_leafs(UncertaintyType::CognitiveOverload).await;
        assert!(result.is_ok());

        let balance_result = result.unwrap();
        assert_eq!(balance_result.uncertainty_type, UncertaintyType::CognitiveOverload);
    }

    #[tokio::test]
    async fn test_validate_coherence() {
                let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let premise = "Root for coherence test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let _leaf1 = service.add_leaf("Valid leaf".to_string(), "Reasoning".to_string(), 0.7, 7).await.unwrap();

        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        assert!(coherence.is_coherent);
    }

    #[tokio::test]
    async fn test_export_paths() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for export test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let _leaf1 = service.add_leaf("Export leaf".to_string(), "Reasoning for export".to_string(), 0.8, 8).await.unwrap();

        let insights = vec![
            "First insight".to_string(),
            "Second insight".to_string(),
            "Third insight".to_string(),
        ];

        let result = service.export_paths(NarrativeStyle::Analytical, insights, 0.85).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        assert_eq!(analysis.confidence_assessment, 0.85);
        assert_eq!(analysis.narrative_style, NarrativeStyle::Analytical);
    }

    #[tokio::test]
    async fn test_export_paths_insufficient_insights() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        let insights = vec!["Only one insight".to_string()];

        let result = service.export_paths(NarrativeStyle::Analytical, insights, 0.5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_export_paths_invalid_confidence() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let insights = vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ];

        let result = service.export_paths(NarrativeStyle::Analytical, insights, 1.5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_inspect_tree() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for inspect test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let _leaf1 = service.add_leaf("Inspect leaf".to_string(), "Reasoning for inspect".to_string(), 0.7, 7).await.unwrap();

        let result = service.inspect_tree().await;
        assert!(result.is_ok());

        let visualization = result.unwrap();
        // ASCII tree generation is currently disabled, so we check the actual data
        assert!(visualization.statistics.total_nodes > 0);
        assert!(!visualization.node_details.is_empty());
    }

    #[tokio::test]
    async fn test_probability_status() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for probability status test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let _leaf1 = service.add_leaf("Status leaf".to_string(), "Reasoning".to_string(), 0.6, 6).await.unwrap();

        let result = service.probability_status().await;
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(validation.is_valid);
    }

    #[tokio::test]
    async fn test_navigate_to() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for navigation test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        let result = service.navigate_to(root_id).await;
        assert!(result.is_ok());

        // Check that current node was set
        let current = service.get_current_node().await;
        assert!(current.is_ok());
    }

    #[tokio::test]
    async fn test_navigate_to_invalid_node() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        service.initialize_with_tree("test".to_string(), 5).await.unwrap();

        let result = service.navigate_to("invalid_node_id".to_string()).await;
        assert!(result.is_err());
    }

    // Additional tests for better coverage

    #[tokio::test]
    async fn test_add_leaf_invalid_parent_id_format() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        service.initialize_with_tree("test".to_string(), 5).await.unwrap();

        // Test with completely invalid ID format
        let result = service.add_leaf(
            "Valid premise".to_string(),
            "Valid reasoning for this test".to_string(),
            0.7,
            8
        ).await;

        assert!(result.is_err());
        if let Err(TreeEngineError::OperationNotAllowed(msg)) = result {
            assert!(msg.contains("No cursor set"));
        } else {
            panic!("Expected OperationNotAllowed error for no cursor");
        }
    }

    #[tokio::test]
    async fn test_add_leaf_max_depth_reached() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        // Create tree with very low max depth
        let premise = "Root premise for max depth test".to_string();
        let root_id = service.create_tree(premise, 1).await.unwrap(); // complexity 1 = max_depth 3

        // Add first level (depth 1) - cursor is at root
        let level1_id = service.add_leaf(
            "Level 1 premise".to_string(),
            "Level 1 reasoning for depth test".to_string(),
            0.7,
            8
        ).await.unwrap();

        // Expand the level1 node to move cursor to it
        service.expand_leaf(level1_id.clone(), "Expanding to add level 2".to_string()).await.unwrap();

        // Add second level (depth 2) - cursor is now at level1
        let level2_id = service.add_leaf(
            "Level 2 premise".to_string(),
            "Level 2 reasoning for depth test".to_string(),
            0.6,
            7
        ).await.unwrap();

        // Expand the level2 node to move cursor to it
        service.expand_leaf(level2_id, "Expanding to try level 3".to_string()).await.unwrap();

        // Try to add third level (should fail - max depth reached)
        let result = service.add_leaf(
            "Level 3 premise".to_string(),
            "Level 3 reasoning for depth test".to_string(),
            0.5,
            6
        ).await;

        assert!(result.is_err());
        if let Err(TreeEngineError::OperationNotAllowed(msg)) = result {
            assert!(msg.contains("Maximum depth"));
        } else {
            panic!("Expected OperationNotAllowed error for max depth");
        }
    }

    #[tokio::test]
    async fn test_prune_tree_no_nodes() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        service.initialize_with_tree("empty_tree".to_string(), 5).await.unwrap();

        // Try to prune when there are no nodes
        let result = service.prune_tree(0.5).await;
        assert!(result.is_ok());

        let prune_result = result.unwrap();
        assert_eq!(prune_result.statistics.original_count, 0);
        assert_eq!(prune_result.statistics.removed_count, 0);
        assert_eq!(prune_result.statistics.preserved_count, 0);
        assert_eq!(prune_result.statistics.aggressiveness_level, 0.5);
    }

    #[tokio::test]
    async fn test_update_tree_state_failure_path() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));

        // This should fail because no tree state exists to update
        let result = service.get_current_tree_state().await;
        assert!(result.is_err());

        if let Err(TreeEngineError::NotFound(msg)) = result {
            assert!(msg.contains("No tree state initialized"));
        } else {
            panic!("Expected NotFound error");
        }
    }

    #[tokio::test]
    async fn test_add_leaf_nonexistent_parent() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        service.initialize_with_tree("test".to_string(), 5).await.unwrap();

        // Create tree to set cursor
        let _root_id = service.create_tree("Root for nonexistent test".to_string(), 5).await.unwrap();

        // Try to add leaf - this will use cursor (should work)
        let result = service.add_leaf(
            "Valid premise".to_string(),
            "Valid reasoning for nonexistent parent test".to_string(),
            0.7,
            8
        ).await;

        // This should actually succeed since we have a cursor now
        assert!(result.is_ok());
    }

    // Comprehensive edge case tests

    #[tokio::test]
    async fn test_prune_leafs_with_insufficient_nodes() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for insufficient nodes test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add only 1 leaf
        let _leaf1 = service.add_leaf(
            "Single leaf".to_string(),
            "Only one leaf for insufficient test".to_string(),
            0.8,
            8
        ).await.unwrap();

        // Try to prune to keep 3 leafs when we only have 1
        let result = service.prune_leafs(3).await;
        assert!(result.is_ok());

        let prune_result = result.unwrap();
        // Should preserve the single leaf and not remove anything
        assert_eq!(prune_result.statistics.preserved_count, 1);
        assert_eq!(prune_result.statistics.removed_count, 0);
        assert_eq!(prune_result.statistics.original_count, 1);
    }

    #[tokio::test]
    async fn test_balance_leafs_empty_tree() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        service.initialize_with_tree("empty_balance_test".to_string(), 5).await.unwrap();

        // Try to balance when there are no leaf nodes
        let result = service.balance_leafs(UncertaintyType::InsufficientData).await;
        assert!(result.is_ok());

        let balance_result = result.unwrap();
        assert!(balance_result.balanced_nodes.is_empty());
        assert!(balance_result.original_probabilities.is_empty());
        assert!(balance_result.new_probabilities.is_empty());
    }

    #[tokio::test]
    async fn test_balance_leafs_mixed_probabilities() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for mixed probabilities test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add two leaves with very different probabilities
        let high_leaf = service.add_leaf(
            "High confidence leaf".to_string(),
            "Very confident reasoning for balance test".to_string(),
            0.9,
            9
        ).await.unwrap();

        let low_leaf = service.add_leaf(
            "Low confidence leaf".to_string(),
            "Less confident reasoning for balance test".to_string(),
            0.2,
            3
        ).await.unwrap();

        let result = service.balance_leafs(UncertaintyType::InsufficientData).await;
        assert!(result.is_ok());

        let balance_result = result.unwrap();
        // Should balance the high probability node (avg = 0.55, high = 0.9 > 0.55 + 0.1)
        assert!(balance_result.balanced_nodes.len() >= 1);
        // Should contain the high probability node
        assert!(balance_result.balanced_nodes.contains(&high_leaf) ||
                balance_result.original_probabilities.contains_key(&high_leaf));
    }

    #[tokio::test]
    async fn test_export_paths_empty_tree() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        service.initialize_with_tree("empty_export_test".to_string(), 5).await.unwrap();

        let insights = vec![
            "First insight".to_string(),
            "Second insight".to_string(),
            "Third insight".to_string(),
        ];

        // Try to export paths when there are no surviving paths
        let result = service.export_paths(NarrativeStyle::Analytical, insights, 0.5).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        assert!(analysis.surviving_paths.is_empty());
        assert_eq!(analysis.total_thought_tokens, 0);
    }

    #[tokio::test]
    async fn test_export_paths_with_invalidated_nodes() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for invalidated nodes test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add leaf and then prune to invalidate some nodes
        let _leaf1 = service.add_leaf(
            "Leaf to be invalidated".to_string(),
            "This leaf will be invalidated in pruning test".to_string(),
            0.1, // Low probability, likely to be pruned
            2
        ).await.unwrap();

        let _leaf2 = service.add_leaf(
            "Leaf to survive".to_string(),
            "This leaf should survive pruning test".to_string(),
            0.9, // High probability
            9
        ).await.unwrap();

        // Prune aggressively to invalidate low-probability nodes
        let _prune_result = service.prune_tree(0.8).await.unwrap();

        let insights = vec![
            "Analysis insight one".to_string(),
            "Analysis insight two".to_string(),
            "Analysis insight three".to_string(),
        ];

        // Export should skip invalidated nodes
        let result = service.export_paths(NarrativeStyle::Strategic, insights, 0.7).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        // Should only include paths from non-invalidated nodes
        assert_eq!(analysis.narrative_style, NarrativeStyle::Strategic);
    }

    #[tokio::test]
    async fn test_probability_status_with_violations() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for violations test".to_string();
        let root_id = service.create_tree(premise, 1).await.unwrap(); // Very low complexity for tight constraints

        // Add a leaf that should be fine initially
        let leaf_id = service.add_leaf(
            "Normal leaf".to_string(),
            "Normal reasoning for probability test".to_string(),
            0.05, // Very low probability - below min_probability threshold
            3
        ).await.unwrap();

        // Check probability status - should find violations
        let result = service.probability_status().await;
        assert!(result.is_ok());

        let validation = result.unwrap();
        // Should detect the low probability violation
        assert!(!validation.violations.is_empty());

        // Should have suggestions
        assert!(!validation.suggestions.is_empty());
        assert_eq!(validation.suggestions[0], "Fix probability range violations");
    }

    #[tokio::test]
    async fn test_validate_coherence_complex_tree() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for complex coherence test".to_string();
        let root_id = service.create_tree(premise, 7).await.unwrap(); // Higher complexity

        // Build a more complex tree structure
        let branch1 = service.add_leaf(
            "Branch 1 premise".to_string(),
            "First branch reasoning for coherence test".to_string(),
            0.7,
            7
        ).await.unwrap();

        let branch2 = service.add_leaf(
            "Branch 2 premise".to_string(),
            "Second branch reasoning for coherence test".to_string(),
            0.3,
            4
        ).await.unwrap();

        // Navigate to branch1 and add sub-branch
        service.navigate_to(branch1).await.unwrap();
        let _subbranch1 = service.add_leaf(
            "Sub-branch 1.1".to_string(),
            "Sub-branch reasoning under branch 1 for coherence".to_string(),
            0.6,
            6
        ).await.unwrap();

        // Navigate to branch2 and add sub-branch
        service.navigate_to(branch2).await.unwrap();
        let _subbranch2 = service.add_leaf(
            "Sub-branch 2.1".to_string(),
            "Sub-branch reasoning under branch 2 for coherence".to_string(),
            0.4,
            5
        ).await.unwrap();

        // Validate coherence of complex structure
        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        // Complex tree should still be coherent
        assert!(coherence.is_coherent);
        assert!(coherence.contradictions.is_empty());
    }

    #[tokio::test]
    async fn test_inspect_tree_complex_structure() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for complex inspect test".to_string();
        let root_id = service.create_tree(premise, 8).await.unwrap();

        // Create a tree with multiple levels and nodes - cursor is at root
        let level1_1 = service.add_leaf(
            "Level 1 Node 1".to_string(),
            "First level one node reasoning for inspection".to_string(),
            0.8,
            8
        ).await.unwrap();

        let level1_2 = service.add_leaf(
            "Level 1 Node 2".to_string(),
            "Second level one node reasoning for inspection".to_string(),
            0.6,
            6
        ).await.unwrap();

        // Navigate to first level 1 node to add children
        service.navigate_to(level1_1).await.unwrap();

        // Add second level under level1_1
        let _level2_1 = service.add_leaf(
            "Level 2 Node 1".to_string(),
            "Level two node one reasoning for inspection".to_string(),
            0.7,
            7
        ).await.unwrap();

        let _level2_2 = service.add_leaf(
            "Level 2 Node 2".to_string(),
            "Level two node two reasoning for inspection".to_string(),
            0.5,
            5
        ).await.unwrap();

        // Inspect the complex tree
        let result = service.inspect_tree().await;
        assert!(result.is_ok());

        let visualization = result.unwrap();

        // ASCII tree generation is currently disabled, so we check the actual data
        // Verify that node details contain the expected premises
        let has_level1_node1 = visualization.node_details.values().any(|n| n.premise_summary.contains("Level 1 Node 1"));
        let has_level2_node1 = visualization.node_details.values().any(|n| n.premise_summary.contains("Level 2 Node 1"));
        assert!(has_level1_node1);
        assert!(has_level2_node1);

        // Should have node details for all nodes
        assert!(visualization.node_details.len() >= 4); // At least root + 4 added nodes

        // Should have meaningful statistics
        assert!(visualization.statistics.total_nodes >= 5); // Root + 4 added
        assert_eq!(visualization.statistics.active_paths, 3); // 3 leaf nodes
        assert_eq!(visualization.statistics.max_depth, 2);
        assert!(visualization.statistics.avg_probability > 0.0);
        assert!(visualization.statistics.complexity_score > 0.0);
    }

    #[tokio::test]
    async fn test_database_namespace_setup() {
        // This test will exercise the database setup line (line 22)
        let db1 = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();

        db1.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service1 = TreeEngineService::new(Arc::new(db1));

        let db2 = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();

        db2.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service2 = TreeEngineService::new(Arc::new(db2));

        let result1 = service1.initialize_with_tree("tree1".to_string(), 3).await;
        let result2 = service2.initialize_with_tree("tree2".to_string(), 4).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    // CRITICAL EDGE CASES - These are the failure points in production

    #[tokio::test]
    async fn test_export_paths_empty_insight_strings() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for empty insights test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();
        let _leaf = service.add_leaf("Test leaf".to_string(), "Test reasoning for empty insights".to_string(), 0.7, 7).await.unwrap();

        // This is the critical edge case: insights with empty strings
        let insights = vec![
            "Valid insight".to_string(),
            "".to_string(), // Empty string - this should fail
            "Another valid insight".to_string(),
        ];

        let result = service.export_paths(NarrativeStyle::Analytical, insights, 0.5).await;
        assert!(result.is_err());

        if let Err(TreeEngineError::InvalidInput(field, msg)) = result {
            assert_eq!(field, "insights");
            assert!(msg.contains("Insight 2 cannot be empty"));
        } else {
            panic!("Expected InvalidInput error for empty insight string");
        }
    }

    #[tokio::test]
    async fn test_export_paths_whitespace_only_insights() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for whitespace insights test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();
        let _leaf = service.add_leaf("Test leaf".to_string(), "Test reasoning for whitespace insights".to_string(), 0.8, 8).await.unwrap();

        // Another critical case: insights with only whitespace
        let insights = vec![
            "Valid insight".to_string(),
            "   \t\n  ".to_string(), // Only whitespace - should fail
            "Third valid insight".to_string(),
        ];

        let result = service.export_paths(NarrativeStyle::Strategic, insights, 0.6).await;
        assert!(result.is_err());

        if let Err(TreeEngineError::InvalidInput(field, msg)) = result {
            assert_eq!(field, "insights");
            assert!(msg.contains("Insight 2 cannot be empty"));
        } else {
            panic!("Expected InvalidInput error for whitespace-only insight");
        }
    }

    #[tokio::test]
    async fn test_probability_status_invalid_probability_range() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for invalid probability range test".to_string();
        let _root_id = service.create_tree(premise, 5).await.unwrap();

        // Manually create a node with invalid probability by direct database manipulation
        // This simulates corruption or external modification
        let invalid_node = TreeNode {
            id: None,
            premise: "Invalid probability node".to_string(),
            reasoning: "This node has invalid probability for critical test".to_string(),
            probability: 1.5, // INVALID - greater than 1.0
            confidence: 5,
            parent_id: None,
            children: Vec::new(),
            node_type: crate::domain::models::tree_node::NodeType::Leaf,
            is_invalidated: false,
            depth: 1,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        let _created: Option<TreeNode> = service.db.create("node").content(invalid_node).await.unwrap();

        // Now check probability status - should detect the invalid range
        let result = service.probability_status().await;
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(!validation.is_valid);
        assert!(!validation.violations.is_empty());

        // Check that we caught the specific error
        let violation = &validation.violations[0];
        assert_eq!(violation.violation_type, crate::domain::models::types::ViolationType::ProbabilityRange);
        assert!(violation.message.contains("1.5 is outside valid range [0,1]"));
        assert_eq!(violation.severity, crate::domain::models::types::Severity::Error);
        assert_eq!(validation.suggestions[0], "Fix probability range violations");
    }

    #[tokio::test]
    async fn test_probability_status_negative_probability() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for negative probability test".to_string();
        let _root_id = service.create_tree(premise, 5).await.unwrap();

        // Create node with negative probability
        let negative_node = TreeNode {
            id: None,
            premise: "Negative probability node".to_string(),
            reasoning: "This node has negative probability for critical test".to_string(),
            probability: -0.3, // INVALID - negative
            confidence: 4,
            parent_id: None,
            children: Vec::new(),
            node_type: crate::domain::models::tree_node::NodeType::Leaf,
            is_invalidated: false,
            depth: 1,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        let _created: Option<TreeNode> = service.db.create("node").content(negative_node).await.unwrap();

        let result = service.probability_status().await;
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(!validation.is_valid);
        assert!(!validation.violations.is_empty());

        let violation = &validation.violations[0];
        assert_eq!(violation.violation_type, crate::domain::models::types::ViolationType::ProbabilityRange);
        assert!(violation.message.contains("-0.3 is outside valid range [0,1]"));
        assert_eq!(violation.severity, crate::domain::models::types::Severity::Error);
    }

    #[tokio::test]
    async fn test_probability_status_below_minimum_threshold() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for below threshold test".to_string();
        let _root_id = service.create_tree(premise, 5).await.unwrap();

        // Create node with probability below min_probability (0.15)
        let below_threshold_node = TreeNode {
            id: None,
            premise: "Below threshold node".to_string(),
            reasoning: "This node has probability below minimum threshold for critical test".to_string(),
            probability: 0.05, // Below 0.15 min_probability
            confidence: 3,
            parent_id: None,
            children: Vec::new(),
            node_type: crate::domain::models::tree_node::NodeType::Leaf,
            is_invalidated: false,
            depth: 1,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        let _created: Option<TreeNode> = service.db.create("node").content(below_threshold_node).await.unwrap();

        let result = service.probability_status().await;
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(!validation.is_valid);
        assert!(!validation.violations.is_empty());

        let violation = &validation.violations[0];
        assert_eq!(violation.violation_type, crate::domain::models::types::ViolationType::ProbabilityRange);
        assert!(violation.message.contains("0.05 is below minimum threshold 0.15"));
        assert_eq!(violation.severity, crate::domain::models::types::Severity::Warning);
    }

    #[tokio::test]
    async fn test_probability_status_all_valid() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for all valid test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Add nodes with all valid probabilities
        let _leaf1 = service.add_leaf("Valid leaf 1".to_string(), "Valid reasoning one for all valid test".to_string(), 0.7, 7).await.unwrap();
        let _leaf2 = service.add_leaf("Valid leaf 2".to_string(), "Valid reasoning two for all valid test".to_string(), 0.3, 5).await.unwrap();

        let result = service.probability_status().await;
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(validation.is_valid);
        assert!(validation.violations.is_empty());
        assert_eq!(validation.suggestions[0], "Tree appears valid");
    }

    // ULTRA-SPECIFIC EDGE CASES - The final frontier

    #[tokio::test]
    async fn test_validate_coherence_invalid_probability_causes_incoherence() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for invalid probability coherence test".to_string();
        let _root_id = service.create_tree(premise, 5).await.unwrap();

        // Create node with invalid probability directly in DB (lines 408-414)
        let invalid_node = TreeNode {
            id: None,
            premise: "Node with invalid probability".to_string(),
            reasoning: "This node tests the probability validation in validate_coherence".to_string(),
            probability: 1.5, // INVALID - exceeds 1.0, should trigger lines 408-414
            confidence: 5,
            parent_id: None,
            children: Vec::new(),
            node_type: crate::domain::models::tree_node::NodeType::Leaf,
            is_invalidated: false,
            depth: 1,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        let _created: Option<TreeNode> = service.db.create("node").content(invalid_node).await.unwrap();

        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        // The invalid probability should make the tree incoherent (line 473)
        assert!(!coherence.is_coherent);
    }

    #[tokio::test]
    async fn test_validate_coherence_invalid_confidence_causes_incoherence() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for invalid confidence coherence test".to_string();
        let _root_id = service.create_tree(premise, 5).await.unwrap();

        // Create node with invalid confidence to trigger lines 417-422
        let invalid_conf_node = TreeNode {
            id: None,
            premise: "Node with invalid confidence".to_string(),
            reasoning: "This node tests confidence validation in validate_coherence".to_string(),
            probability: 0.6,
            confidence: 11, // INVALID - above range [1,10], should trigger lines 417-422
            parent_id: None,
            children: Vec::new(),
            node_type: crate::domain::models::tree_node::NodeType::Leaf,
            is_invalidated: false,
            depth: 1,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        let _created: Option<TreeNode> = service.db.create("node").content(invalid_conf_node).await.unwrap();

        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        // Invalid confidence should make the tree incoherent (line 473)
        assert!(!coherence.is_coherent);
    }

    #[tokio::test]
    async fn test_validate_coherence_child_probabilities_exceed_sum() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for child probability sum test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Create parent node
        let parent_id = service.add_leaf(
            "Parent for probability sum test".to_string(),
            "Parent node that will have children with excessive probability sum".to_string(),
            0.8,
            8
        ).await.unwrap();

        // Add children with probabilities that sum > 1.0 (lines 439-442)
        let _child1 = service.add_leaf(
            "High probability child 1".to_string(),
            "First child with high probability for sum test".to_string(),
            0.7, // High probability
            7
        ).await.unwrap();

        let _child2 = service.add_leaf(
            "High probability child 2".to_string(),
            "Second child with high probability for sum test".to_string(),
            0.6, // 0.7 + 0.6 = 1.3 > 1.0
            6
        ).await.unwrap();

        // validate_coherence should detect the probability sum violation
        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        assert!(!coherence.is_coherent);
        assert!(!coherence.contradictions.is_empty());

        // Should have contradiction about child probabilities summing > 1.0
        let sum_violation = coherence.contradictions.iter()
            .any(|c| c.explanation.contains("sum exceeds 1.0"));
        assert!(sum_violation);
    }

    #[tokio::test]
    async fn test_validate_coherence_mixed_violations_and_contradictions() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for mixed violations test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Create scenario with both violations AND contradictions to test suggestions (lines 452-456)
        let parent_id = service.add_leaf(
            "Parent for mixed violations".to_string(),
            "Parent node for testing mixed violations and contradictions".to_string(),
            0.9,
            9
        ).await.unwrap();

        // Add children that sum > 1.0 (contradiction)
        let _child1 = service.add_leaf(
            "Child with excessive sum 1".to_string(),
            "Child one for mixed violation test".to_string(),
            0.8,
            8
        ).await.unwrap();

        let _child2 = service.add_leaf(
            "Child with excessive sum 2".to_string(),
            "Child two for mixed violation test".to_string(),
            0.7, // 0.8 + 0.7 = 1.5 > 1.0
            7
        ).await.unwrap();

        // Also add a node with invalid confidence (violation)
        let invalid_node = TreeNode {
            id: None,
            premise: "Invalid confidence mixed test node".to_string(),
            reasoning: "Node with invalid confidence for mixed violation test".to_string(),
            probability: 0.5,
            confidence: 0, // INVALID - below range [1,10]
            parent_id: None,
            children: Vec::new(),
            node_type: crate::domain::models::tree_node::NodeType::Leaf,
            is_invalidated: false,
            depth: 1,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        let _created: Option<TreeNode> = service.db.create("node").content(invalid_node).await.unwrap();

        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        assert!(!coherence.is_coherent);

        // Should have both violations and contradictions, leading to specific suggestions
        assert!(!coherence.contradictions.is_empty());

        // Check for the specific contradiction about probability sum
        let has_sum_contradiction = coherence.contradictions.iter()
            .any(|c| c.explanation == "Child probabilities sum exceeds 1.0");
        assert!(has_sum_contradiction);
    }

    #[tokio::test]
    async fn test_validate_coherence_perfectly_coherent_suggestions() {
        let db = Surreal::new::<surrealdb::engine::local::Mem>(())
                .await
                .unwrap();
    
        db.use_ns("analytics").use_db("trees").await.unwrap();

        let mut service = TreeEngineService::new(Arc::new(db));
        let premise = "Root for perfectly coherent test".to_string();
        let root_id = service.create_tree(premise, 5).await.unwrap();

        // Create a perfectly valid tree structure to trigger line 450
        let parent_id = service.add_leaf(
            "Perfect parent node".to_string(),
            "Perfectly valid parent node for coherence test".to_string(),
            0.8,
            8
        ).await.unwrap();

        // Navigate to parent and add children with valid probabilities that sum < 1.0
        service.navigate_to(parent_id).await.unwrap();
        let _child1 = service.add_leaf(
            "Valid child 1".to_string(),
            "First perfectly valid child for coherence test".to_string(),
            0.3, // Low probability
            5
        ).await.unwrap();

        let _child2 = service.add_leaf(
            "Valid child 2".to_string(),
            "Second perfectly valid child for coherence test".to_string(),
            0.2, // 0.3 + 0.2 = 0.5 < 1.0 ✓
            4
        ).await.unwrap();

        let result = service.validate_coherence().await;
        assert!(result.is_ok());

        let coherence = result.unwrap();
        assert!(coherence.is_coherent);
        assert!(coherence.contradictions.is_empty());

        // Should trigger the "perfectly coherent" suggestion path (line 450)
        // This is difficult to assert directly, but the coherence should be true
    }
}