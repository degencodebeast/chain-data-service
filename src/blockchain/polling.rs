use crate::blockchain::client::{SolanaClient, ClientError};
use crate::blockchain::models::extract_transaction;
use crate::db::{address, transaction};
use crate::models::Transaction;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, interval};
use tokio_util::sync::CancellationToken;
use backon::{Retryable, ExponentialBuilder};
use tracing::{debug, error, info, warn};
use crate::state::AppState;
use tokio::sync::mpsc;
use crate::blockchain::{worker_pool::WorkerPool, batch_manager::{BatchManager, BatchConfig}};
use num_cpus;

pub async fn start_polling(state: Arc<AppState>, shutdown: CancellationToken) {
    info!("Starting blockchain polling service");
    
    // Get configuration
    let config = &state.config;
    let db_pool = &state.db_pool;
    
    // Create Solana client
    let client = Arc::new(SolanaClient::new(config));
    
    // Initialize worker pool
    let worker_pool = WorkerPool::new(state.clone(), num_cpus::get());
    let worker_sender = worker_pool.get_sender();
    
    // Create BatchManager configuration
    let batch_config = BatchConfig {
        max_batch_size: 100, // Adjust based on your needs
        flush_interval: Duration::from_secs(5),
        worker_count: num_cpus::get(),
    };
    
    // Create batch manager
    let batch_manager = Arc::new(tokio::sync::Mutex::new(
        BatchManager::new(state.clone(), batch_config, worker_sender)
    ));
    
    // Start batch manager in background task
    let batch_manager_for_task = batch_manager.clone();
    let batch_shutdown = shutdown.clone();
    tokio::spawn(async move {
        // Lock only briefly to get a clone of what we need, then release
        {
            let manager = batch_manager_for_task.lock().await;
            // Call start directly on the manager without holding the lock for the duration
            manager.start(batch_shutdown).await;
        }
    });
    
    // Set polling interval
    let polling_interval = Duration::from_secs(config.polling_interval_secs);
    
    // Start historical transaction polling with batch manager
    let historical_state = state.clone();
    let historical_client = client.clone();
    let historical_batch_manager = batch_manager.clone();
    let historical_shutdown = shutdown.clone();
    
    let historical_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match poll_historical_transactions(
                        &historical_client, 
                        &historical_state.db_pool,
                        &historical_batch_manager
                    ).await {
                        Ok(count) => {
                            if count > 0 {
                                info!("Processed {} historical transactions", count);
                            }
                        },
                        Err(e) => error!("Error polling historical transactions: {}", e),
                    }
                }
                _ = historical_shutdown.cancelled() => {
                    info!("Shutting down historical transaction polling");
                    break;
                }
            }
        }
    });
    
    // Get initial slot with retry
    let current_slot = match client.get_slot().await {
        Ok(slot) => slot,
        Err(e) => {
            error!("Failed to get initial slot: {}", e);
            // Default to some value and we'll catch up
            100_000
        }
    };
    
    info!("Starting blockchain polling from slot {}", current_slot);
    
    // Main polling loop with slot tracking
    let mut current_slot = current_slot;
    let mut ticker = interval(polling_interval);
    
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Get latest slot
                let latest_slot = match client.get_slot().await {
                    Ok(slot) => slot,
                    Err(e) => {
                        error!("Failed to get latest slot: {}", e);
                        continue;
                    }
                };
                
                // Check if we have new blocks
                if latest_slot <= current_slot {
                    continue;
                }
                
                debug!("Polling new transactions from slot {} to {}", current_slot, latest_slot);
                
                // Process new transactions with batch manager
                match poll_new_transactions(
                    &client, 
                    &db_pool, 
                    current_slot, 
                    latest_slot, 
                    &batch_manager
                ).await {
                    Ok(count) => {
                        // Log even when no transactions are found
                        debug!("Checked slots {}-{}: found {} transactions", 
                               current_slot, latest_slot, count);
                        current_slot = latest_slot;
                    },
                    Err(e) => error!("Error polling new transactions: {}", e),
                }
            }
            _ = shutdown.cancelled() => {
                info!("Shutting down blockchain polling service");
                break;
            }
        }
    }
    
    // Wait for historical polling to complete
    let _ = historical_handle.await;
}

