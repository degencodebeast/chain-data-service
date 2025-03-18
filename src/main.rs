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
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use num_cpus;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting chain-data-service");

    // Load configuration
    let config = Config::from_env();
    info!("Configuration loaded: {:?}", config);

    // Setup database connection
    let db_pool = db::connection::establish_connection().await?;
    info!("Database connection established");

    // Run a simple query to ensure the database is properly initialized
    sqlx::query("SELECT 1").execute(&db_pool).await?;
    info!("Database connection verified");

    // Initialize cache
    let cache = cache::init_cache(&config);
    info!("Cache initialized with TTL: {:?} and capacity: {}", 
          config.cache_ttl, config.cache_max_capacity);

    // Create shared state
    let app_state = Arc::new(AppState {
        config: config.clone(),
        db_pool: db_pool.clone(),
        cache: Arc::new(Mutex::new(cache)),
    });

    // Main shutdown token
    let shutdown = CancellationToken::new();

    // Initialize worker pool with proper number of cores
    let worker_pool = WorkerPool::new(app_state.clone(), num_cpus::get());
    let worker_sender = worker_pool.get_sender();
    let worker_shutdown = shutdown.clone();

    // Initialize batch manager with default config
    let batch_config = BatchConfig::default();
    let mut batch_manager = BatchManager::new(
        app_state.clone(),
        batch_config,
        worker_sender.clone(),
    );
    let batch_shutdown = shutdown.clone();

    // Start batch manager in background
    let batch_handle = tokio::spawn(async move {
        batch_manager.start(batch_shutdown).await;
    });

    // Start blockchain polling with worker sender
    let polling_state = app_state.clone();
    let polling_shutdown = shutdown.clone();
    let polling_handle = tokio::spawn(async move {
        blockchain::polling::start_polling(polling_state, polling_shutdown).await;
    });
    info!("Blockchain polling task started");

    // Start HTTP server
    let app = api::create_router(app_state);
    let addr = format!("{}:{}", config.server_host, config.server_port);
    info!("Starting server on {}", addr);
    
    // Create a TCP listener
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Server listening on {}", addr);
    
    let server_shutdown = shutdown.clone();
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                server_shutdown.cancelled().await;
            })
            .await
            .expect("Server error");
    });
    
    // Wait for shutdown signal
    tokio::select! {
        _ = shutdown_signal() => {
            info!("Shutdown signal received, starting graceful shutdown");
        }
    }
    
    // Initiate shutdown
    info!("Initiating graceful shutdown sequence");
    shutdown.cancel();
    
    // Wait for components to shut down (optional timeout)
    let shutdown_timeout = tokio::time::Duration::from_secs(10);
    tokio::select! {
        _ = async {
            // Wait for all components to complete
            let _ = tokio::join!(polling_handle, batch_handle, server_handle);
        } => {
            info!("All components shut down successfully");
        }
        _ = tokio::time::sleep(shutdown_timeout) => {
            warn!("Shutdown timed out after {:?}, forcing exit", shutdown_timeout);
        }
    }
    
    info!("Blockchain transaction microservice stopped");
    Ok(())
}

// Signal handler for shutdown
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
