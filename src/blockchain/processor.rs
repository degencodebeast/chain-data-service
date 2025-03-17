use crate::blockchain::client::{SolanaClient, ClientError};
use crate::blockchain::models::extract_transaction;
use crate::models::Transaction;
use crate::db::address;  // Import the address module
use sqlx::SqlitePool;
use tracing::{debug, error, warn};
//use futures::stream::{self, StreamExt};
use std::collections::HashSet;
use tokio::time::sleep;
use std::time::Duration;

/// Process a batch of transactions
pub async fn process_transaction_batch(
    client: &SolanaClient, 
    db_pool: &SqlitePool,
    signatures: &[String],
    tracked_addresses: &HashSet<String>
) -> Result<usize, ClientError> {
    if signatures.is_empty() {
        return Ok(0);
    }
    
    let mut processed = 0;
    let mut transactions = Vec::new();
    
    // Process in smaller batches to avoid memory issues
    let batch_size = 10;
    for chunk in signatures.chunks(batch_size) {
        let mut chunk_transactions = Vec::new();
        
        for sig in chunk {
            match client.get_transaction(sig).await {
                Ok(tx) => {
                    if let Some(transaction) = extract_transaction(sig, &tx) {
                        // Check if transaction involves tracked addresses
                        if is_transaction_relevant(&transaction, tracked_addresses) {
                            chunk_transactions.push(transaction);
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to get transaction {}: {}", sig, e);
                    continue;
                }
            }
            
            // Small delay between requests to avoid rate limiting
            sleep(Duration::from_millis(100)).await;
        }
        
        transactions.extend(chunk_transactions);
        
        // Avoid overloading the client with too many requests at once
        sleep(Duration::from_millis(200)).await;
    }
    
    // Store transactions in database
    if !transactions.is_empty() {
        // First ensure all addresses are registered in the database
        let mut addresses_to_register = HashSet::new();
        
        for tx in &transactions {
            // Add source address
            addresses_to_register.insert(tx.source_address.clone());
            
            // Add destination address if present
            if let Some(dest) = &tx.destination_address {
                addresses_to_register.insert(dest.clone());
            }
        }
        
        // Register addresses using the existing function in address.rs
        for address_str in addresses_to_register {
            match address::add_address(db_pool, &address_str).await {
                Ok(_) => debug!("Ensured address is tracked: {}", address_str),
                Err(e) => {
                    error!("Failed to register address {}: {}", address_str, e);
                    // Continue with other addresses rather than failing completely
                }
            }
        }
        
        // Now store the transactions
        match crate::db::transaction::add_transactions(db_pool, &transactions).await {
            Ok(_) => {
                processed = transactions.len();
                debug!("Successfully stored {} transactions", processed);
            },
            Err(e) => {
                error!("Failed to store transactions: {}", e);
            }
        }
    }
    
    Ok(processed)
}

/// Check if a transaction involves tracked addresses
fn is_transaction_relevant(transaction: &Transaction, tracked_addresses: &HashSet<String>) -> bool {
    // Check source address
    if tracked_addresses.contains(&transaction.source_address) {
        return true;
    }
    
    // Check destination address if available
    if let Some(dest) = &transaction.destination_address {
        if tracked_addresses.contains(dest) {
            return true;
        }
    }
    
    false
}

/// Transform transactions to optimize for database storage
pub fn batch_transactions(transactions: Vec<Transaction>, batch_size: usize) -> Vec<Vec<Transaction>> {
    transactions
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

