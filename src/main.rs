use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod db;
mod models;
mod sync;
mod utils;

use db::Database;
use sync::SyncService;

#[derive(Parser, Debug, Clone)]
#[command(name = "ergo-index")]
#[command(about = "Lightweight Ergo blockchain indexer and explorer API")]
pub struct Config {
    /// Ergo node URLs (comma-separated for parallel fetching)
    #[arg(long, env = "ERGO_NODES", default_value = "http://localhost:9053")]
    pub nodes: String,

    /// Database file path
    #[arg(long, env = "DATABASE_PATH", default_value = "./data/ergo-index.duckdb")]
    pub database: String,

    /// HTTP server port
    #[arg(long, env = "PORT", default_value = "8080")]
    pub port: u16,

    /// HTTP server host
    #[arg(long, env = "HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// Sync batch size (blocks per batch, higher is faster but uses more memory)
    #[arg(long, env = "SYNC_BATCH_SIZE", default_value = "50")]
    pub sync_batch_size: u32,

    /// Sync interval in seconds
    #[arg(long, env = "SYNC_INTERVAL", default_value = "10")]
    pub sync_interval: u64,

    /// Enable embedded node mode
    #[arg(long, env = "EMBEDDED_NODE", default_value = "false")]
    pub embedded_node: bool,

    /// Node API key (if required)
    #[arg(long, env = "NODE_API_KEY")]
    pub node_api_key: Option<String>,

    /// Network: mainnet or testnet
    #[arg(long, env = "NETWORK", default_value = "mainnet")]
    pub network: String,
}

pub struct AppState {
    pub db: Database,
    pub config: Config,
    pub sync_service: Arc<SyncService>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ergo_index=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::parse();

    tracing::info!("Starting ergo-index v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Nodes: {}", config.nodes);
    tracing::info!("Database: {}", config.database);
    tracing::info!("Network: {}", config.network);

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&config.database).parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Initialize database
    let db = Database::new(&config.database)?;
    db.migrate()?;
    tracing::info!("Database initialized");

    // Initialize sync service
    let node_urls: Vec<String> = config.nodes.split(',').map(|s| s.trim().to_string()).collect();
    let sync_service = Arc::new(SyncService::new(
        node_urls,
        db.clone(),
        config.sync_batch_size,
        config.node_api_key.clone(),
    ));

    // Start background sync
    let sync_handle = sync_service.clone();
    let sync_interval = config.sync_interval;
    tokio::spawn(async move {
        sync_handle.run(sync_interval).await;
    });

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        sync_service,
    });

    // Build router
    let app = Router::new()
        // API v1 routes
        .nest("/api/v1", api::routes(state.clone()))
        // Status endpoint
        .route("/status", get(api::status::get_status))
        // Health check
        .route("/health", get(|| async { "OK" }))
        // Swagger UI
        .merge(api::swagger::swagger_routes())
        // Static files for UI
        .nest_service("/", ServeDir::new("ui/static").append_index_html_on_directories(true))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting server at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
