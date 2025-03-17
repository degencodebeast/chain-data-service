-- Fix amount type to ensure REAL
CREATE TABLE IF NOT EXISTS transactions_new (
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

-- Copy data with conversion
INSERT OR IGNORE INTO transactions_new 
SELECT 
    signature, 
    block_time, 
    slot, 
    source_address, 
    destination_address, 
    CAST(amount AS REAL), 
    program_id, 
    success 
FROM transactions WHERE EXISTS (SELECT 1 FROM transactions);

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_transactions_source_time ON transactions_new(source_address, block_time);
CREATE INDEX IF NOT EXISTS idx_transactions_destination_time ON transactions_new(destination_address, block_time);
CREATE INDEX IF NOT EXISTS idx_transactions_time ON transactions_new(block_time);

-- Drop old table
DROP TABLE IF EXISTS transactions;

-- Rename new table
ALTER TABLE transactions_new RENAME TO transactions;