/// Poll for historical transactions for all tracked addresses
async fn poll_historical_transactions(
    client: &SolanaClient,
    db_pool: &SqlitePool,
    batch_manager: &Arc<tokio::sync::Mutex<BatchManager>>,
) -> Result<usize, ClientError> {
    // Get all tracked addresses
    let addresses = match address::get_all_tracked_addresses(db_pool).await {
        Ok(addrs) => addrs,
        Err(e) => {
            error!("Failed to get tracked addresses: {}", e);
            return Ok(0);
        }
    };
    
    if addresses.is_empty() {
        return Ok(0);
    }
    
    let mut total_processed = 0;
    
    // Process each address
    for addr in addresses {
        match process_historical_address(client, db_pool, &addr, batch_manager).await {
            Ok(count) => total_processed += count,
            Err(e) => error!("Error processing historical transactions for {}: {}", addr, e),
        }
    }
    
    Ok(total_processed)
}

/// Process historical transactions for a single address
async fn process_historical_address(
    client: &SolanaClient,
    db_pool: &SqlitePool,
    address: &str,
    batch_manager: &Arc<tokio::sync::Mutex<BatchManager>>,
) -> Result<usize, ClientError> {
    debug!("Processing historical transactions for {}", address);
    
    // Set a reasonable limit for historical transactions
    let limit = 50;
    let mut before = None;
    let mut total_processed = 0;
    
    // Track seen signatures to avoid duplicates
    let mut seen_signatures = HashSet::new();
    
    // Get the current timestamp
    let now = SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_secs() as i64;
    
    // Maximum age of transactions to process (90 days)
    let max_age_secs = 60 * 60 * 24 * 90;
    
    loop {
        // Get signatures for address (already has retry logic in client)
        let signatures = match client.get_signatures_for_address(
            address,
            before.as_deref(),
            None,
            Some(limit),
        ).await {
            Ok(sigs) => sigs,
            Err(e) => {
                warn!("Failed to get signatures for {}: {}", address, e);
                break;
            }
        };
        
        if signatures.is_empty() {
            break;
        }
        
        // Update pagination cursor
        before = signatures.last().map(|sig| sig.signature.clone());
        
        // Check for age limit
        let oldest_tx_time = signatures.last()
            .and_then(|sig| sig.block_time)
            .unwrap_or(0);
        
        if now - oldest_tx_time > max_age_secs {
            debug!("Reached age limit for historical transactions for {}", address);
            break;
        }
        
        // Filter out signatures we've already seen
        let new_signatures: Vec<_> = signatures.iter()
            .filter(|sig| !seen_signatures.contains(&sig.signature))
            .map(|sig| sig.signature.clone())
            .collect();
        
        if new_signatures.is_empty() {
            break;
        }
        
        // Add to seen set
        for sig in &new_signatures {
            seen_signatures.insert(sig.clone());
        }
        
        // Process this batch of transactions with batch manager
        let batch_result = process_transaction_signatures(client, db_pool, &new_signatures, batch_manager).await?;
        total_processed += batch_result;
        
        // Avoid rate limiting
        sleep(Duration::from_millis(200)).await;
        
        // If we got less than the limit, we've reached the end
        if signatures.len() < limit {
            break;
        }
    }
    
    Ok(total_processed)
}

