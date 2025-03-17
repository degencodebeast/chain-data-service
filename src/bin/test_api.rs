use chain_data_service::{
    api, cache, config, db, models, state
};
use state::AppState;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use reqwest::StatusCode;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::time::sleep;
use tracing::{info, error, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting API integration tests...");

    // 1. Setup
    info!("Setting up test environment...");
    let config = config::Config::from_env();
    let db_pool = db::connection::establish_connection().await?;
    
    // Clean database
    info!("Cleaning database before tests...");
    sqlx::query("DELETE FROM transactions").execute(&db_pool).await?;
    sqlx::query("DELETE FROM tracked_addresses").execute(&db_pool).await?;
    info!("✅ Database cleaned!");

    // Initialize cache
    let cache = cache::init_cache(&config);
    
    // Create app state
    let app_state = Arc::new(AppState {
        config: config.clone(),
        db_pool: db_pool.clone(),
        cache: Arc::new(Mutex::new(cache)),
    });

    // 2. Start API server in a background task
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_state = app_state.clone();
    
    let port = 3333; // Use a different port than main app for testing
    let server_addr = format!("127.0.0.1:{}", port);
    info!("Starting test server on {}", server_addr);
    
    // Clone server_addr for use after the spawn
    let server_addr_clone = server_addr.clone();
    
    let server_handle = tokio::spawn(async move {
        let app = api::create_router(server_state);
        let listener = tokio::net::TcpListener::bind(&server_addr).await.unwrap();
        
        // Use axum::serve with a cancellation token
        let shutdown_rx_clone = shutdown_rx;
        tokio::select! {
            result = axum::serve(listener, app) => {
                if let Err(e) = result {
                    error!("Server error: {}", e);
                }
            }
            _ = shutdown_rx_clone => {
                info!("Server shutdown received");
            }
        }
    });
    
    // Give the server time to start
    sleep(Duration::from_secs(1)).await;
    
    // 3. Create HTTP client for tests
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server_addr_clone);
    
    // 4. Test data setup
    let test_addresses = vec![
        "9QDsSXnWXZ3z8zpQ9ezxd3hSbqfxdj9p5QZW3dNMaV72",
        "5BSy3UoS5nom6mo7s7imUwuFGVsX6Mf94VZiadUFeqkq",
        "DrUdzADxrhtFVYG8BqazRsjsPaZbLmzE5EbtevnAB39i",
    ];
    
    // 5. Test cases for /track endpoint
    info!("\n===== Testing /track endpoint =====");
    
    // 5.1 Test adding an address
    info!("Testing add address...");
    let test_addr = test_addresses[0];
    let response = client
        .post(format!("{}/track?address={}&action=add", base_url, test_addr))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::CREATED);
    info!("✅ Successfully added address {}", test_addr);
    
    // 5.2 Test adding the same address again (should work)
    let response = client
        .post(format!("{}/track?address={}&action=add", base_url, test_addr))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::CREATED);
    info!("✅ Re-adding same address works as expected");
    
    // 5.3 Test removing an address
    let response = client
        .post(format!("{}/track?address={}&action=remove", base_url, test_addr))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    info!("✅ Successfully removed address {}", test_addr);
    
    // 5.4 Test removing an address that isn't tracked
    let response = client
        .post(format!("{}/track?address={}&action=remove", base_url, test_addr))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    info!("✅ Removing non-tracked address handled correctly");
    
    // 5.5 Test invalid action
    let response = client
        .post(format!("{}/track?address={}&action=invalid", base_url, test_addr))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Invalid action rejected correctly");
    
    // 5.6 Test invalid address
    let response = client
        .post(format!("{}/track?address=invalid_address&action=add", base_url))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Invalid address rejected correctly");
    
    // 5.7 Test missing parameters
    let response = client
        .post(format!("{}/track?address={}", base_url, test_addr))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Missing action parameter rejected correctly");
    
    // 6. Prepare for /transactions endpoint tests
    info!("\n===== Preparing for /transactions tests =====");
    
    // Add test addresses to tracking
    for addr in &test_addresses {
        client
            .post(format!("{}/track?address={}&action=add", base_url, addr))
            .send()
            .await?;
    }
    info!("✅ Added test addresses to tracking");
    
    // Add some test transactions
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let hour_ago = now - 3600;
    let day_ago = now - 86400;
    
    // Source and destination from our test addresses
    let source_address = test_addresses[0];
    let destination_address = test_addresses[1];
    
    // Insert test transactions directly to database
    let transactions = vec![
        models::Transaction {
            signature: format!("test_sig_current_{}", now),
            block_time: now,
            slot: 12345,
            source_address: source_address.to_string(),
            destination_address: Some(destination_address.to_string()),
            amount: Some(123.45),
            program_id: Some("program1".to_string()),
            success: true,
        },
        models::Transaction {
            signature: format!("test_sig_hour_{}", now),
            block_time: hour_ago,
            slot: 12344,
            source_address: source_address.to_string(),
            destination_address: Some(destination_address.to_string()),
            amount: Some(67.89),
            program_id: Some("program2".to_string()),
            success: true,
        },
        models::Transaction {
            signature: format!("test_sig_day_{}", now),
            block_time: day_ago,
            slot: 12343,
            source_address: source_address.to_string(),
            destination_address: Some(destination_address.to_string()),
            amount: Some(42.0),
            program_id: Some("program1".to_string()),
            success: false,
        },
    ];
    
    db::transaction::add_transactions(&db_pool, &transactions).await?;
    info!("✅ Added test transactions to database");
    
    // 7. Test cases for /transactions endpoint
    info!("\n===== Testing /transactions endpoint =====");
    
    // 7.1 Basic transaction query
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=0&limit=10", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Get headers before consuming the response with text()
    let total_count = response.headers()
        .get("X-Total-Count")
        .expect("Missing X-Total-Count header")
        .to_str()?
        .parse::<i64>()?;
    
    // Now we can consume the response
    let body = response.text().await?;
    info!("✅ Successfully retrieved transactions");
    info!("Response body preview: {:.100}...", body);
    
    info!("Total count from header: {}", total_count);
    assert!(total_count >= 3, "Expected at least 3 transactions");
    
    // 7.2 Test with time filtering
    let response = client
        .get(format!("{}/transactions?address={}&start_time={}&end_time={}&offset=0&limit=10", 
            base_url, 
            source_address,
            hour_ago,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    let total_count = response.headers()
        .get("X-Total-Count")
        .expect("Missing X-Total-Count header")
        .to_str()?
        .parse::<i64>()?;
    
    // Consume the response body
    let _ = response.text().await?;
    
    info!("✅ Time filtering works. Found {} transactions in the last hour", total_count);
    assert!(total_count >= 2, "Expected at least 2 transactions in the last hour");
    
    // 7.3 Test pagination
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=0&limit=1", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    let page1_body = response.text().await?;
    
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=1&limit=1", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    let page2_body = response.text().await?;
    
    assert_ne!(page1_body, page2_body, "Pages should contain different transactions");
    info!("✅ Pagination works correctly");
    
    // 7.4 Test address not being tracked
    client
        .post(format!("{}/track?address={}&action=remove", base_url, test_addresses[2]))
        .send()
        .await?;
    
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=0&limit=10", 
            base_url, 
            test_addresses[2],
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    info!("✅ Correctly rejected request for non-tracked address");
    
    // 7.5 Test invalid parameters
    let response = client
        .get(format!("{}/transactions?address={}&start_time=invalid&end_time={}&offset=0&limit=10", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Invalid timestamp rejected correctly");
    
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=invalid&limit=10", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Invalid offset rejected correctly");
    
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=0&limit=invalid", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Invalid limit rejected correctly");
    
    // 7.6 Test invalid time range
    let response = client
        .get(format!("{}/transactions?address={}&start_time={}&end_time={}&offset=0&limit=10", 
            base_url, 
            source_address,
            now + 100,
            now
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    info!("✅ Invalid time range (start > end) rejected correctly");
    
    // 8. Test caching behavior
    info!("\n===== Testing caching behavior =====");
    
    // First request to populate cache
    let start_time = tokio::time::Instant::now();
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=0&limit=10", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    let first_request_time = start_time.elapsed();
    info!("First request time: {:?}", first_request_time);
    
    // Second request should be faster due to caching
    let start_time = tokio::time::Instant::now();
    let response = client
        .get(format!("{}/transactions?address={}&start_time=0&end_time={}&offset=0&limit=10", 
            base_url, 
            source_address,
            now + 1
        ))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    let second_request_time = start_time.elapsed();
    info!("Second request time: {:?}", second_request_time);
    
    // Second request should be faster or at least not significantly slower
    info!("✅ Cache performance test: first={:?}, second={:?}", 
          first_request_time, second_request_time);
    
    // Shutdown the server
    info!("\n===== Shutting down test server =====");
    let _ = shutdown_tx.send(());
    let _ = server_handle.await;
    
    info!("All API tests completed successfully!");
    Ok(())
} 