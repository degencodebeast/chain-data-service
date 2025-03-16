use chain_data_service::db::{connection, transaction};
use chain_data_service::models::Transaction;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up database connection
    println!("Establishing database connection...");
    let pool = connection::establish_connection().await?;
    println!("✅ Database connection established!");
    
    // Create test transaction
    let test_tx = Transaction {
        signature: format!("test_sig_{}", chrono::Utc::now().timestamp()),
        block_time: chrono::Utc::now().timestamp(),
        slot: 12345,
        source_address: "test_source_address".to_string(),
        destination_address: Some("test_dest_address".to_string()),
        amount: Some(123.45),
        program_id: Some("test_program".to_string()),
        success: true,
    };
    
    // Add transaction
    println!("Adding test transaction...");
    transaction::add_transactions(&pool, &[test_tx.clone()]).await?;
    println!("✅ Transaction added successfully!");
    
    // Query transaction
    println!("Querying transactions...");
    let (txs, count) = transaction::get_transactions(
        &pool,
        &test_tx.source_address,
        0,
        chrono::Utc::now().timestamp() + 1000,
        0,
        10
    ).await?;
    
    println!("Found {} transactions (total count: {})", txs.len(), count);
    for tx in txs {
        println!("Transaction: {} -> {} ({} SOL)", 
            tx.source_address,
            tx.destination_address.unwrap_or_else(|| "None".to_string()),
            tx.amount.unwrap_or(0.0)
        );
    }
    
    // Count transactions
    let count = transaction::count_transactions(
        &pool,
        &test_tx.source_address,
        0,
        chrono::Utc::now().timestamp() + 1000
    ).await?;
    println!("✅ Count check: found {} transactions", count);
    
    println!("All tests completed successfully!");
    Ok(())
}
