//! Address tracking cache implementation

use std::time::Duration;
use moka::future::Cache;
use super::keys::CacheKey;
use tracing::debug;

/// Manages caching for address tracking status
#[derive(Clone)]
pub struct AddressCacheManager {
    /// Cache for address existence checks
    cache: Cache<String, bool>,
}

impl AddressCacheManager {
    /// Create a new address cache manager
    pub fn new(capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(ttl)
            .build();
            
        Self { cache }
    }
    
    /// Check if an address is tracked (cached)
    pub async fn is_tracked(&self, address: &str) -> Option<bool> {
        self.cache.get(address).await
    }
    
    /// Cache address tracking status
    pub async fn set_tracked(&self, address: &str, is_tracked: bool) {
        self.cache.insert(address.to_string(), is_tracked).await;
        debug!("Cached address tracking status: {} = {}", address, is_tracked);
    }
    
    /// Invalidate address tracking status
    pub async fn invalidate(&self, address: &str) {
        self.cache.invalidate(address).await;
        debug!("Invalidated address tracking status: {}", address);
    }
}
