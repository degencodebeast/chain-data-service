use moka::future::Cache;
use crate::{config::Config, models::Transaction};
//use std::time::Duration;

pub type TransactionCache = Cache<String, Vec<Transaction>>;

pub fn init_cache(config: &Config) -> TransactionCache {
    Cache::builder()
        .time_to_live(config.cache_ttl)
        .max_capacity(config.cache_max_capacity)
        .build()
}

pub fn generate_cache_key(address: &str, start_time: i64, end_time: i64) -> String {
    format!("{}:{}:{}", address, start_time, end_time)
}
