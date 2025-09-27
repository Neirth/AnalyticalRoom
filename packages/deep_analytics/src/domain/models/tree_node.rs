use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::RecordId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub premise: String,
    pub reasoning: String,
    pub probability: f64,
    pub confidence: i64,
    pub parent_id: Option<RecordId>,
    pub children: Vec<RecordId>,
    pub node_type: NodeType,
    pub is_invalidated: bool,
    pub depth: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeType {
    Root,
    Branch,
    Leaf,
}

impl TreeNode {
    pub fn new_root(premise: String, complexity: i64) -> Self {
        Self {
            id: None,
            premise,
            reasoning: "Root premise of the analysis".to_string(),
            probability: 1.0,
            confidence: 10,
            parent_id: None,
            children: Vec::new(),
            node_type: NodeType::Root,
            is_invalidated: false,
            depth: 0,
            created_at: chrono::Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("complexity".to_string(), complexity.to_string());
                map
            },
        }
    }

    pub fn new_leaf(
        premise: String,
        reasoning: String,
        probability: f64,
        confidence: i64,
        parent_id: RecordId,
        depth: i64,
    ) -> Self {
        Self {
            id: None,
            premise,
            reasoning,
            probability,
            confidence,
            parent_id: Some(parent_id),
            children: Vec::new(),
            node_type: NodeType::Leaf,
            is_invalidated: false,
            depth,
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self.node_type, NodeType::Leaf)
    }

    pub fn can_expand(&self) -> bool {
        self.is_leaf() && !self.is_invalidated
    }

    pub fn expand_to_branch(&mut self) {
        if self.can_expand() {
            self.node_type = NodeType::Branch;
        }
    }

    pub fn add_child(&mut self, child_id: RecordId) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    pub fn remove_child(&mut self, child_id: &RecordId) {
        self.children.retain(|id| id != child_id);
    }

    pub fn invalidate(&mut self) {
        self.is_invalidated = true;
    }

    pub fn get_path_probability(&self, parent_probability: f64) -> f64 {
        parent_probability * self.probability
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_root_creation() {
        let premise = "This is a test premise for root node".to_string();
        let complexity = 5;

        let root = TreeNode::new_root(premise.clone(), complexity);

        assert_eq!(root.premise, premise);
        assert_eq!(root.reasoning, "Root premise of the analysis");
        assert_eq!(root.probability, 1.0);
        assert_eq!(root.confidence, 10);
        assert!(root.parent_id.is_none());
        assert!(root.children.is_empty());
        assert_eq!(root.node_type, NodeType::Root);
        assert!(!root.is_invalidated);
        assert_eq!(root.depth, 0);
        assert_eq!(root.metadata.get("complexity").unwrap(), &complexity.to_string());
    }

    #[test]
    fn test_new_leaf_creation() {
        let premise = "This is a test premise for leaf node".to_string();
        let reasoning = "This is the reasoning for the leaf node".to_string();
        let probability = 0.7;
        let confidence = 8;
        let parent_id: RecordId = "node:test_parent".parse().unwrap();
        let depth = 2;

        let leaf = TreeNode::new_leaf(
            premise.clone(),
            reasoning.clone(),
            probability,
            confidence,
            parent_id.clone(),
            depth,
        );

        assert_eq!(leaf.premise, premise);
        assert_eq!(leaf.reasoning, reasoning);
        assert_eq!(leaf.probability, probability);
        assert_eq!(leaf.confidence, confidence);
        assert_eq!(leaf.parent_id, Some(parent_id));
        assert!(leaf.children.is_empty());
        assert_eq!(leaf.node_type, NodeType::Leaf);
        assert!(!leaf.is_invalidated);
        assert_eq!(leaf.depth, depth);
        assert!(leaf.metadata.is_empty());
    }

    #[test]
    fn test_is_leaf() {
        let mut node = TreeNode::new_root("Test premise".to_string(), 5);
        assert!(!node.is_leaf());

        node.node_type = NodeType::Leaf;
        assert!(node.is_leaf());

        node.node_type = NodeType::Branch;
        assert!(!node.is_leaf());
    }

    #[test]
    fn test_can_expand() {
        let parent_id: RecordId = "node:parent".parse().unwrap();
        let mut leaf = TreeNode::new_leaf(
            "Test premise".to_string(),
            "Test reasoning".to_string(),
            0.5,
            7,
            parent_id,
            1,
        );

        assert!(leaf.can_expand());

        leaf.is_invalidated = true;
        assert!(!leaf.can_expand());

        leaf.is_invalidated = false;
        leaf.node_type = NodeType::Branch;
        assert!(!leaf.can_expand());
    }

    #[test]
    fn test_expand_to_branch() {
        let parent_id: RecordId = "node:parent".parse().unwrap();
        let mut leaf = TreeNode::new_leaf(
            "Test premise".to_string(),
            "Test reasoning".to_string(),
            0.5,
            7,
            parent_id,
            1,
        );

        assert_eq!(leaf.node_type, NodeType::Leaf);

        leaf.expand_to_branch();
        assert_eq!(leaf.node_type, NodeType::Branch);

        // Test that invalidated leaf cannot be expanded
        let mut invalidated_leaf = TreeNode::new_leaf(
            "Test premise".to_string(),
            "Test reasoning".to_string(),
            0.5,
            7,
            "node:parent2".parse().unwrap(),
            1,
        );
        invalidated_leaf.invalidate();
        invalidated_leaf.expand_to_branch();
        assert_eq!(invalidated_leaf.node_type, NodeType::Leaf); // Should remain leaf
    }

    #[test]
    fn test_add_child() {
        let mut root = TreeNode::new_root("Test premise".to_string(), 5);
        let child_id: RecordId = "node:child1".parse().unwrap();

        assert!(root.children.is_empty());

        root.add_child(child_id.clone());
        assert_eq!(root.children.len(), 1);
        assert!(root.children.contains(&child_id));

        // Test adding duplicate
        root.add_child(child_id.clone());
        assert_eq!(root.children.len(), 1); // Should not add duplicate
    }

    #[test]
    fn test_remove_child() {
        let mut root = TreeNode::new_root("Test premise".to_string(), 5);
        let child_id: RecordId = "node:child1".parse().unwrap();

        root.add_child(child_id.clone());
        assert_eq!(root.children.len(), 1);

        root.remove_child(&child_id);
        assert!(root.children.is_empty());

        // Test removing non-existent child
        root.remove_child(&child_id);
        assert!(root.children.is_empty()); // Should not panic
    }

    #[test]
    fn test_invalidate() {
        let mut root = TreeNode::new_root("Test premise".to_string(), 5);
        assert!(!root.is_invalidated);

        root.invalidate();
        assert!(root.is_invalidated);
    }

    #[test]
    fn test_get_path_probability() {
        let parent_id: RecordId = "node:parent".parse().unwrap();
        let leaf = TreeNode::new_leaf(
            "Test premise".to_string(),
            "Test reasoning".to_string(),
            0.6,
            7,
            parent_id,
            1,
        );

        let parent_probability = 0.8;
        let path_probability = leaf.get_path_probability(parent_probability);

        assert!((path_probability - (0.8 * 0.6)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_serialization() {
        let root = TreeNode::new_root("Test premise".to_string(), 5);

        let serialized = serde_json::to_string(&root).unwrap();
        let deserialized: TreeNode = serde_json::from_str(&serialized).unwrap();

        assert_eq!(root.premise, deserialized.premise);
        assert_eq!(root.node_type, deserialized.node_type);
        assert_eq!(root.probability, deserialized.probability);
    }
}