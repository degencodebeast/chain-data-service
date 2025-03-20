// Create configuration structure for:
// - RPC endpoint URL (from environment variables or command line)
// - Database connection string
// - Server listening address/port
// - Blockchain polling interval
// - Cache settings (size, TTL)

use dotenv::dotenv;
use std::env;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub solana_rpc_url: String,
    pub poll_interval: Duration,
    pub cache_ttl: Duration,
    pub cache_max_capacity: u64,
    pub solana_commitment_level: String,
    pub rpc_timeout_secs: u64,
    pub polling_interval_secs: u64,
    pub rpc_rate_limit: Option<u32>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok();

        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:///data.db".to_string());
        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);
        let solana_rpc_url = env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let poll_interval = env::var("POLL_INTERVAL")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .map(|secs| Duration::from_secs(secs))
            .unwrap_or(Duration::from_secs(10));
        let cache_ttl = env::var("CACHE_TTL")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .map(|secs| Duration::from_secs(secs))
            .unwrap_or(Duration::from_secs(60));
        let cache_max_capacity = env::var("CACHE_MAX_CAPACITY")
            .unwrap_or_else(|_| "1000".to_string())
            .parse()
            .unwrap_or(1000);
        let solana_commitment_level = env::var("SOLANA_COMMITMENT_LEVEL")
            .unwrap_or_else(|_| "confirmed".to_string());
        let rpc_timeout_secs = env::var("RPC_TIMEOUT_SECS")
            .map(|v| v.parse().unwrap_or(30))
            .unwrap_or(30);
        let polling_interval_secs = env::var("POLLING_INTERVAL_SECS")
            .map(|v| v.parse().unwrap_or(10))
            .unwrap_or(10);
        let rpc_rate_limit = env::var("RPC_RATE_LIMIT")
            .map(|v| v.parse().ok())
            .unwrap_or(None);

        Self {
            database_url,
            server_host,
            server_port,
            solana_rpc_url,
            poll_interval,
            cache_ttl,
            cache_max_capacity,
            solana_commitment_level,
            rpc_timeout_secs,
            polling_interval_secs,
            rpc_rate_limit,
        }
    }
}