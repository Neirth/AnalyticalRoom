//! Logical Inference Service - Wrapper for Nemo Worker
//!
//! This service provides a high-level API for interacting with Nemo workers.
//! It manages worker lifecycle and delegates operations to the appropriate worker
//! based on the session ID.
//!
//! # Architecture
//! - Each service instance is tied to a specific MCP session ID
//! - Operations are delegated to the corresponding worker thread
//! - Workers are managed by the global worker pool
//! - Service instances are lightweight wrappers around worker handles

use crate::domain::errors::EngineResult;
use crate::domain::models::{AddBulkResult, InferenceResult, ValidateResult};
use crate::domain::services::nemo_worker::GLOBAL_WORKER_POOL;
use std::sync::Arc;

/// Logical Inference Service - provides async API for Nemo operations
///
/// This service wraps a Nemo worker and provides a clean async API.
/// Each service instance is associated with a specific MCP session ID.
pub struct LogicalInferenceService {
    /// Session ID for this service instance
    session_id: String,
}

impl LogicalInferenceService {
    /// Create a new Logical Inference Service for a session
    ///
    /// # Arguments
    /// * `session_id` - The MCP session ID for this service instance
    ///
    /// # Example
    /// ```no_run
    /// # use logical_engine::domain::services::LogicalInferenceService;
    /// let service = LogicalInferenceService::new("session-123".to_string());
    /// ```
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }

    /// Get the worker handle for this service's session
    async fn get_worker(&self) -> Arc<crate::domain::services::nemo_worker::NemoWorkerHandle> {
        GLOBAL_WORKER_POOL.get_worker(self.session_id.clone()).await
    }

    /// Load a single fact into the knowledge base
    ///
    /// # Arguments
    /// * `fact` - A Datalog fact (e.g., "perro(fido).")
    ///
    /// # Example
    /// ```no_run
    /// # use logical_engine::domain::services::LogicalInferenceService;
    /// # async fn example() {
    /// # let service = LogicalInferenceService::new("session-123".to_string());
    /// service.load_fact("perro(fido).".to_string()).await;
    /// # }
    /// ```
    pub async fn load_fact(&self, fact: String) -> EngineResult<()> {
        let worker = self.get_worker().await;
        worker.load_fact(fact).await
    }

    /// Load a single rule into the knowledge base
    ///
    /// # Arguments
    /// * `rule` - A Datalog rule (e.g., "come(X) :- perro(X), existe(X).")
    ///
    /// # Example
    /// ```no_run
    /// # use logical_engine::domain::services::LogicalInferenceService;
    /// # async fn example() {
    /// # let service = LogicalInferenceService::new("session-123".to_string());
    /// service.load_rule("come(X) :- perro(X), existe(X).".to_string()).await;
    /// # }
    /// ```
    pub async fn load_rule(&self, rule: String) -> EngineResult<()> {
        let worker = self.get_worker().await;
        worker.load_rule(rule).await
    }

    /// Load multiple facts and/or rules in bulk
    ///
    /// # Arguments
    /// * `datalog` - Multiple Datalog statements separated by newlines
    /// * `atomic` - If true, all statements must be valid or none are applied
    ///
    /// # Returns
    /// AddBulkResult with details about which statements were added
    pub async fn load_bulk(&self, datalog: String, atomic: bool) -> AddBulkResult {
        let worker = self.get_worker().await;
        worker.load_bulk(datalog, atomic).await
    }

    /// Execute a query against the knowledge base
    ///
    /// # Arguments
    /// * `query_str` - A Datalog query (e.g., "?- come(X).")
    /// * `timeout_ms` - Maximum time to wait for the query in milliseconds
    ///
    /// # Returns
    /// InferenceResult with the query result and trace
    pub async fn query(&self, query_str: String, timeout_ms: u64) -> InferenceResult {
        let worker = self.get_worker().await;
        worker.query(query_str, timeout_ms).await
    }

    /// Materialize all derivable facts
    ///
    /// This runs the inference engine to compute all facts that can be derived
    /// from the current rules and facts.
    pub async fn materialize(&self, timeout_ms: u64) -> EngineResult<()> {
        let worker = self.get_worker().await;
        worker.materialize(timeout_ms).await
    }

    /// Get JSON trace from the last query
    ///
    /// Returns the trace in JSON format if available
    pub async fn get_trace_json(&self) -> Option<serde_json::Value> {
        let worker = self.get_worker().await;
        worker.get_trace_json().await
    }

    /// Reset the knowledge base
    ///
    /// Clears all facts, rules, and resets the engine
    pub async fn reset(&self) {
        let worker = self.get_worker().await;
        worker.reset().await
    }

    /// List all premises (facts and rules) currently in the knowledge base
    ///
    /// Returns the current program as a string
    pub async fn list_premises(&self) -> String {
        let worker = self.get_worker().await;
        worker.list_premises().await
    }

    /// Validate a rule for syntax and basic issues
    ///
    /// # Arguments
    /// * `rule_str` - The rule to validate
    ///
    /// # Returns
    /// ValidateResult with any errors or warnings found
    pub async fn validate_rule(&self, rule_str: String) -> ValidateResult {
        let worker = self.get_worker().await;
        worker.validate_rule(rule_str).await
    }

    /// Add a predicate annotation for human-readable explanations
    ///
    /// # Arguments
    /// * `predicate` - The predicate name (e.g., "perro")
    /// * `annotation` - Human-readable description (e.g., "is a dog")
    pub async fn add_predicate_annotation(&self, predicate: String, annotation: String) {
        let worker = self.get_worker().await;
        worker.add_predicate_annotation(predicate, annotation).await
    }

    /// Explain an inference using natural language
    ///
    /// # Arguments
    /// * `trace_json` - The trace JSON from Nemo
    /// * `short` - Whether to return a short summary
    ///
    /// # Returns
    /// A human-readable explanation of the inference
    pub async fn explain_inference(&self, trace_json: serde_json::Value, short: bool) -> String {
        let worker = self.get_worker().await;
        worker.explain_inference(trace_json, short).await
    }

    /// Shutdown and cleanup this service's worker
    ///
    /// This removes the worker from the global pool and shuts it down.
    /// The service instance should not be used after calling this method.
    ///
    /// # Important
    /// Prefer calling this method explicitly before the service is dropped
    /// to ensure clean shutdown. The Drop implementation provides best-effort
    /// cleanup but may not execute if the Tokio runtime is unavailable.
    pub async fn shutdown(&self) {
        GLOBAL_WORKER_POOL.remove_worker(&self.session_id).await;
    }

    /// Get the session ID for this service
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

