//! OAuth2 Authentication Controller for MCP Server
//!
//! This module provides OAuth2 endpoints that work with MCP clients.
//! All endpoints provide public access without real authentication.

use std::collections::HashMap;
use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde_json::json;
use uuid::Uuid;

/// OAuth Authorization Server Discovery - Standard OAuth2 endpoints
pub async fn oauth_authorization_server_discovery(headers: HeaderMap) -> Json<serde_json::Value> {
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1:8080");

    let protocol = if host.starts_with("127.0.0.1") || host.starts_with("localhost") {
        "http"
    } else {
        "https"
    };
    let origin = format!("{}://{}", protocol, host);

    Json(json!({
        "issuer": origin,
        "authorization_endpoint": format!("{}/oauth/authorize", origin),
        "token_endpoint": format!("{}/oauth/token", origin),
        "registration_endpoint": format!("{}/oauth/register", origin),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": [],
        "code_challenge_methods_supported": ["plain", "S256"],
        "public_access": true,
        "auth_required": false,
        "require_request_uri_registration": false
    }))
}

/// OAuth Authorization Endpoint - Auto-redirect without auth
pub async fn oauth_authorize_handler(
    Query(params): Query<HashMap<String, String>>,
) -> Result<axum::response::Response, StatusCode> {
    let redirect_uri = params.get("redirect_uri");
    let state = params.get("state");
    let _code_challenge = params.get("code_challenge");
    let _code_challenge_method = params.get("code_challenge_method");

    if redirect_uri.is_none() {
        return Ok(axum::response::Response::builder()
            .status(400)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(
                json!({
                    "error": "invalid_request",
                    "error_description": "Missing redirect_uri parameter"
                }).to_string().into()
            )
            .unwrap());
    }

    let redirect_uri = redirect_uri.unwrap();

    // Generate a dummy authorization code for public access
    let auth_code = format!("public_{}",
        Uuid::new_v4().simple().to_string().chars().take(7).collect::<String>());

    // Build redirect URL with auth code and state
    let mut redirect_url = url::Url::parse(redirect_uri)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    redirect_url.query_pairs_mut()
        .append_pair("code", &auth_code);

    if let Some(state) = state {
        redirect_url.query_pairs_mut()
            .append_pair("state", state);
    }

    // Redirect to callback URL
    Ok(axum::response::Response::builder()
        .status(302)
        .header("Location", redirect_url.to_string())
        .header("Access-Control-Allow-Origin", "*")
        .body("".into())
        .unwrap())
}

/// OAuth Token Endpoint - Return public access token
pub async fn oauth_token_handler() -> Json<serde_json::Value> {
    Json(json!({
        "access_token": "public_access",
        "token_type": "bearer",
        "expires_in": 3600,
        "scope": "",
        "public_access": true
    }))
}

/// OAuth Registration Endpoint - Return dummy client
pub async fn oauth_register_handler() -> Json<serde_json::Value> {
    let client_id = format!("mcp-public-{}",
        Uuid::new_v4().simple().to_string().chars().take(7).collect::<String>());

    Json(json!({
        "client_id": client_id,
        "client_name": "MCP Public Client",
        "redirect_uris": [],
        "token_endpoint_auth_method": "none",
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "scope": "",
        "public_access": true
    }))
}

/// OAuth Protected Resource Discovery - Public Resource
pub async fn oauth_protected_resource_discovery(headers: HeaderMap) -> Json<serde_json::Value> {
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1:8080");

    let protocol = if host.starts_with("127.0.0.1") || host.starts_with("localhost") {
        "http"
    } else {
        "https"
    };
    let origin = format!("{}://{}", protocol, host);

    Json(json!({
        "resource": format!("{}/mcp", origin),
        "resource_name": "Deep Analytics MCP Server",
        "authorization_servers": [], // Empty = no auth required
        "scopes_supported": [], // Empty = no scopes required
        "public_access": true
    }))
}