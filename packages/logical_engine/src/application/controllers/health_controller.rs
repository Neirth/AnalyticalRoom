//! Health Check Controller for MCP Server
//!
//! This module provides health check endpoints for monitoring and system status.

use axum::response::Json;

/// Health check endpoint
pub async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "logical_engine_mcp",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "authentication": "active (dummy mode)"
    }))
}

/// Root endpoint - provides server information
pub async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "Logical Inference MCP Server",
        "version": "0.1.0",
        "description": "Datalog-based logical inference server with MCP protocol support using Nemo engine",
        "endpoints": {
            "mcp": "/mcp",
            "health": "/health",
            "oauth": {
                "discovery": "/.well-known/oauth-authorization-server",
                "authorize": "/oauth/authorize",
                "token": "/oauth/token",
                "register": "/oauth/register",
                "protected_resource": "/.well-known/oauth-protected-resource/mcp"
            }
        },
        "authentication": "dummy (always allows access)",
        "session_isolation": true,
        "nemo_version": "main branch (git)",
        "inference_engine": "Nemo"
    }))
}
