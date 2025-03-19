//! Cache key generation and management

use std::fmt;
use std::hash::{Hash, Hasher};

/// A structured cache key that can be converted to a string
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    /// Key for transaction query results
    TransactionQuery {
        address: String,
        start_time: i64,
        end_time: i64,
        offset: i64,
        limit: i64,
    },
    /// Key for checking if an address is tracked
    AddressTracked(String),
}

impl CacheKey {
    /// Create a new transaction query key
    pub fn transaction_query(
        address: &str,
        start_time: i64,
        end_time: i64,
        offset: i64,
        limit: i64,
    ) -> Self {
        Self::TransactionQuery {
            address: address.to_string(),
            start_time,
            end_time,
            offset,
            limit,
        }
    }
    
    /// Create a new address tracked key
    pub fn address_tracked(address: &str) -> Self {
        Self::AddressTracked(address.to_string())
    }
    
    /// Get the base address from a key (for pattern invalidation)
    pub fn address(&self) -> Option<&str> {
        match self {
            Self::TransactionQuery { address, .. } => Some(address),
            Self::AddressTracked(address) => Some(address),
        }
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransactionQuery { address, start_time, end_time, offset, limit } => {
                write!(f, "tx:{}:{}:{}:{}:{}", address, start_time, end_time, offset, limit)
            }
            Self::AddressTracked(address) => {
                write!(f, "addr:{}", address)
            }
        }
    }
}


































