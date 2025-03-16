use sqlx::{Pool, Sqlite};
use crate::models::Transaction;

pub async fn add_transactions(pool: &Pool<Sqlite>, transactions: &[Transaction]) -> Result<(), sqlx::Error> {
    // Start a transaction for batch insert
    let mut tx = pool.begin().await?;

    for transaction in transactions {
        // sqlx::query!(
        //     r#"
        //     INSERT INTO transactions 
        //     (signature, block_time, slot, source_address, destination_address, amount, program_id, success)
        //     VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        //     ON CONFLICT(signature) DO NOTHING
        //     "#,
        //     transaction.signature,
        //     transaction.block_time,
        //     transaction.slot,
        //     transaction.source_address,
        //     transaction.destination_address,
        //     transaction.amount.map(|val| val as f64),
        //     transaction.program_id,
        //     transaction.success as i32
        // )
        .execute(&mut tx)
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
    let total_count = sqlx::query!(
        r#"
        SELECT COUNT(*) as count FROM transactions 
        WHERE (source_address = ? OR destination_address = ?)
        AND block_time >= ? AND block_time < ?
        "#,
        address, address, start_time, end_time
    )
    .fetch_one(pool)
    .await?
    .count;

    // Get paginated results
    let rows = sqlx::query!(
        r#"
        SELECT * FROM transactions 
        WHERE (source_address = ? OR destination_address = ?)
        AND block_time >= ? AND block_time < ?
        ORDER BY block_time ASC
        LIMIT ? OFFSET ?
        "#,
        address, address, start_time, end_time, limit, offset
    )
    .fetch_all(pool)
    .await?;

    let transactions = rows
        .into_iter()
        .map(|row| Transaction {
            signature: row.signature,
            block_time: row.block_time,
            slot: row.slot,
            source_address: row.source_address,
            destination_address: row.destination_address,
            amount: row.amount,
            program_id: row.program_id,
            success: row.success != 0,
        })
        .collect();

    Ok((transactions, total_count))
}
