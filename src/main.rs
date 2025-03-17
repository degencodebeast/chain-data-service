use chain_data_service::{
    api, 
    blockchain::{
        self,
        worker_pool::WorkerPool,
        batch_manager::{BatchManager, BatchConfig}
    },
    cache, 
    config, 
    db, 
    models, 
    service, 
    validation, 
    state
    // Any other modules you need
};

use state::AppState;
use std::sync::Arc;
use config::Config;
use tokio::sync::Mutex;
use cache::TransactionCache;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use num_cpus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting chain-data-service");

    // Load configuration
    let config = Config::from_env();
    tracing::info!("Configuration loaded: {:?}", config);

    // Setup database connection
    let db_pool = db::connection::establish_connection().await?;
    tracing::info!("Database connection established");

    // Run a simple query to ensure the database is properly initialized
    sqlx::query("SELECT 1").execute(&db_pool).await?;
    tracing::info!("Database connection verified");

    // Initialize cache
    let cache = cache::init_cache(&config);
    tracing::info!("Cache initialized with TTL: {:?} and capacity: {}", 
                  config.cache_ttl, config.cache_max_capacity);

    // Create shared state
    let app_state = Arc::new(AppState {
        config: config.clone(),
        db_pool: db_pool.clone(),
        cache: Arc::new(Mutex::new(cache)),
    });

    // Initialize worker pool with proper number of cores
    let worker_pool = WorkerPool::new(app_state.clone(), num_cpus::get());
    let worker_sender = worker_pool.get_sender();

    // Initialize batch manager with default config
    let batch_config = BatchConfig::default();
    let batch_manager = BatchManager::new(
        app_state.clone(),
        batch_config,
        worker_sender.clone(), // Clone sender for batch manager
    );

    // Start batch manager in background
    let batch_manager_handle = tokio::spawn(async move {
        batch_manager.start().await;
    });

    // Start blockchain polling with worker sender
    let polling_state = app_state.clone();
    tokio::spawn(async move {
        blockchain::polling::start_polling(polling_state).await;
    });
    tracing::info!("Blockchain polling task started");

    // Start HTTP server
    let app = api::create_router(app_state);
    let addr = format!("{}:{}", config.server_host, config.server_port);
    tracing::info!("Starting server on {}", addr);
    
    // Create a TCP listener
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);
    
    // axum::Server::bind(&addr.parse()?)
    //     .serve(app.into_make_service())
    //     .await?;
    // Serve the application
    axum::serve(listener, app).await?;

    Ok(())
}
