//! Transaction cache implementation using Moka

use std::time::Duration;
use moka::future::Cache;
use crate::models::Transaction;
use super::keys::CacheKey;
use tracing::{debug, info};

/// Manages caching for transaction query results
#[derive(Clone)]
pub struct TransactionCacheManager {
    /// Primary cache for transaction query results
    cache: Cache<CacheKey, (Vec<Transaction>, i64)>,
    /// Default TTL for cache entries
    default_ttl: Duration,
}

impl TransactionCacheManager {
    /// Create a new transaction cache manager
    pub fn new(capacity: u64, default_ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(default_ttl)
            .build();
            
        Self {
            cache,
            default_ttl,
        }
    }
    
    /// Get cached transaction query results
    pub async fn get(&self, key: &CacheKey) -> Option<(Vec<Transaction>, i64)> {
        let result = self.cache.get(key).await;
        if result.is_some() {
            debug!("Cache hit for key: {}", key);
        } else {
            debug!("Cache miss for key: {}", key);
        }
        result
    }
    
    /// Store transaction query results in cache
    pub async fn insert(
        &self, 
        key: CacheKey, 
        value: (Vec<Transaction>, i64),
        ttl: Option<Duration>,
    ) {
        let ttl = ttl.unwrap_or(self.default_ttl);
        
        // Use dynamic TTL based on recency of data
        self.cache.insert(key, value).await;
        debug!("Cached transaction results with TTL: {:?}", ttl);
    }
    
    /// Invalidate all cache entries for a specific address
    pub async fn invalidate_for_address(&self, address: &str) {
        // Clone address to avoid reference escaping the closure
        let address_owned = address.to_string();
        
        // Moka doesn't have direct pattern invalidation, so we use a predicate
        let _ = self.cache.invalidate_entries_if(move |k, _| {
            if let Some(key_addr) = k.address() {
                return key_addr == &address_owned;
            }
            false
        });
        
        info!("Invalidated cache entries for address: {}", address);
    }
    
    /// Get the appropriate TTL based on data recency
    pub fn get_ttl_for_time_range(&self, start_time: i64, end_time: i64) -> Duration {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        // Use shorter TTL for recent data
        if end_time > now - 3600 { // Data from last hour
            Duration::from_secs(30) // 30 seconds TTL
        } else if end_time > now - 86400 { // Data from last day
            Duration::from_secs(300) // 5 minutes TTL
        } else {
            Duration::from_secs(1800) // 30 minutes TTL for historical data
        }
    }
}
