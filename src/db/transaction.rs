use sqlx::{Pool, Sqlite, Row};
use crate::models::Transaction;

pub async fn add_transactions(pool: &Pool<Sqlite>, transactions: &[Transaction]) -> Result<(), sqlx::Error> {
    // Start a transaction for batch insert
    let mut tx = pool.begin().await?;

    for transaction in transactions {
        // Store these values to extend their lifetime
        let amount = transaction.amount.map(|val| val as f64);
        let success_val = if transaction.success { 1_i32 } else { 0_i32 };

        sqlx::query!(
            r#"
            INSERT INTO transactions 
            (signature, block_time, slot, source_address, destination_address, amount, program_id, success)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(signature) DO NOTHING
            "#,
            transaction.signature,
            transaction.block_time,
            transaction.slot,
            transaction.source_address,
            transaction.destination_address,
            amount,
            transaction.program_id,
            success_val
        )
        .execute(&mut *tx)
        .await?;
    }

    // Commit the transaction
    tx.commit().await?;

    Ok(())
}

pub async fn get_transactions(
    pool: &Pool<Sqlite>,
    address: &str,
    start_time: i64,
    end_time: i64,
    offset: i64,
    limit: i64,
) -> Result<(Vec<Transaction>, i64), sqlx::Error> {
    // Get total count first
    let total_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM transactions 
           WHERE (source_address = ? OR destination_address = ?)
           AND block_time >= ? AND block_time < ?"#,
        address, address, start_time, end_time
    )
    .fetch_one(pool)
    .await?;
    
    // Use query instead of query! to avoid compile-time type checking
    let rows = sqlx::query(
        r#"SELECT signature, block_time, slot, source_address, destination_address, 
                  amount, program_id, success
           FROM transactions 
           WHERE (source_address = ? OR destination_address = ?)
           AND block_time >= ? AND block_time < ?
           ORDER BY block_time ASC
           LIMIT ? OFFSET ?"#
    )
    .bind(address)
    .bind(address)
    .bind(start_time)
    .bind(end_time)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    
    // Convert rows to Transaction structs
    let transactions = rows.iter().map(|row| {
        Transaction {
            signature: row.get("signature"),
            block_time: row.get("block_time"),
            slot: row.get("slot"),
            source_address: row.get("source_address"),
            destination_address: row.get("destination_address"),
            amount: row.get::<Option<f64>, _>("amount"),
            program_id: row.get("program_id"),
            success: row.get::<i32, _>("success") != 0,
        }
    }).collect();

    Ok((transactions, total_count))
}

pub async fn count_transactions(
    pool: &Pool<Sqlite>,
    address: &str,
    start_time: i64,
    end_time: i64,
) -> Result<i64, sqlx::Error> {
    // Use query instead of query_scalar! to avoid compile-time checking
    let count = sqlx::query(
        "SELECT COUNT(*) FROM transactions 
         WHERE (source_address = ? OR destination_address = ?) 
         AND block_time >= ? AND block_time < ?"
    )
    .bind(address)
    .bind(address)
    .bind(start_time)
    .bind(end_time)
    .fetch_one(pool)
    .await?
    .get::<i64, _>(0);

    Ok(count)
}
