use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub tree_id: String,
    pub config: TreeConfig,
    pub metadata: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeConfig {
    pub root_id: Option<RecordId>,
    pub max_depth: u8,
    pub min_probability: f64,
    pub branch_limit: usize,
    pub use_laplace: bool,
    pub complexity: u8,
}

impl TreeState {
    pub fn new(tree_id: String, complexity: u8) -> Self {
        Self {
            id: None,
            tree_id,
            config: TreeConfig::new(complexity),
            metadata: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }


    pub fn set_root_id(&mut self, root_id: RecordId) {
        self.config.root_id = Some(root_id);
        self.updated_at = chrono::Utc::now();
    }
}

impl TreeConfig {
    pub fn new(complexity: u8) -> Self {
        let (max_depth, branch_limit) = match complexity {
            1..=2 => (3, 3),
            3..=4 => (4, 4),
            5..=7 => (6, 5),
            _ => (8, 6),
        };

        Self {
            root_id: None,
            max_depth,
            branch_limit,
            min_probability: 0.15,
            use_laplace: true,
            complexity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_state_creation() {
        let tree_id = "test_tree_123".to_string();
        let complexity = 5;

        let tree_state = TreeState::new(tree_id.clone(), complexity);

        assert_eq!(tree_state.tree_id, tree_id);
        assert!(tree_state.id.is_none());
        assert_eq!(tree_state.config.complexity, complexity);
        assert_eq!(tree_state.config.max_depth, 6);
        assert_eq!(tree_state.config.branch_limit, 5);
        assert!(tree_state.metadata.is_empty());
    }

    #[test]
    fn test_set_root_id() {
        let mut tree_state = TreeState::new("test_tree".to_string(), 3);
        let root_id: RecordId = "node:root".parse().unwrap();

        assert!(tree_state.config.root_id.is_none());

        let initial_updated_at = tree_state.updated_at;

        tree_state.set_root_id(root_id.clone());

        assert_eq!(tree_state.config.root_id, Some(root_id));
        assert!(tree_state.updated_at > initial_updated_at);
    }

    #[test]
    fn test_tree_config_complexity_mapping() {
        let config_low = TreeConfig::new(2);
        assert_eq!(config_low.max_depth, 3);
        assert_eq!(config_low.branch_limit, 3);

        let config_medium = TreeConfig::new(4);
        assert_eq!(config_medium.max_depth, 4);
        assert_eq!(config_medium.branch_limit, 4);

        let config_high = TreeConfig::new(6);
        assert_eq!(config_high.max_depth, 6);
        assert_eq!(config_high.branch_limit, 5);

        let config_extreme = TreeConfig::new(10);
        assert_eq!(config_extreme.max_depth, 8);
        assert_eq!(config_extreme.branch_limit, 6);
    }

    #[test]
    fn test_tree_config_defaults() {
        let config = TreeConfig::new(5);

        assert!(config.root_id.is_none());
        assert_eq!(config.min_probability, 0.15);
        assert!(config.use_laplace);
        assert_eq!(config.complexity, 5);
    }

    #[test]
    fn test_serialization() {
        let tree_state = TreeState::new("test_tree_serialization".to_string(), 5);

        let serialized = serde_json::to_string(&tree_state).unwrap();
        let deserialized: TreeState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tree_state.tree_id, deserialized.tree_id);
        assert_eq!(tree_state.config.complexity, deserialized.config.complexity);
        assert_eq!(tree_state.config.max_depth, deserialized.config.max_depth);
    }
}
