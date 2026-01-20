mod config;
mod db;
mod error;
mod proxy;
mod stats;

use axum::{
    Router,
    routing::{any, get},
};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lms_metrics_proxy=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = config::Config::from_env()?;
    tracing::info!(
        "Starting token counter proxy on port {} with LM Studio at {}",
        config.port,
        config.lm_studio_url
    );

    // Initialize database
    // Parse the database URL to extract the file path and ensure parent directory exists
    let db_path = config
        .database_url
        .strip_prefix("sqlite:")
        .unwrap_or(&config.database_url);
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("{}?mode=rwc", config.database_url))
        .await?;

    db::init_db(&db).await?;
    tracing::info!("Database initialized at {}", config.database_url);

    // Create HTTP client
    let client = proxy::create_client();

    // Create shared state
    let state = Arc::new(proxy::AppState {
        config: config.clone(),
        db,
        client,
    });

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(stats::health_check))
        // Statistics endpoints
        .route("/stats/summary", get(stats::get_summary))
        .route("/stats/by-model", get(stats::get_by_model))
        .route("/stats/recent", get(stats::get_recent))
        // Proxy endpoints - catch all /v1/* routes with any HTTP method
        .route("/v1/{*path}", any(proxy::proxy_handler))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    tracing::info!("Proxy server listening on 0.0.0.0:{}", config.port);

    axum::serve(listener, app).await?;

    Ok(())
}
