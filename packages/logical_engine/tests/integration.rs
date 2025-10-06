/// Integration tests for the Logical Inference Engine
/// 
/// These tests demonstrate end-to-end functionality of the logical_engine package,
/// showing how to use add_bulk, query, and explain_inference together.

use logical_engine::domain::services::LogicalInferenceEngine;

#[tokio::test]
async fn test_integration_happy_path() {
    // Create a new engine
    let mut engine = LogicalInferenceEngine::new();
    
    // Step 1: add_bulk with facts and rules
    let datalog = r#"
% Facts about animals
perro(fido).
perro(rex).
existe(fido).
existe(rex).

% Rule: dogs that exist can eat
come(X) :- perro(X), existe(X).
"#;
    
    let result = engine.load_bulk(datalog, true).await;
    
    // Verify bulk add succeeded
    assert_eq!(result.added_count, 5, "Should add 5 statements (2+2 facts + 1 rule)");
    assert_eq!(result.total_count, 5);
    assert!(result.errors.is_empty(), "Should have no errors");
    assert!(!result.rolled_back, "Should not be rolled back");
    
    // Step 2: list_premises to verify they were added
    let premises = engine.list_premises();
    assert!(premises.contains("perro(fido)"), "Should contain perro(fido)");
    assert!(premises.contains("come(X)"), "Should contain the rule");
    
    // Step 3: query to test inference
    let query_result = engine.query("?- come(fido).", 5000).await;
    
    // Note: Full query execution is not yet implemented, so this demonstrates the structure
    // In a complete implementation, this would return proven=true
    assert!(query_result.explanation.is_some(), "Should have an explanation");
    
    // Step 4: validate a rule
    let validation = engine.validate_rule("mortal(X) :- humano(X).");
    assert!(validation.is_valid, "Valid rule should pass validation");
    assert!(validation.errors.is_empty(), "Should have no errors");
}

#[tokio::test]
async fn test_integration_atomic_rollback() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Try to add bulk with atomic=true and one invalid statement
    let datalog = r#"
perro(fido).
invalid syntax here!
come(X) :- perro(X).
"#;
    
    let result = engine.load_bulk(datalog, true).await;
    
    // With atomic=true, nothing should be added if there's an error
    assert_eq!(result.added_count, 0, "Nothing should be added with atomic rollback");
    assert!(result.rolled_back, "Should be rolled back");
    assert!(!result.errors.is_empty(), "Should have errors");
    
    // Verify nothing was actually added
    let premises = engine.list_premises();
    assert!(!premises.contains("perro"), "Should not contain any facts after rollback");
}

#[tokio::test]
async fn test_integration_non_atomic_partial_success() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Try to add bulk with atomic=false and one invalid statement
    let datalog = r#"
perro(fido).
invalid syntax here!
come(X) :- perro(X).
"#;
    
    let result = engine.load_bulk(datalog, false).await;
    
    // With atomic=false, valid statements should be added
    assert_eq!(result.added_count, 2, "Should add 2 valid statements");
    assert_eq!(result.total_count, 3, "Total should be 3");
    assert_eq!(result.errors.len(), 1, "Should have 1 error");
    assert!(!result.rolled_back, "Should not be rolled back");
    
    // Verify valid statements were added
    let premises = engine.list_premises();
    assert!(premises.contains("perro(fido)"), "Should contain valid fact");
    assert!(premises.contains("come(X)"), "Should contain valid rule");
}

#[tokio::test]
async fn test_integration_validation_unbound_variable() {
    let engine = LogicalInferenceEngine::new();
    
    // Validate a rule with unbound variable in head
    let result = engine.validate_rule("head(X, Y) :- body(X).");
    
    assert!(!result.is_valid, "Rule with unbound variable should be invalid");
    assert!(!result.errors.is_empty(), "Should have errors");
    assert!(result.errors.iter().any(|e| e.contains("Y") && e.contains("not bound")), 
            "Should mention Y is not bound");
}

#[tokio::test]
async fn test_integration_session_isolation() {
    // Create two separate engine instances
    let mut engine_a = LogicalInferenceEngine::new();
    let mut engine_b = LogicalInferenceEngine::new();
    
    // Add facts to engine A
    let result_a = engine_a.load_bulk("perro(fido).\nexiste(fido).", true).await;
    assert_eq!(result_a.added_count, 2);
    
    // Engine B should not see engine A's facts
    let premises_b = engine_b.list_premises();
    assert!(!premises_b.contains("perro(fido)"), "Engine B should not see Engine A's facts");
    
    // Add different facts to engine B
    let result_b = engine_b.load_bulk("gato(whiskers).", true).await;
    assert_eq!(result_b.added_count, 1);
    
    // Engine A should not see engine B's facts
    let premises_a = engine_a.list_premises();
    assert!(!premises_a.contains("gato"), "Engine A should not see Engine B's facts");
    assert!(premises_a.contains("perro(fido)"), "Engine A should still have its own facts");
}

#[tokio::test]
async fn test_integration_reset() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Add some facts
    let _ = engine.load_bulk("perro(fido).\nexiste(fido).", true).await;
    
    // Verify they exist
    let premises_before = engine.list_premises();
    assert!(premises_before.contains("perro(fido)"));
    
    // Reset the engine
    engine.reset();
    
    // Verify everything is cleared
    let premises_after = engine.list_premises();
    assert!(!premises_after.contains("perro"), "Premises should be cleared");
    assert!(premises_after.contains("No premises"), "Should indicate no premises loaded");
}

#[tokio::test]
async fn test_integration_validate_before_add() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Validate a rule before adding it
    let rule = "come(X) :- perro(X), existe(X).";
    let validation = engine.validate_rule(rule);
    
    assert!(validation.is_valid, "Valid rule should pass validation");
    
    // Now add it
    let result = engine.load_rule(rule).await;
    assert!(result.is_ok(), "Valid rule should be added successfully");
    
    // Verify it's in the knowledge base
    let premises = engine.list_premises();
    assert!(premises.contains("come(X)"), "Rule should be in knowledge base");
}

#[tokio::test]
async fn test_integration_complex_knowledge_base() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Build a more complex knowledge base
    let datalog = r#"
% Animal hierarchy
animal(X) :- perro(X).
animal(X) :- gato(X).

% Specific animals
perro(fido).
perro(rex).
gato(whiskers).

% Properties
vive(fido).
vive(rex).
vive(whiskers).

% Complex rule
puede_vivir(X) :- animal(X), vive(X).
"#;
    
    let result = engine.load_bulk(datalog, true).await;
    
    // Verify all statements were added
    assert!(result.added_count >= 8, "Should add multiple facts and rules");
    assert!(result.errors.is_empty(), "Complex KB should be valid");
    
    // Check that all components are present
    let premises = engine.list_premises();
    assert!(premises.contains("animal(X) :- perro(X)"), "Should have first rule");
    assert!(premises.contains("animal(X) :- gato(X)"), "Should have second rule");
    assert!(premises.contains("puede_vivir(X)"), "Should have complex rule");
    assert!(premises.contains("perro(fido)"), "Should have facts");
}
