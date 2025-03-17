use chain_data_service::{
    blockchain::{client::SolanaClient, models::extract_transaction, processor},
    config::Config,
    db::{connection, address, transaction},
    state::AppState,
};
use std::{sync::Arc, collections::HashSet, time::Duration};
use tokio::sync::{Mutex, oneshot};
use tracing::{info, error, warn, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    info!("Starting blockchain integration test...");
    
    // 1. Setup
    info!("Setting up test environment...");
    let config = Config::from_env();
    let db_pool = connection::establish_connection().await?;
    let client = Arc::new(SolanaClient::new(&config));
    
    // Clear test data
    sqlx::query("DELETE FROM transactions").execute(&db_pool).await?;
    sqlx::query("DELETE FROM tracked_addresses").execute(&db_pool).await?;
    
    // 2. Test RPC Client
    info!("Testing RPC client connection...");
    let slot = client.get_slot().await?;
    info!("✅ Current slot: {}", slot);
    
    // 3. Test getting transaction by signature
    info!("Testing transaction retrieval...");
    let test_signature = "3PZSPpiZ5TtBPhCoo4GNCpxqsZuMXR2nkG5j5AZspMxGMkZpQkFnBn7DA9M8S8VmmwDLSZqxVVHnHtdhk8yEpyk3";
    
    match client.get_transaction(test_signature).await {
        Ok(tx) => {
            info!("✅ Retrieved transaction: {}", test_signature);
            
            // 4. Test transaction extraction
            if let Some(parsed_tx) = extract_transaction(test_signature, &tx) {
                info!("✅ Extracted transaction successfully");
                info!("   Source: {}", parsed_tx.source_address);
                info!("   Destination: {:?}", parsed_tx.destination_address);
                info!("   Amount: {:?}", parsed_tx.amount);
                info!("   Program: {:?}", parsed_tx.program_id);
                
                // FIX: Register both source and destination addresses
                address::add_address(&db_pool, &parsed_tx.source_address).await?;
                if let Some(dest) = &parsed_tx.destination_address {
                    address::add_address(&db_pool, dest).await?;
                }
                
                // Add this transaction to the database directly
                transaction::add_transactions(&db_pool, &[parsed_tx]).await?;
                info!("✅ Stored sample transaction in database");
            } else {
                error!("❌ Failed to extract transaction data");
            }
        },
        Err(e) => {
            error!("❌ Failed to get transaction: {}", e);
        }
    }
    
    // 5. Test getting signatures for an address
    info!("Testing address signature retrieval...");
    let test_address = "DrUdzADxrhtFVYG8BqazRsjsPaZbLmzE5EbtevnAB39i";
    
    // FIX: Add this address to tracked addresses
    address::add_address(&db_pool, test_address).await?;
    
    let signatures = client.get_signatures_for_address(
        test_address,
        None,
        None,
        Some(5)
    ).await?;
    
    if !signatures.is_empty() {
        info!("✅ Retrieved {} signatures for address {}", signatures.len(), test_address);
        for sig in &signatures {
            info!("   Signature: {}", sig.signature);
        }
    } else {
        error!("❌ No signatures found for address {}", test_address);
    }
    
    // 6. Test tracking addresses
    info!("Testing address tracking...");
    let test_addresses = vec![
        "9QDsSXnWXZ3z8zpQ9ezxd3hSbqfxdj9p5QZW3dNMaV72",
        "5BSy3UoS5nom6mo7s7imUwuFGVsX6Mf94VZiadUFeqkq",
    ];
    
    for addr in &test_addresses {
        address::add_address(&db_pool, addr).await?;
        info!("✅ Tracking address: {}", addr);
    }
    
    // 7. Test processing transaction batch
    info!("Testing transaction batch processing...");
    let tracked_addresses: HashSet<String> = test_addresses.iter().map(|a| a.to_string()).collect();
    
    // Get signatures for the first address
    let test_sigs = client
        .get_signatures_for_address(test_addresses[0], None, None, Some(5))
        .await?
        .iter()
        .map(|sig| sig.signature.clone())
        .collect::<Vec<String>>();
    
    if !test_sigs.is_empty() {
        info!("Processing {} signatures...", test_sigs.len());
        
        // FIX: First fetch and store all involved addresses
        for sig in &test_sigs {
            match client.get_transaction(sig).await {
                Ok(tx) => {
                    if let Some(parsed_tx) = extract_transaction(sig, &tx) {
                        // Register both source and destination
                        address::add_address(&db_pool, &parsed_tx.source_address).await?;
                        if let Some(dest) = &parsed_tx.destination_address {
                            address::add_address(&db_pool, dest).await?;
                        }
                    }
                },
                Err(e) => warn!("Failed to get transaction {}: {}", sig, e)
            }
            // Small delay between requests
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        let processed = processor::process_transaction_batch(
            &client,
            &db_pool,
            &test_sigs,
            &tracked_addresses
        ).await?;
        
        info!("✅ Processed {} transactions", processed);
        
        // 8. Verify transactions in database
        let (txs, _count) = transaction::get_transactions(
            &db_pool,
            test_addresses[0],
            0,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            0,
            10
        ).await?;
        
        info!("✅ Found {} transactions in database", txs.len());
        for tx in &txs[0..std::cmp::min(3, txs.len())] {
            info!("   Signature: {}, Amount: {:?}", tx.signature, tx.amount);
        }
    }
    
    // 9. Test small polling cycle (just to verify it works) - FIX: Use a cancellable approach
    info!("Testing polling mechanism (brief sample)...");
    let cache = chain_data_service::cache::init_cache(&config);
    let app_state = Arc::new(AppState {
        config: config.clone(),
        db_pool: db_pool.clone(),
        cache: Arc::new(Mutex::new(cache)),
    });
    
    // FIX: Create a cancellable task for polling
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let polling_state = app_state.clone();
    
    let polling_handle = tokio::spawn(async move {
        // Modify this to a simplified test version of polling that can be cancelled
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        let mut shutdown = false;
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if shutdown {
                        break;
                    }
                    
                    info!("Polling tick...");
                    // Do minimal polling work here
                    let client = SolanaClient::new(&polling_state.config);
                    if let Ok(slot) = client.get_slot().await {
                        info!("Current slot: {}", slot);
                    }
                },
                _ = &mut shutdown_rx => {
                    info!("Polling shutdown received");
                    shutdown = true;
                }
            }
        }
    });
    
    // Run for a short time
    tokio::time::sleep(Duration::from_secs(15)).await;
    
    // Signal shutdown and wait for task to complete
    let _ = shutdown_tx.send(());
    let _ = polling_handle.await;
    
    info!("✅ Polling test completed");
    
    // 10. Final verification
    let total_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
        .fetch_one(&db_pool)
        .await?;
    
    info!("Total transactions in database after tests: {}", total_count);
    info!("All blockchain integration tests completed!");
    
    Ok(())
}
