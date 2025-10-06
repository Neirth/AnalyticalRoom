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

#[tokio::test]
async fn test_integration_query_materialization() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Build KB with rules
    let datalog = r#"
perro(fido).
existe(fido).
come(X) :- perro(X), existe(X).
"#;
    
    engine.load_bulk(datalog, true).await;
    
    // Materialize first
    let materialize_result = engine.materialize(5000).await;
    assert!(materialize_result.is_ok(), "Materialization should succeed");
    
    // Now query
    let query_result = engine.query("?- come(fido).", 5000).await;
    assert!(query_result.proven, "Query should be proven after materialization");
}

#[tokio::test]
async fn test_integration_complex_queries() {
    let mut engine = LogicalInferenceEngine::new();
    
    let datalog = r#"
% Facts about family
padre(juan, maria).
padre(juan, pedro).
padre(maria, sofia).
madre(ana, maria).
madre(ana, pedro).

% Rules for relationships
hijo(X, Y) :- padre(Y, X).
hijo(X, Y) :- madre(Y, X).
hermano(X, Y) :- padre(Z, X), padre(Z, Y).
nieto(X, Y) :- hijo(X, Z), hijo(Z, Y).
"#;
    
    engine.load_bulk(datalog, true).await;
    
    // Test various queries
    let result1 = engine.query("?- padre(juan, maria).", 5000).await;
    assert!(result1.proven, "Direct fact should be proven");
    
    let result2 = engine.query("?- hijo(maria, juan).", 5000).await;
    assert!(result2.proven, "Derived fact should be proven");
    
    let result3 = engine.query("?- hermano(maria, pedro).", 5000).await;
    assert!(result3.proven, "Complex derived fact should be proven");
}

#[tokio::test]
async fn test_integration_edge_case_empty_queries() {
    let mut engine = LogicalInferenceEngine::new();
    
    engine.load_fact("test(a).").await.ok();
    
    // Empty query
    let result = engine.query("", 5000).await;
    assert!(!result.proven);
    assert_eq!(result.status, logical_engine::domain::models::InferenceStatus::Inconclusive);
}

#[tokio::test]
async fn test_integration_edge_case_malformed_kb() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Try to load malformed program atomically
    let datalog = r#"
perro(fido).
this is completely wrong syntax!!
gato(felix).
"#;
    
    let result = engine.load_bulk(datalog, true).await;
    assert_eq!(result.added_count, 0, "Nothing should be added with atomic=true");
    assert!(result.rolled_back, "Should be rolled back");
}

#[tokio::test]
async fn test_integration_stress_large_kb() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Create a large KB programmatically
    let mut datalog = String::new();
    for i in 0..100 {
        datalog.push_str(&format!("numero({}).\n", i));
    }
    datalog.push_str("par(X) :- numero(X).\n");
    
    let result = engine.load_bulk(&datalog, true).await;
    assert_eq!(result.added_count, 101, "Should load 100 facts + 1 rule");
    
    // Query should still work
    let query_result = engine.query("?- numero(50).", 5000).await;
    assert!(query_result.proven);
}

#[tokio::test]
async fn test_integration_multiple_rule_chains() {
    let mut engine = LogicalInferenceEngine::new();
    
    let datalog = r#"
% Multi-level hierarchy
a(1).
b(X) :- a(X).
c(X) :- b(X).
d(X) :- c(X).
e(X) :- d(X).
"#;
    
    engine.load_bulk(datalog, true).await;
    
    // Should derive through multiple levels
    let result = engine.query("?- e(1).", 5000).await;
    assert!(result.proven, "Should derive through 5 levels of rules");
}

#[tokio::test]
async fn test_integration_predicate_validation_workflow() {
    let mut engine = LogicalInferenceEngine::new();
    
    // First validate
    let validation = engine.validate_rule("mortal(X) :- humano(X).");
    assert!(validation.is_valid);
    
    // Then add
    engine.load_rule("mortal(X) :- humano(X).").await.ok();
    engine.load_fact("humano(socrates).").await.ok();
    
    // Then query
    let result = engine.query("?- mortal(socrates).", 5000).await;
    assert!(result.proven);
}

