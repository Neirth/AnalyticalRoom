use rand::Rng;

const ADJECTIVES: &[&str] = &[
    "clever", "brilliant", "wise", "analytical", "strategic", "creative", "focused", "sharp",
    "insightful", "logical", "systematic", "methodical", "precise", "efficient", "elegant",
    "robust", "dynamic", "flexible", "adaptive", "innovative", "thoughtful", "careful",
    "meticulous", "thorough", "comprehensive", "detailed", "accurate", "reliable", "consistent",
    "coherent", "rational", "objective", "balanced", "measured", "deliberate", "calculated",
];

const SCIENTISTS: &[&str] = &[
    "einstein", "curie", "newton", "darwin", "tesla", "galileo", "kepler", "pasteur",
    "mendel", "faraday", "maxwell", "bohr", "heisenberg", "schrodinger", "hawking",
    "feynman", "turing", "nash", "godel", "ramanujan", "gauss", "euler", "fibonacci",
    "archimedes", "pythagoras", "aristotle", "plato", "bacon", "descartes", "kant",
    "locke", "hume", "spinoza", "leibniz", "hobbes", "rousseau", "voltaire", "diderot",
];

pub fn generate_friendly_id() -> String {
    let mut rng = rand::rng();
    let adjective = ADJECTIVES[rng.random_range(0..ADJECTIVES.len())];
    let scientist = SCIENTISTS[rng.random_range(0..SCIENTISTS.len())];
    
    format!("{}_{}", adjective.to_lowercase(), scientist)
}

// Since we're using friendly IDs as the primary IDs now, resolve validates format and returns the ID
pub fn resolve_node_id(id: &str) -> Option<String> {
    // Validate that the ID has the expected friendly ID format (adjective_scientist)
    // Must be exactly two parts separated by one underscore
    let parts: Vec<&str> = id.split('_').collect();
    if parts.len() == 2 
        && parts[0].chars().all(|c| c.is_lowercase() && c.is_alphabetic())
        && parts[1].chars().all(|c| c.is_lowercase() && c.is_alphabetic())
        && ADJECTIVES.contains(&parts[0])
        && SCIENTISTS.contains(&parts[1]) {
        Some(id.to_string())
    } else {
        None // Invalid format
    }
}

// Get friendly ID - since IDs are already friendly, just return the same ID
pub fn get_friendly_id(id: &str) -> Option<String> {
    Some(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_friendly_id() {
        let id = generate_friendly_id();
        
        // Should contain an underscore
        assert!(id.contains('_'));
        
        // Should be lowercase
        assert_eq!(id.to_lowercase(), id);
        
        // Should contain one of the adjectives
        assert!(ADJECTIVES.iter().any(|&adj| id.contains(&adj.to_lowercase())));
        
        // Should contain one of the scientists
        assert!(SCIENTISTS.iter().any(|&sci| id.contains(&sci.to_lowercase())));
    }
    
    #[test]
    fn test_friendly_ids_are_unique() {
        let id1 = generate_friendly_id();
        let id2 = generate_friendly_id();
        
        // Two consecutive IDs should be different
        assert_ne!(id1, id2);
    }
}