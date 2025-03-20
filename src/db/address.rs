// Implement functions for:
// - add_transactions(transactions: &[Transaction]) -> Result<()>  (batch insert)
// - get_transactions(address: &str, start_time: u64, end_time: u64, offset: u64, limit: u64) -> Result<(Vec<Transaction>, u64)>
//   Returns transactions and total count

use sqlx::{Pool, Sqlite, SqlitePool};
// This import is unused and can be removed
// use crate::models::Address;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::validation::validate_solana_address;


pub async fn add_address(pool: &Pool<Sqlite>, address: &str) -> Result<(), sqlx::Error> {
    // Validate address
    validate_solana_address(address)
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Convert to use query! macro
    sqlx::query!(
        r#"
        INSERT INTO tracked_addresses (address, added_at) 
        VALUES (?, ?) 
        ON CONFLICT(address) DO UPDATE SET added_at = excluded.added_at
        "#,
        address,
        timestamp
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn remove_address(pool: &Pool<Sqlite>, address: &str) -> Result<bool, sqlx::Error> {
    // Convert to use query! macro
    let result = sqlx::query!(
        r#"DELETE FROM tracked_addresses WHERE address = ?"#,
        address
    )
    .execute(pool)
    .await?;
    
    Ok(result.rows_affected() > 0)
}

pub async fn is_address_tracked(pool: &Pool<Sqlite>, address: &str) -> Result<bool, sqlx::Error> {
    // Use query_scalar! macro
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM tracked_addresses WHERE address = ?"#,
        address
    )
    .fetch_one(pool)
    .await?;
    
    Ok(count > 0)
}

pub async fn get_all_tracked_addresses(pool: &Pool<Sqlite>) -> Result<Vec<String>, sqlx::Error> {
    // Option 1: Using the query! macro with filtering
    let addresses = sqlx::query!(
        r#"SELECT address FROM tracked_addresses"#
    )
    .fetch_all(pool)
    .await?;
    
    // Filter out any NULL addresses and unwrap the rest
    Ok(addresses
        .into_iter()
        .filter_map(|row| row.address)
        .collect())
    
    // Option 2: Alternative approach using query() like transaction.rs does
    /*
    let rows = sqlx::query("SELECT address FROM tracked_addresses")
        .fetch_all(pool)
        .await?;
    
    let addresses = rows.iter()
        .map(|row| row.get::<String, _>("address"))
        .collect();
    
    Ok(addresses)
    */
}

// pub async fn get_last_updated_slot(db_pool: &SqlitePool, address: &str) -> Result<Option<u64>, sqlx::Error> {
//     // Use query_scalar! to get the MAX(slot) directly as i64, then convert to u64
//     let result: Option<i64> = sqlx::query!(
//         "SELECT MAX(slot) FROM transactions 
//          WHERE source_address = ? OR destination_address = ?",
//         address,
//         address
//     )
//     .fetch_one(db_pool)
//     .await?;
    
//     // Convert i64 to u64 safely
//     Ok(result.map(|val| val as u64))
// }

pub async fn get_last_updated_slot(db_pool: &SqlitePool, address: &str) -> Result<Option<u64>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT MAX(slot) as last_slot FROM transactions 
         WHERE source_address = ? OR destination_address = ?",
        address,
        address
    )
    .fetch_one(db_pool)
    .await?;
    
    Ok(result.last_slot)
}