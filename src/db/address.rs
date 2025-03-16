// Implement functions for:
// - add_transactions(transactions: &[Transaction]) -> Result<()>  (batch insert)
// - get_transactions(address: &str, start_time: u64, end_time: u64, offset: u64, limit: u64) -> Result<(Vec<Transaction>, u64)>
//   Returns transactions and total count

use sqlx::{Pool, Sqlite};
use crate::models::Address;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn add_address(pool: &Pool<Sqlite>, address: &str) -> Result<(), sqlx::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    sqlx::query!(
        "INSERT INTO tracked_addresses (address, added_at) VALUES (?, ?)
         ON CONFLICT(address) DO NOTHING",
        address, now
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn remove_address(pool: &Pool<Sqlite>, address: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM tracked_addresses WHERE address = ?", address)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn is_address_tracked(pool: &Pool<Sqlite>, address: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT address FROM tracked_addresses WHERE address = ?", address)
        .fetch_optional(pool)
        .await?;

    Ok(result.is_some())
}

pub async fn get_all_tracked_addresses(pool: &Pool<Sqlite>) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!("SELECT address FROM tracked_addresses")
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(|row| row.address).collect())
}