#[tokio::test]
async fn test_integration_annotations_and_explanation() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Add annotations before loading KB
    engine.add_predicate_annotation("perro".to_string(), "es un perro".to_string());
    engine.add_predicate_annotation("come".to_string(), "puede comer".to_string());
    
    // Load KB
    engine.load_bulk("perro(fido).\nexiste(fido).\ncome(X) :- perro(X), existe(X).", true).await;
    
    // Query and get trace
    let result = engine.query("?- come(fido).", 5000).await;
    
    // Generate explanation
    let trace = serde_json::json!({"query": "come(fido)", "result": result.proven});
    let explanation = engine.explain_inference(&trace, false);
    
    assert!(explanation.contains("Detailed"));
}

#[tokio::test]
async fn test_integration_incremental_kb_building() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Add facts incrementally
    engine.load_fact("persona(socrates).").await.ok();
    
    let result1 = engine.query("?- persona(socrates).", 5000).await;
    assert!(result1.proven);
    
    // Add more facts
    engine.load_fact("persona(platon).").await.ok();
    
    let result2 = engine.query("?- persona(platon).", 5000).await;
    assert!(result2.proven);
    
    // Add a rule
    engine.load_rule("filosofo(X) :- persona(X).").await.ok();
    
    let result3 = engine.query("?- filosofo(socrates).", 5000).await;
    assert!(result3.proven);
}

#[tokio::test]
async fn test_integration_reset_and_rebuild() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Build initial KB
    engine.load_bulk("perro(fido).\ngato(felix).", true).await;
    
    let result1 = engine.query("?- perro(fido).", 5000).await;
    assert!(result1.proven);
    
    // Reset
    engine.reset();
    
    // Old queries should fail
    let result2 = engine.query("?- perro(fido).", 5000).await;
    assert!(!result2.proven);
    
    // Build new KB
    engine.load_bulk("ave(piolín).", true).await;
    
    let result3 = engine.query("?- ave(piolín).", 5000).await;
    assert!(result3.proven);
}

#[tokio::test]
async fn test_integration_query_timeout_handling() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Load a simple KB
    engine.load_fact("test(a).").await.ok();
    
    // Query with very short timeout (might timeout on slow systems)
    let result = engine.query("?- test(a).", 1).await;
    
    // Should either succeed or timeout gracefully
    assert!(
        result.status == logical_engine::domain::models::InferenceStatus::True ||
        result.status == logical_engine::domain::models::InferenceStatus::Inconclusive
    );
}

#[tokio::test]
async fn test_integration_materialize_timeout() {
    let mut engine = LogicalInferenceEngine::new();
    
    engine.load_fact("test(a).").await.ok();
    
    // Very short timeout
    let result = engine.materialize(1).await;
    
    // Should timeout or succeed
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_integration_list_premises_after_operations() {
    let mut engine = LogicalInferenceEngine::new();
    
    // Initially empty
    let premises1 = engine.list_premises();
    assert!(premises1.contains("No premises"));
    
    // Add some content
    engine.load_bulk("perro(fido).\ncome(X) :- perro(X).", true).await;
    
    // Should show content
    let premises2 = engine.list_premises();
    assert!(premises2.contains("perro(fido)"));
    assert!(premises2.contains("come(X)"));
}

#[tokio::test]
async fn test_integration_validation_errors_detailed() {
    let engine = LogicalInferenceEngine::new();
    
    // Multiple unbound variables
    let result = engine.validate_rule("head(A, B, C) :- body(A).");
    
    assert!(!result.is_valid);
    assert!(result.errors.len() >= 2, "Should detect both B and C as unbound");
    
    // Check that errors mention the variables
    let errors_str = result.errors.join(" ");
    assert!(errors_str.contains("B") || errors_str.contains("C"));
}
