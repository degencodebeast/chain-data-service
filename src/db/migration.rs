use sqlx::SqlitePool;
use tracing::info;

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    info!("Running database migrations...");
    
    // Create tracked_addresses table if not exists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tracked_addresses (
            address TEXT PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        )"
    )
    .execute(pool)
    .await?;
    
    // Create transactions table if not exists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS transactions (
            signature TEXT PRIMARY KEY,
            block_time INTEGER NOT NULL,
            slot INTEGER NOT NULL,
            source_address TEXT NOT NULL,
            destination_address TEXT,
            amount REAL,
            program_id TEXT,
            success BOOLEAN NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            FOREIGN KEY (source_address) REFERENCES tracked_addresses(address),
            FOREIGN KEY (destination_address) REFERENCES tracked_addresses(address)
        )"
    )
    .execute(pool)
    .await?;
    
    // Add indexes for common queries
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_transactions_source_address 
         ON transactions(source_address)"
    )
    .execute(pool)
    .await?;
    
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_transactions_destination_address 
         ON transactions(destination_address)"
    )
    .execute(pool)
    .await?;
    
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_transactions_block_time 
         ON transactions(block_time)"
    )
    .execute(pool)
    .await?;
    
    info!("Database migrations completed successfully");
    Ok(())
}



