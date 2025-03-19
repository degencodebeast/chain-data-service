pub mod address;
pub mod connection;
pub mod transaction;
pub mod migration;

pub const INIT_SCHEMA: &str = r#"
-- Create tracked_addresses table
CREATE TABLE IF NOT EXISTS tracked_addresses (
    address TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);

-- Create transactions table
CREATE TABLE IF NOT EXISTS transactions (
    signature TEXT PRIMARY KEY,
    block_time INTEGER NOT NULL,
    slot INTEGER NOT NULL,
    source_address TEXT NOT NULL,
    destination_address TEXT,
    amount REAL,
    program_id TEXT,
    success BOOLEAN NOT NULL,
    FOREIGN KEY (source_address) REFERENCES tracked_addresses(address),
    FOREIGN KEY (destination_address) REFERENCES tracked_addresses(address)
);

-- Create indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_transactions_source_time ON transactions(source_address, block_time);
CREATE INDEX IF NOT EXISTS idx_transactions_destination_time ON transactions(destination_address, block_time);
CREATE INDEX IF NOT EXISTS idx_transactions_time ON transactions(block_time);
"#;