/// Poll for new transactions across all tracked addresses by checking recent blocks
async fn poll_new_transactions(
    client: &SolanaClient,
    db_pool: &SqlitePool,
    start_slot: u64,
    end_slot: u64,
    batch_manager: &Arc<tokio::sync::Mutex<BatchManager>>,
) -> Result<usize, ClientError> {
    // Log polling activity so you can see it's working
    debug!("Polling for new transactions in slots {}-{}", start_slot, end_slot);
    
    // Get all tracked addresses
    let addresses = match address::get_all_tracked_addresses(db_pool).await {
        Ok(addrs) => {
            debug!("Found {} tracked addresses", addrs.len());
            addrs
        },
        Err(e) => {
            error!("Failed to get tracked addresses: {}", e);
            return Ok(0);
        }
    };
    
    if addresses.is_empty() {
        return Ok(0);
    }
    
    // Convert to HashSet for faster lookups
    let tracked_addresses: HashSet<_> = addresses.into_iter().collect();
    
    // Process in smaller ranges to avoid overloading the RPC
    let range_size = 10;
    let mut total_processed = 0;
    
    for batch_start in (start_slot..=end_slot).step_by(range_size) {
        let batch_end = (batch_start + range_size as u64 - 1).min(end_slot);
        
        // Get signatures for each address in this slot range
        let mut all_signatures = Vec::new();
        
        for address in &tracked_addresses {
            // Get signatures (client already implements retry)
            match client.get_signatures_for_address(
                address,
                None,
                None,
                Some(100), // Reasonable limit
            ).await {
                Ok(signatures) => {
                    // Filter by slot range
                    let filtered_sigs: Vec<_> = signatures.iter()
                        .filter(|sig| {
                            let slot = sig.slot;
                            slot >= batch_start && slot <= batch_end
                        })
                        .map(|sig| sig.signature.clone())
                        .collect();
                    
                    all_signatures.extend(filtered_sigs);
                },
                Err(e) => {
                    warn!("Failed to get signatures for {}: {}", address, e);
                    continue;
                }
            }
            
            // Small delay to avoid rate limiting
            sleep(Duration::from_millis(100)).await;
        }
        
        // Remove duplicates
        all_signatures.sort_unstable();
        all_signatures.dedup();
        
        // Process batch with batch manager instead of worker_sender
        if !all_signatures.is_empty() {
            match process_transaction_signatures(client, db_pool, &all_signatures, batch_manager).await {
                Ok(count) => total_processed += count,
                Err(e) => error!("Error processing transaction batch: {}", e),
            }
        }
    }
    
    // Before returning, log the results
    debug!("Found {} new transactions in slots {}-{}", 
           total_processed, start_slot, end_slot);
    
    Ok(total_processed)
}

/// Process a batch of transaction signatures
async fn process_transaction_signatures(
    client: &SolanaClient,
    db_pool: &SqlitePool,
    signatures: &[String],
    batch_manager: &Arc<tokio::sync::Mutex<BatchManager>>,
) -> Result<usize, ClientError> {
    if signatures.is_empty() {
        return Ok(0);
    }
    
    let mut processed = 0;
    
    // Process transactions in batches
    let batch_size = 10;
    for chunk in signatures.chunks(batch_size) {
        let mut chunk_transactions = Vec::new();
        
        let futures: Vec<_> = chunk.iter().map(|sig| {
            let sig_clone = sig.clone();
            let client_clone = client.clone();
            
            async move {
                client_clone.get_transaction(&sig_clone).await
            }
        }).collect();
        
        for (i, result) in futures::future::join_all(futures).await.into_iter().enumerate() {
            match result {
                Ok(tx) => {
                    if let Some(transaction) = extract_transaction(&chunk[i], &tx) {
                        chunk_transactions.push(transaction);
                    }
                },
                Err(e) => {
                    warn!("Failed to get transaction {}: {}", chunk[i], e);
                }
            }
        }
        
        // Add each transaction to batch manager, holding the lock only briefly each time
        for transaction in chunk_transactions {
            // Only hold the lock while adding the transaction
            {
                let manager = batch_manager.lock().await;
                manager.add_transaction(transaction).await;
                processed += 1;
            }
        }
        
        // Rate limiting
        sleep(Duration::from_millis(200)).await;
    }
    
    debug!("Added {} transactions to batch manager", processed);
    Ok(processed)
}

