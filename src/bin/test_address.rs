use chain_data_service::db::{connection, address};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up database connection
    println!("Establishing database connection...");
    let pool = connection::establish_connection().await?;
    println!("✅ Database connection established!");
    
    // IMPORTANT: Verify the table exists
    sqlx::query("CREATE TABLE IF NOT EXISTS tracked_addresses (
        ROWID INTEGER PRIMARY KEY,
        address TEXT NOT NULL UNIQUE,
        added_at INTEGER NOT NULL
    )")
    .execute(&pool)
    .await?;
    println!("✅ Ensured table exists");
    
    // Create test address - use a valid Solana address
    let test_address = "AhAkbf3cGD6HkFod2rBEE8mie8ks9p7vuss6WGkUFAM9"; // Valid Solana address
    
    // Add address
    println!("Adding test address...");
    address::add_address(&pool, test_address).await?;
    println!("✅ Address added successfully!");
    
    // Check if address is tracked
    println!("Checking if address is tracked...");
    let is_tracked = address::is_address_tracked(&pool, test_address).await?;
    println!("✅ Is address tracked: {}", is_tracked);
    
    // Debug: Directly check database content
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracked_addresses")
        .fetch_one(&pool)
        .await?;
    println!("Direct database check: {} addresses in table", count.0);
    
    // Get all tracked addresses
    println!("Getting all tracked addresses...");
    let addresses = address::get_all_tracked_addresses(&pool).await?;
    println!("✅ Retrieved {} tracked addresses", addresses.len());
    
    // Check if our test address is in the list
    let found = addresses.iter().any(|addr| addr == &test_address);
    println!("✅ Test address found in tracked list: {}", found);
    
    if !found {
        println!("❌ ERROR: Test address not found in tracked addresses!");
        return Err("Address retrieval test failed".into());
    }
    
    // Print all addresses
    println!("Tracked addresses:");
    for addr in &addresses {
        println!("  - {}", addr);
    }
    
    // // Test removing an address
    // println!("Removing test address...");
    // let removed = address::remove_address(&pool, &test_address).await?;
    // println!("✅ Address removed successfully: {}", removed);
    
    // if !removed {
    //     println!("❌ ERROR: Address should have been removed but wasn't!");
    //     return Err("Address removal test failed".into());
    // }
    
    // Verify address is no longer tracked
    // println!("Verifying address is no longer tracked...");
    // let is_tracked_after_removal = address::is_address_tracked(&pool, &test_address).await?;
    // println!("✅ Is address still tracked: {}", is_tracked_after_removal);
    
    // if is_tracked_after_removal {
    //     println!("❌ ERROR: Address should not be tracked after removal!");
    //     return Err("Address removal verification failed".into());
    // }
    
    println!("All tests completed successfully!");
    Ok(())
}
