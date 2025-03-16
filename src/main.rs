// Initialize configuration
// Set up logging
// Create database connection pool
// Initialize cache
// Create shared state
// Start blockchain polling task
// Start HTTP server

// Only include modules unique to the binary
// Remove duplicates that exist in lib.rs

// Add these imports instead
use chain_data_service::{
    api, blockchain, cache, config, db, models, service, validation,
    // Any other modules you need
};

use std::sync::Arc;
use config::Config;
use tokio::sync::Mutex;
use cache::TransactionCache;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
//use chain_data_service::validation::validate_solana_address;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    // tracing_subscriber::registry()
    //     .with(tracing_subscriber::EnvFilter::new(
    //         std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
    //     ))
    //     .with(tracing_subscriber::fmt::layer())
    //     .init();

    // tracing::info!("Starting chain-data-service");

    // // Load configuration
    // let config = Config::from_env();
    // tracing::info!("Configuration loaded: {:?}", config);

    // // Setup database connection
    // let db_pool = db::connection::establish_connection().await?;
    // tracing::info!("Database connection established");

    // // Run a simple query to ensure the database is properly initialized
    // sqlx::query("SELECT 1").execute(&db_pool).await?;
    // tracing::info!("Database connection verified");

    // // Initialize cache
    // let cache = cache::init_cache(&config);
    // tracing::info!("Cache initialized with TTL: {:?} and capacity: {}", 
    //               config.cache_ttl, config.cache_max_capacity);

    // // Create shared state
    // let app_state = Arc::new(AppState {
    //     config: config.clone(),
    //     db_pool: db_pool.clone(),
    //     cache: Arc::new(Mutex::new(cache)),
    // });

    // // Start blockchain polling task
    // let polling_state = app_state.clone();
    // tokio::spawn(async move {
    //     blockchain::polling::start_polling(polling_state).await;
    // });
    // tracing::info!("Blockchain polling task started");

    // // Start HTTP server
    // let app = api::create_router(app_state);
    // let addr = format!("{}:{}", config.server_host, config.server_port);
    // tracing::info!("Starting server on {}", addr);
    
    // axum::Server::bind(&addr.parse()?)
    //     .serve(app.into_make_service())
    //     .await?;

    //let db_pool = db::connection::establish_connection().await?;
    //let db_pool = db::connection::establish_connection().await?;
    
    Ok(())
}

pub struct AppState {
    pub config: Config,
    pub db_pool: sqlx::SqlitePool,
    pub cache: Arc<Mutex<TransactionCache>>,
}