impl Drop for LogicalInferenceService {
    fn drop(&mut self) {
        // Try to schedule worker cleanup if a Tokio runtime is available
        // This is a best-effort cleanup; prefer calling shutdown() explicitly
        let session_id = self.session_id.clone();
        
        // Check if we have a Tokio runtime available
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // Runtime is available, spawn async cleanup
            handle.spawn(async move {
                GLOBAL_WORKER_POOL.remove_worker(&session_id).await;
            });
        } else {
            // No runtime available - worker will be cleaned up when pool is dropped
            // or on next access. This is safe because workers are independent.
            // Log a warning in debug mode
            #[cfg(debug_assertions)]
            eprintln!(
                "Warning: LogicalInferenceService dropped without runtime. \
                 Worker for session '{}' will be cleaned up later. \
                 Consider calling shutdown() explicitly.",
                session_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_new() {
        let service = LogicalInferenceService::new("test-session".to_string());
        assert_eq!(service.session_id(), "test-session");
    }

    #[tokio::test]
    async fn test_service_load_fact() {
        let service = LogicalInferenceService::new("test-fact".to_string());
        let result = service.load_fact("perro(fido).".to_string()).await;
        assert!(result.is_ok());
        
        let premises = service.list_premises().await;
        assert!(premises.contains("perro(fido)"));
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_load_rule() {
        let service = LogicalInferenceService::new("test-rule".to_string());
        let result = service.load_rule("come(?X) :- perro(?X).".to_string()).await;
        assert!(result.is_ok());
        
        let premises = service.list_premises().await;
        assert!(premises.contains("come(?X)"));
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_reset() {
        let service = LogicalInferenceService::new("test-reset".to_string());
        service.load_fact("test(a).".to_string()).await.ok();
        service.reset().await;
        
        let premises = service.list_premises().await;
        assert!(premises.contains("No premises"));
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_isolation() {
        let service1 = LogicalInferenceService::new("iso-1".to_string());
        let service2 = LogicalInferenceService::new("iso-2".to_string());
        
        service1.load_fact("data1(a).".to_string()).await.ok();
        service2.load_fact("data2(b).".to_string()).await.ok();
        
        let premises1 = service1.list_premises().await;
        let premises2 = service2.list_premises().await;
        
        assert!(premises1.contains("data1"));
        assert!(!premises1.contains("data2"));
        assert!(premises2.contains("data2"));
        assert!(!premises2.contains("data1"));
        
        service1.shutdown().await;
        service2.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_bulk_load() {
        let service = LogicalInferenceService::new("test-bulk".to_string());
        
        let datalog = "perro(fido).\nexiste(fido).";
        let result = service.load_bulk(datalog.to_string(), true).await;
        
        assert_eq!(result.added_count, 2);
        assert!(result.errors.is_empty());
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_validate_rule() {
        let service = LogicalInferenceService::new("test-validate".to_string());
        
        let result = service.validate_rule("mortal(?X) :- humano(?X).".to_string()).await;
        assert!(result.is_valid);
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_predicate_annotation() {
        let service = LogicalInferenceService::new("test-annotation".to_string());
        
        service.add_predicate_annotation("perro".to_string(), "is a dog".to_string()).await;
        
        // Annotation is stored, we can't directly query it but it doesn't error
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_explain_inference() {
        let service = LogicalInferenceService::new("test-explain".to_string());
        
        let trace = serde_json::json!({"test": "data"});
        let explanation = service.explain_inference(trace, true).await;
        
        assert!(explanation.contains("Inference"));
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_materialize() {
        let service = LogicalInferenceService::new("test-materialize".to_string());
        
        service.load_fact("perro(fido).".to_string()).await.ok();
        service.load_rule("come(?X) :- perro(?X).".to_string()).await.ok();
        
        let result = service.materialize(5000).await;
        assert!(result.is_ok());
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_get_trace_json() {
        let service = LogicalInferenceService::new("test-trace".to_string());
        
        let trace = service.get_trace_json().await;
        assert!(trace.is_none()); // Currently returns None
        
        service.shutdown().await;
    }

    #[tokio::test]
    async fn test_explicit_shutdown() {
        // Test that explicit shutdown works correctly
        let service = LogicalInferenceService::new("shutdown-test".to_string());
        
        // Load some data
        service.load_fact("test(data).".to_string()).await.ok();
        
        // Explicitly shutdown
        service.shutdown().await;
        
        // Worker should be removed from pool
        // If we try to access it again with a new service, it will create a new worker
        let service2 = LogicalInferenceService::new("shutdown-test".to_string());
        let premises = service2.list_premises().await;
        
        // New worker starts with empty knowledge base
        assert!(!premises.contains("test(data)"));
        
        service2.shutdown().await;
    }

    #[tokio::test]
    async fn test_drop_with_runtime() {
        // Test that Drop works when runtime is available
        let service = LogicalInferenceService::new("drop-test".to_string());
        
        // Load some data
        service.load_fact("drop(test).".to_string()).await.ok();
        
        // Drop the service (runtime is available in tokio::test)
        drop(service);
        
        // Give Drop cleanup time to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        // Worker should eventually be cleaned up
    }

    #[test]
    fn test_drop_without_runtime() {
        // Test that Drop doesn't panic when no runtime is available
        // This test runs WITHOUT #[tokio::test], so no runtime exists
        let service = LogicalInferenceService::new("no-runtime-drop".to_string());
        
        // Drop should not panic even without runtime
        drop(service);
        
        // Test passes if we reach here without panicking
    }
}
