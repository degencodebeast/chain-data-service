// Define Transaction struct with all fields from your database
// Define API request/response models
// Implement serialization/deserialization

use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize)]
pub struct Address {
    pub address: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub signature: String,
    pub block_time: i64,
    pub slot: i64,
    pub source_address: String,
    pub destination_address: Option<String>,
    pub amount: Option<f64>,
    pub program_id: Option<String>,
    pub success: bool,
}

impl Transaction {
    pub fn new(
        signature: String,
        block_time: i64,
        slot: i64,
        source_address: String,
        destination_address: Option<String>,
        amount: Option<f64>,
        program_id: Option<String>,
        success: bool,
    ) -> Self {
        Self {
            signature,
            block_time,
            slot,
            source_address,
            destination_address,
            amount,
            program_id,
            success,
        }
    }
}

// API response models
#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub data: Vec<Transaction>,
    pub meta: Meta,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}