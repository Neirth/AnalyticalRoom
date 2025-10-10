use deep_analytics::controllers::{
    mcp_controller::TreeEngineServer,
    auth_controller::{
        oauth_authorization_server_discovery, oauth_authorize_handler,
        oauth_token_handler, oauth_register_handler, oauth_protected_resource_discovery,
    },
    health_controller::{health_handler, root_handler},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::transport::stdio;
use rmcp::serve_server;
use axum::{
    routing::{get, post},
};
use tower::ServiceBuilder;
use clap::{Parser, Subcommand};

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8080;

#[derive(Parser)]
#[command(name = "deep_analytics")]
#[command(about = "Deep Analytics MCP Server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Host address to bind to (default: 0.0.0.0)
    #[arg(long, default_value = DEFAULT_HOST)]
    host: String,
    
    /// Port to bind to (default: 8080)
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
}

#[derive(Subcommand)]
enum Commands {
    /// Run server in stdio mode
    Stdio,
    /// Run server in HTTP mode (default)
    Http {
        /// Host address to bind to
        #[arg(long)]
        host: Option<String>,
        
        /// Port to bind to
        #[arg(long)]
        port: Option<u16>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    // Determine the mode and configuration
    let (mode, host, port) = match cli.command {
        Some(Commands::Stdio) => ("stdio", String::new(), 0),
        Some(Commands::Http { host: cmd_host, port: cmd_port }) => {
            ("http", cmd_host.unwrap_or(cli.host.clone()), cmd_port.unwrap_or(cli.port))
        }
        None => ("http", cli.host, cli.port), // Default to HTTP mode
    };
    
    if mode == "stdio" {
        // Run in stdio mode
        run_stdio_mode()
    } else {
        // Run in HTTP mode
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(run_http_mode(&host, port))
    }
}

fn run_stdio_mode() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    
    rt.block_on(async {
        println!("🚀 Deep Analytics MCP Server starting in stdio mode");
        
        let server = TreeEngineServer::new();
        let transport = stdio();
        
        let running_service = serve_server(server, transport).await?;
        let _ = running_service.waiting().await;
        
        Ok(())
    })
}

async fn run_http_mode(host: &str, port: u16) -> anyhow::Result<()> {
    let bind_address = format!("{}:{}", host, port);
    
    println!("🔐 Initializing Dummy Authentication System");
    println!("   - Always allows access (dummy auth for MCP compatibility)");
    println!("   - Real security via MCP session isolation");

    // Enable StreamableHttpService - each session gets its own server instance
    let service = StreamableHttpService::new(
        || Ok(TreeEngineServer::new()),
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

    let tcp_listener = tokio::net::TcpListener::bind(&bind_address).await?;

    println!("🚀 Deep Analytics MCP Server starting on {}", bind_address);
    println!("📡 MCP endpoint: http://{}/mcp", bind_address);
    println!("🔑 Auth endpoints:");
    println!("   - POST /oauth/token - Get authentication token");
    println!("   - GET  /oauth/validate - Validate token");
    println!("   - GET  /oauth/sessions - List active sessions");
    println!("   - GET  /oauth/info - Authentication system info");
    println!("🔍 Discovery endpoints:");
    println!("   - GET  /.well-known/oauth-authorization-server");
    println!("   - GET  /.well-known/oauth-protected-resource/mcp");
    println!("🏥 Health check: http://{}/health", bind_address);

    // Guard and run the server
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
