//! Cache implementation using Moka for high-performance concurrent caching

mod keys;
mod transaction;
mod address;

use std::time::Duration;
use crate::config::Config;
use moka::future::Cache;
use crate::models::Transaction;

pub use self::transaction::TransactionCacheManager;
pub use self::address::AddressCacheManager;
pub use self::keys::CacheKey;

/// Central cache structure that holds various caches
#[derive(Clone)]
pub struct AppCache {
    /// Cache for transaction query results
    pub transaction_cache: TransactionCacheManager,
    /// Cache for address existence checks
    pub address_cache: AddressCacheManager,
}

/// Initialize cache with configuration
pub fn init_cache(config: &Config) -> AppCache {
    let transaction_cache = TransactionCacheManager::new(
        config.cache_max_capacity,
        config.cache_ttl,
    );
    
    let address_cache = AddressCacheManager::new(
        1000, // Smaller cache for addresses
        Duration::from_secs(60), // 1 minute TTL for address cache
    );
    
    AppCache {
        transaction_cache,
        address_cache,
    }
}

/// Cache metrics for monitoring
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub size: u64,
}
