use logical_engine::application::controllers::{
    mcp_controller::LogicalInferenceServer,
    auth_controller::{
        oauth_authorization_server_discovery, oauth_authorize_handler,
        oauth_token_handler, oauth_register_handler, oauth_protected_resource_discovery,
    },
    health_controller::{health_handler, root_handler},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};
use axum::{
    routing::{get, post},
};
use tower::ServiceBuilder;

const BIND_ADDRESS: &str = "0.0.0.0:8080";

fn main() -> anyhow::Result<()> {
    // Build tokio runtime explicitly
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    println!("🔐 Initializing Dummy Authentication System");
    println!("   - Always allows access (dummy auth for MCP compatibility)");
    println!("   - Real security via MCP session isolation");

    // Enable StreamableHttpService - each session gets its own server instance
    let service = StreamableHttpService::new(
        || Ok(LogicalInferenceServer::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    // Health router
    let health_routes = axum::Router::new()
        .route("/health", get(health_handler));

    // Create well-known discovery routes and OAuth endpoints
    let well_known_routes = axum::Router::new()
        .route("/oauth-authorization-server", get(oauth_authorization_server_discovery))
        .route("/oauth-protected-resource/mcp", get(oauth_protected_resource_discovery));

    // OAuth endpoints
    let oauth_routes = axum::Router::new()
        .route("/authorize", get(oauth_authorize_handler))
        .route("/token", post(oauth_token_handler))
        .route("/register", post(oauth_register_handler));

    // Set up a permissive CORS layer for development
    let cors_layer = ServiceBuilder::new().layer(
        tower_http::cors::CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any),
    );

    // Prepare the main routes - MCP service WITHOUT authentication (handles its own session management)
    let router = axum::Router::new()
        .nest_service("/mcp", service)  // MCP service handles its own auth via session management
        .nest("/.well-known", well_known_routes)
        .nest("/oauth", oauth_routes)
        .merge(health_routes)
        .route("/", get(root_handler));

    let tcp_listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;

    println!("🚀 Logical Inference MCP Server starting on {}", BIND_ADDRESS);
    println!("📡 MCP endpoint: http://{}/mcp", BIND_ADDRESS);
    println!("🔑 Auth endpoints:");
    println!("   - POST /oauth/token - Get authentication token");
    println!("   - GET  /oauth/validate - Validate token");
    println!("   - GET  /oauth/sessions - List active sessions");
    println!("   - GET  /oauth/info - Authentication system info");
    println!("🔍 Discovery endpoints:");
    println!("   - GET  /.well-known/oauth-authorization-server");
    println!("   - GET  /.well-known/oauth-protected-resource/mcp");
    println!("🏥 Health check: http://{}/health", BIND_ADDRESS);
    println!("🧠 Nemo engine: main branch (git)");

    // Apply the CORS layer to all routes
    let app = router.layer(cors_layer);
    let _ = axum::serve(tcp_listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
            println!("\n⚠️  Graceful shutdown signal received...");
            println!("✅ Server shutdown complete");
            std::process::exit(0);
        })
        .await;

    Ok(())
}
