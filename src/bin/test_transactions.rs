use chain_data_service::db::{connection, transaction};
use chain_data_service::models::Transaction;
use std::time::{SystemTime, UNIX_EPOCH};
use chain_data_service::db::address;
use chain_data_service::config::Config;
use chain_data_service::cache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Establishing database connection...");
    let pool = connection::establish_connection().await?;
    println!("✅ Database connection established!");
    
    // Initialize a test cache
    let config = Config::from_env();
    let test_cache = cache::init_cache(&config);
    
    println!("Cleaning database before testing...");
    sqlx::query("DELETE FROM transactions").execute(&pool).await?;
    sqlx::query("DELETE FROM tracked_addresses").execute(&pool).await?;
    println!("✅ Database cleaned!");
    // Get current timestamp
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let hour_ago = now - 3600;
    let day_ago = now - 86400;
    let source_address = "9ii1FEiWSgDzXAbwj2oTmJXzkfCw78mnHwPQv9WQ5iTn".to_string();
    let destination_address = "AhAkbf3cGD6HkFod2rBEE8mie8ks9p7vuss6WGkUFAM9".to_string();
    let source_address_2 = "FwKc3s5x7SguXzNPPJP7AV2UUhCF4rnEQCFdA2Q8NGCi".to_string();
    let destination_address_2 = "424CJUQd2RQWNgygWbNpRmQStZ77Mea2f29CATe8M2hS".to_string();

    // Add after database connection and before creating transactions
    println!("Registering test addresses...");
    address::add_address(&pool, &source_address).await?;
    address::add_address(&pool, &destination_address).await?;
    address::add_address(&pool, &source_address_2).await?;
    address::add_address(&pool, &destination_address_2).await?;
    println!("✅ Test addresses registered!");

    // Create test transactions
    let transactions = vec![
        // Current transaction - use first pair
        Transaction {
            signature: format!("test_sig_current_{}", now),
            block_time: now,
            slot: 12345,
            source_address: source_address.clone(),
            destination_address: Some(destination_address.clone()),
            amount: Some(123.45),
            program_id: Some("program1".to_string()),
            success: true,
        },
        // Hour old transaction - use first pair
        Transaction {
            signature: format!("test_sig_hour_{}", now),
            block_time: hour_ago,
            slot: 12344,
            source_address: source_address.clone(),
            destination_address: Some(destination_address.clone()),
            amount: Some(67.89),
            program_id: Some("program2".to_string()),
            success: true,
        },
        // Day old transaction - use second pair
        Transaction {
            signature: format!("test_sig_day_{}", now),
            block_time: day_ago,
            slot: 12343,
            source_address: source_address_2.clone(),
            destination_address: Some(destination_address_2.clone()),
            amount: Some(42.0),
            program_id: Some("program1".to_string()),
            success: false,
        },
        // Transaction with NULL fields - use second source
        Transaction {
            signature: format!("test_sig_null_{}", now),
            block_time: now,
            slot: 12342,
            source_address: source_address_2.clone(),
            destination_address: None,
            amount: None,
            program_id: None,
            success: false,
        },
    ];
    
    // TEST 1: Batch insert
    println!("Adding multiple transactions...");
    transaction::add_transactions(&pool, &transactions).await?;
    println!("✅ Batch transaction insert successful!");
    
    // TEST 2: Duplicate insert (should not error)
    println!("Testing duplicate handling...");
    transaction::add_transactions(&pool, &transactions[0..1]).await?;
    println!("✅ Duplicate handling successful!");
    
    // TEST 3: Get transactions with time filtering
    println!("Testing time filtering...");
    let (recent_txs, _recent_count) = transaction::get_transactions(
        &pool,
        &test_cache,
        &source_address,
        hour_ago,  // From hour ago
        now + 1,   // To now
        0,         // No offset
        10         // Get up to 10
    ).await?;
    println!("✅ Found {} recent transactions", recent_txs.len());
    assert_eq!(recent_txs.len(), 2, "Should find 2 transactions in the last hour");
    
    // TEST 4: Pagination
    println!("Testing pagination...");
    let (page1, _) = transaction::get_transactions(
        &pool,
        &test_cache,
        &source_address,
        0,         // From beginning of time
        now + 1,   // To now
        0,         // First page
        1          // Only 1 per page
    ).await?;
    
    let (page2, _) = transaction::get_transactions(
        &pool,
        &test_cache,
        &source_address,
        0,         // From beginning of time
        now + 1,   // To now
        1,         // Second page
        1          // Only 1 per page
    ).await?;
    
    assert_eq!(page1.len(), 1, "Page 1 should have 1 transaction");
    assert_eq!(page2.len(), 1, "Page 2 should have 1 transaction");
    assert_ne!(page1[0].signature, page2[0].signature, "Pages should have different transactions");
    println!("✅ Pagination successful!");
    
    // TEST 5: Destination address query
    println!("Testing destination address query...");
    let (dest_txs, _) = transaction::get_transactions(
        &pool,
        &test_cache,
        &destination_address,
        0,
        now + 1,
        0,
        10
    ).await?;
    println!("✅ Found {} transactions for destination address", dest_txs.len());
    assert!(dest_txs.len() >= 2, "Should find at least 2 transactions for dest1");
    
    // TEST 6: Count with time filtering
    println!("Testing count with time filtering...");
    let day_count = transaction::count_transactions(
        &pool,
        &source_address, 
        0,          // From beginning of time
        now + 1     // To now
    ).await?;
    
    let hour_count = transaction::count_transactions(
        &pool,
        &source_address,
        hour_ago,   // From hour ago
        now + 1     // To now
    ).await?;
    
    assert!(day_count >= hour_count, "Day count should be >= hour count");
    println!("✅ Count filtering successful! (day: {}, hour: {})", day_count, hour_count);
    
    // TEST 7: Null field handling
    println!("Testing NULL field handling...");
    let (null_txs, _) = transaction::get_transactions(
        &pool,
        &test_cache,
        &source_address_2,
        now - 10,  // Recent only
        now + 1,
        0,
        10
    ).await?;
    
    let null_tx = null_txs.iter().find(|tx| tx.destination_address.is_none());
    assert!(null_tx.is_some(), "Should find transaction with NULL destination");
    println!("✅ NULL field handling successful!");
    
    println!("All transaction tests completed successfully!");
    Ok(())
} 