use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};
use crate::domain::models::tree_state::TreeConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub surviving_paths: Vec<PathResult>,
    pub insights: Vec<String>,
    pub confidence_assessment: f64,
    pub narrative_style: NarrativeStyle,
    pub total_thought_tokens: usize,
    // Enhanced verbose information like inspect_tree
    pub tree_visualization: Option<TreeVisualization>,
    pub node_details: HashMap<String, NodeVisualization>,
    pub tree_statistics: Option<TreeStatsSummary>,
    pub tree_distributions: Option<TreeDistributions>,
    pub active_paths_detail: Vec<ActivePath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResult {
    pub path: Vec<String>, // Changed from Uuid to String
    pub premises: Vec<String>,
    pub final_probability: f64,
    pub reasoning_chain: String,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NarrativeStyle {
    Analytical,
    Strategic,
    Storytelling,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UncertaintyType {
    InsufficientData,
    EqualLikelihood,
    CognitiveOverload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<ValidationViolation>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationViolation {
    pub violation_type: ViolationType,
    pub node_id: String, // Changed from Uuid to String
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    KolmogorovAxiom,
    HierarchicalConstraint,
    LogicalIncoherence,
    DepthLimit,
    BranchLimit,
    ProbabilityRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningResult {
    pub nodes_removed: Vec<String>, // Changed from Uuid to String
    pub nodes_preserved: Vec<String>, // Changed from Uuid to String
    pub manual_overrides: Vec<String>, // Changed from Uuid to String
    pub statistics: PruningStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningStatistics {
    pub original_count: usize,
    pub removed_count: usize,
    pub preserved_count: usize,
    pub aggressiveness_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancingResult {
    pub balanced_nodes: Vec<String>, // Changed from Uuid to String
    pub uncertainty_type: UncertaintyType,
    pub original_probabilities: HashMap<String, f64>, // Changed from Uuid to String
    pub new_probabilities: HashMap<String, f64>, // Changed from Uuid to String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAnalysis {
    pub is_coherent: bool,
    pub contradictions: Vec<ContradictionResult>,
    pub truth_table: Vec<TruthTableRow>,
    pub eliminated_nodes: Vec<String>, // Changed from Uuid to String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionResult {
    pub node_id: String, // Changed from Uuid to String
    pub conflicting_premises: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruthTableRow {
    pub premises: HashMap<String, bool>,
    pub is_consistent: bool,
    pub affected_nodes: Vec<String>, // Changed from Uuid to String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeVisualization {
    pub ascii_tree: String, // Empty in service layer, filled by presentation layer
    pub node_details: HashMap<String, NodeVisualization>,
    pub tree_metadata: TreeMetadata,
    pub statistics: TreeStatsSummary,
    pub distributions: TreeDistributions,
    pub active_paths: Vec<ActivePath>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeMetadata {
    pub tree_id: String,
    pub complexity: i64,
    pub config: TreeConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeDistributions {
    pub confidence_distribution: HashMap<i64, usize>,
    pub depth_distribution: HashMap<u32, usize>,
    pub probability_distribution: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePath {
    pub path_number: usize,
    pub leaf_id: String,
    pub premise: String,
    pub probability: f64,
    pub confidence: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVisualization {
    pub friendly_id: String,
    pub premise_summary: String,
    pub full_premise: String,
    pub full_reasoning: String,
    pub probability: f64,
    pub depth: u32,
    pub children_count: usize,
    pub children_ids: Vec<String>,
    pub parent_id: Option<String>,
    pub is_leaf: bool,
    pub can_expand: bool,
    pub confidence: i64,
    pub status: NodeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Invalidated,
    Pruned,
    Expanded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeStatsSummary {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub invalidated_nodes: usize,
    pub active_paths: usize,
    pub max_depth: u32,
    pub avg_depth: f64,
    pub avg_probability: f64,
    pub probability_median: f64,
    pub probability_range: (f64, f64),
    pub complexity_score: f64,
    pub avg_premise_length: f64,
    pub avg_reasoning_length: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTreeRequest {
    pub premise: String,
    pub complexity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddLeafRequest {
    pub parent_id: Option<String>,
    pub premise: String,
    pub reasoning: String,
    pub probability: f64,
    pub confidence: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandLeafRequest {
    pub node_id: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateToRequest {
    pub node_id: String,
    pub justification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneTreeRequest {
    pub aggressiveness: Option<f64>,
    pub manual_overrides: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneLeafsRequest {
    pub parent_id: Option<String>,
    pub keep_count: Option<usize>,
    pub manual_overrides: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceLeafsRequest {
    pub parent_id: Option<String>,
    pub uncertainty_type: UncertaintyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateCoherenceRequest {
    pub node_id: Option<String>,
    pub analysis_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPathsRequest {
    pub narrative_style: NarrativeStyle,
    pub insights: Vec<String>,
    pub confidence_assessment: f64,
}

impl Default for NarrativeStyle {
    fn default() -> Self {
        NarrativeStyle::Analytical
    }
}

impl Default for UncertaintyType {
    fn default() -> Self {
        UncertaintyType::InsufficientData
    }
}

// Display implementations for verbose output

impl Display for AnalysisResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(f, "=== PROBABILITY TREE ANALYSIS RESULT ===")?;
        writeln!(f, "Narrative Style: {:?}", self.narrative_style)?;
        writeln!(f, "Confidence Assessment: {:.2}%", self.confidence_assessment * 100.0)?;
        writeln!(f, "Total Thought Tokens: {}", self.total_thought_tokens)?;
        writeln!(f)?;

        // Insights Section
        writeln!(f, "📝 INSIGHTS ({}):", self.insights.len())?;
        for (i, insight) in self.insights.iter().enumerate() {
            writeln!(f, "  {}. {}", i + 1, insight)?;
        }
        writeln!(f)?;

        // Surviving Paths Section
        writeln!(f, "🛤️  SURVIVING PATHS ({}):", self.surviving_paths.len())?;
        for (i, path) in self.surviving_paths.iter().enumerate() {
            writeln!(f, "  Path {}: {:.2}% probability", i + 1, path.final_probability * 100.0)?;
            writeln!(f, "    Confidence: {:.2}", path.confidence_score)?;
            writeln!(f, "    Premises: {}", path.premises.join(" → "))?;
            writeln!(f, "    Reasoning: {}", path.reasoning_chain)?;
            writeln!(f)?;
        }

        // Tree Statistics Section (if available)
        if let Some(ref stats) = self.tree_statistics {
            writeln!(f, "📊 TREE STATISTICS:")?;
            writeln!(f, "  Total Nodes: {}", stats.total_nodes)?;
            writeln!(f, "  Active Nodes: {}", stats.active_nodes)?;
            writeln!(f, "  Maximum Depth: {}", stats.max_depth)?;
            writeln!(f, "  Average Probability: {:.2}%", stats.avg_probability * 100.0)?;
            writeln!(f, "  Complexity Score: {:.2}", stats.complexity_score)?;
            writeln!(f)?;
        }

        // Node Details Section
        if !self.node_details.is_empty() {
            writeln!(f, "🔍 NODE DETAILS ({} nodes):", self.node_details.len())?;
            for (node_id, node) in &self.node_details {
                writeln!(f, "  {}: {} (P: {:.2}%, D: {}, C: {})",
                    node_id,
                    node.premise_summary,
                    node.probability * 100.0,
                    node.depth,
                    node.confidence
                )?;
            }
            writeln!(f)?;
        }

        // Tree Distributions Section (if available)
        if let Some(ref distributions) = self.tree_distributions {
            writeln!(f, "📈 DISTRIBUTIONS:")?;
            writeln!(f, "  Confidence Distribution: {:?}", distributions.confidence_distribution)?;
            writeln!(f, "  Depth Distribution: {:?}", distributions.depth_distribution)?;
            if !distributions.probability_distribution.is_empty() {
                let prob_sum: f64 = distributions.probability_distribution.iter().sum();
                let prob_avg = prob_sum / distributions.probability_distribution.len() as f64;
                writeln!(f, "  Probability Average: {:.2}%", prob_avg * 100.0)?;
            }
            writeln!(f)?;
        }

        // Active Paths Detail Section
        if !self.active_paths_detail.is_empty() {
            writeln!(f, "🌟 ACTIVE PATHS DETAIL ({}):", self.active_paths_detail.len())?;
            for path in &self.active_paths_detail {
                writeln!(f, "  Path {}: {} (P: {:.2}%, C: {})",
                    path.path_number,
                    path.premise,
                    path.probability * 100.0,
                    path.confidence
                )?;
            }
            writeln!(f)?;
        }

        // Tree Visualization ASCII (if available)
        if let Some(ref viz) = self.tree_visualization {
            if !viz.ascii_tree.is_empty() {
                writeln!(f, "🌳 TREE VISUALIZATION:")?;
                writeln!(f, "{}", viz.ascii_tree)?;
            }

            if !viz.recommendations.is_empty() {
                writeln!(f, "💡 RECOMMENDATIONS:")?;
                for (i, rec) in viz.recommendations.iter().enumerate() {
                    writeln!(f, "  {}. {}", i + 1, rec)?;
                }
            }
        }

        Ok(())
    }
}

impl Display for PathResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(f, "Path Probability: {:.2}%", self.final_probability * 100.0)?;
        writeln!(f, "Confidence Score: {:.2}", self.confidence_score)?;
        writeln!(f, "Premises: {}", self.premises.join(" → "))?;
        writeln!(f, "Reasoning Chain: {}", self.reasoning_chain)?;
        Ok(())
    }
}

// Display implementations for rich text formatting

use std::fmt;

impl fmt::Display for TreeVisualization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;
        writeln!(f, "  🌳 PROBABILITY TREE ANALYSIS REPORT")?;
        writeln!(f, "  Tree ID: {}", self.tree_metadata.tree_id)?;
        writeln!(f, "  Complexity: {} | Config: {:?}", self.tree_metadata.complexity, self.tree_metadata.config)?;
        writeln!(f, "  Created: {} | Status: {}", self.tree_metadata.created_at.format("%Y-%m-%d %H:%M UTC"), self.tree_metadata.status)?;
        writeln!(f, "═══════════════════════════════════════════════════════════════\n")?;

        writeln!(f, "📊 TREE STRUCTURE & NODE ANALYSIS:\n")?;

        // Sort nodes by depth and probability for display
        let mut sorted_nodes: Vec<_> = self.node_details.values().collect();
        sorted_nodes.sort_by(|a, b| {
            a.depth.cmp(&b.depth)
                .then_with(|| b.probability.partial_cmp(&a.probability).unwrap_or(std::cmp::Ordering::Equal))
        });

        for node in sorted_nodes {
            write!(f, "{}", node)?;
            writeln!(f)?;
        }

        write!(f, "{}", self.statistics)?;
        write!(f, "{}", self.distributions)?;

        writeln!(f, "\n🛤️  ACTIVE DECISION PATHS:\n")?;
        for path in &self.active_paths {
            writeln!(f, "{}", path)?;
        }

        writeln!(f, "\n💡 ANALYSIS RECOMMENDATIONS:\n")?;
        for recommendation in &self.recommendations {
            writeln!(f, "{}", recommendation)?;
        }

        writeln!(f, "\n═══════════════════════════════════════════════════════════════")?;

        Ok(())
    }
}

impl fmt::Display for NodeVisualization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = "  ".repeat(self.depth as usize);
        let status_symbol = match self.status {
            NodeStatus::Invalidated => "❌",
            _ => if self.is_leaf { "🌿" } else { "🌳" }
        };

        writeln!(f, "{}{}[{}] PREMISE: {}", indent, status_symbol, self.friendly_id, self.full_premise)?;

        if !self.full_reasoning.trim().is_empty() {
            writeln!(f, "{}    ├─ REASONING: {}", indent, self.full_reasoning)?;
        }

        writeln!(f, "{}    ├─ PROBABILITY: {:.4} ({:.1}%) | CONFIDENCE: {}/10 | DEPTH: {}",
            indent, self.probability, self.probability * 100.0, self.confidence, self.depth)?;

        writeln!(f, "{}    ├─ CHILDREN: {} | EXPANDABLE: {} | STATUS: {:?}",
            indent, self.children_count, if self.can_expand { "YES" } else { "NO" }, self.status)?;

        if !self.children_ids.is_empty() {
            let friendly_children: Vec<String> = self.children_ids.iter()
                .map(|c| format!("N{}", c.chars().take(8).collect::<String>()))
                .collect();
            writeln!(f, "{}    └─ CHILD_IDs: [{}]", indent, friendly_children.join(", "))?;
        }

        Ok(())
    }
}

impl fmt::Display for TreeStatsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "📈 ADVANCED TREE STATISTICS:\n")?;

        writeln!(f, "├─ NODE COUNTS: Total={}, Active={}, Invalidated={}, Leaves={}",
            self.total_nodes, self.active_nodes, self.invalidated_nodes, self.active_paths)?;

        writeln!(f, "├─ TREE DEPTH: Max={}, Avg={:.1}", self.max_depth, self.avg_depth)?;

        writeln!(f, "├─ PROBABILITIES: Avg={:.4}, Median={:.4}, Range=[{:.4}, {:.4}]",
            self.avg_probability, self.probability_median, self.probability_range.0, self.probability_range.1)?;

        writeln!(f, "├─ COMPLEXITY SCORE: {:.2} (Based on depth×nodes formula)", self.complexity_score)?;

        writeln!(f, "├─ CONTENT ANALYSIS: Avg premise length: {:.0} chars", self.avg_premise_length)?;
        writeln!(f, "│                   Avg reasoning length: {:.0} chars", self.avg_reasoning_length)?;

        Ok(())
    }
}

impl fmt::Display for ActivePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path #{}: {} → {} (p={:.4}, c={})",
            self.path_number, self.leaf_id, self.premise, self.probability, self.confidence)
    }
}

impl fmt::Display for TreeDistributions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "├─ CONFIDENCE DISTRIBUTION:")?;
        for conf in 1..=10 {
            let count = self.confidence_distribution.get(&conf).copied().unwrap_or(0);
            let total = self.confidence_distribution.values().sum::<usize>() as f64;
            let percentage = if total > 0.0 { count as f64 / total * 100.0 } else { 0.0 };
            writeln!(f, "│  └─ Confidence {}: {} nodes ({:.1}%)", conf, count, percentage)?;
        }

        writeln!(f, "├─ DEPTH DISTRIBUTION:")?;
        let max_depth = self.depth_distribution.keys().max().copied().unwrap_or(0);
        for depth in 0..=max_depth {
            let count = self.depth_distribution.get(&depth).copied().unwrap_or(0);
            let total = self.depth_distribution.values().sum::<usize>() as f64;
            let percentage = if total > 0.0 { count as f64 / total * 100.0 } else { 0.0 };
            writeln!(f, "│  └─ Depth {}: {} nodes ({:.1}%)", depth, count, percentage)?;
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_result_creation() {
        let result = AnalysisResult {
            surviving_paths: vec![],
            insights: vec!["Test insight".to_string()],
            confidence_assessment: 0.8,
            narrative_style: NarrativeStyle::Analytical,
            total_thought_tokens: 1000,
            tree_visualization: None,
            node_details: HashMap::new(),
            tree_statistics: None,
            tree_distributions: None,
            active_paths_detail: vec![],
        };

        assert_eq!(result.insights.len(), 1);
        assert_eq!(result.confidence_assessment, 0.8);
        assert!(matches!(result.narrative_style, NarrativeStyle::Analytical));
    }

    #[test]
    fn test_validation_result() {
        let violation = ValidationViolation {
            violation_type: ViolationType::KolmogorovAxiom,
            node_id: "test_node_123".to_string(),
            message: "Test violation".to_string(),
            severity: Severity::Error,
        };

        let result = ValidationResult {
            is_valid: false,
            violations: vec![violation],
            suggestions: vec!["Fix the issue".to_string()],
        };

        assert!(!result.is_valid);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(result.violations[0].severity, Severity::Error));
    }

    #[test]
    fn test_default_implementations() {
        let default_style = NarrativeStyle::default();
        assert!(matches!(default_style, NarrativeStyle::Analytical));

        let default_uncertainty = UncertaintyType::default();
        assert!(matches!(default_uncertainty, UncertaintyType::InsufficientData));
    }

    #[test]
    fn test_serialization() {
        let request = CreateTreeRequest {
            premise: "Test premise".to_string(),
            complexity: 5,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CreateTreeRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.premise, deserialized.premise);
        assert_eq!(request.complexity, deserialized.complexity);
    }
}
