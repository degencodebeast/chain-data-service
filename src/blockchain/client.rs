use crate::config::Config;
use solana_client::rpc_client::{RpcClient, GetConfirmedSignaturesForAddress2Config};
use solana_client::rpc_config::{RpcTransactionConfig, RpcAccountInfoConfig};
use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use solana_sdk::signature::Signature;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use std::str::FromStr;
use std::time::Duration;
use std::sync::Arc;
use std::num::NonZeroU32;
use governor::{Quota, RateLimiter};
use governor::state::{NotKeyed, InMemoryState};
use governor::clock::DefaultClock;
use backon::{Retryable, ExponentialBuilder};
use thiserror::Error;
use tracing::{error, info, debug};
use governor::clock::Clock;


#[derive(Error, Debug)]
pub enum ClientError {
    #[error("RPC error: {0}")]
    RpcError(#[from] solana_client::client_error::ClientError),
    
    #[error("Invalid signature: {0}")]
    SignatureError(String),
    
    #[error("Invalid public key: {0}")]
    PubkeyError(String),
    
    #[error("Transaction not found: {0}")]
    NotFound(String),
    
    #[error("Rate limit exceeded")]
    RateLimited,
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,
}

impl ClientError {
    pub fn is_network_error(&self) -> bool {
        match self {
            ClientError::RpcError(e) => {
                e.to_string().contains("timed out") || 
                e.to_string().contains("connection refused") ||
                e.to_string().contains("reset by peer")
            },
            ClientError::NetworkError(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct SolanaClient {
    rpc_client: Arc<RpcClient>,
    commitment: CommitmentConfig,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl SolanaClient {
    pub fn new(config: &Config) -> Self {
        let rpc_url = &config.solana_rpc_url;
        let timeout = Duration::from_secs(config.rpc_timeout_secs);
        
        // Use commitment level from config or default to "confirmed"
        let commitment = match config.solana_commitment_level.as_str() {
            "processed" => CommitmentConfig::processed(),
            "confirmed" => CommitmentConfig::confirmed(),
            "finalized" => CommitmentConfig::finalized(),
            _ => CommitmentConfig::confirmed(),
        };
        
        // Create rate limiter with sensible defaults
        let rps = config.rpc_rate_limit.unwrap_or(5);
        let quota = Quota::per_second(NonZeroU32::new(rps).unwrap());
        
        // Remove the GovernorConfigBuilder and use direct creation with jitter
        let rate_limiter = Arc::new(RateLimiter::direct(quota));
        
        info!("Initializing Solana client with RPC endpoint: {}, commitment: {:?}, rate limit: {} RPS", 
              rpc_url, commitment, rps);
        
        let rpc_client = RpcClient::new_with_timeout_and_commitment(
            rpc_url.clone(),
            timeout,
            commitment,
        );
        
        Self {
            rpc_client: Arc::new(rpc_client),
            commitment,
            rate_limiter,
        }
    }
    
    // Helper method to enforce rate limiting
    async fn rate_limited<F, T, E>(&self, operation: F) -> Result<T, ClientError>
    where
        F: FnOnce() -> Result<T, E>,
        E: Into<ClientError>,
    {
        match self.rate_limiter.check() {
            Ok(()) => operation().map_err(Into::into),
            Err(negative) => {
                // If rate limit is hit, sleep for the required duration
                let wait_time = negative.wait_time_from(governor::clock::DefaultClock::default().now());
                debug!("Rate limit hit, waiting for {:?}", wait_time);
                tokio::time::sleep(wait_time).await;
                operation().map_err(Into::into)
            }
        }
    }
    
    // Configure a default retry policy
    fn retry_policy() -> ExponentialBuilder {
        ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(10))
            .with_max_times(5)
            .with_jitter()
    }
    
    /// Get the current slot (block height)
    pub async fn get_slot(&self) -> Result<u64, ClientError> {
        // Using backon's Retryable trait for retry with exponential backoff
        (move || async move {
            self.rate_limited(|| {
                self.rpc_client.get_slot_with_commitment(self.commitment)
                    .map_err(|e| {
                        if e.to_string().contains("timed out") || 
                           e.to_string().contains("connection refused") {
                            // Network errors should be retried
                            ClientError::NetworkError(e.to_string())
                        } else {
                            // Other errors shouldn't be retried
                            ClientError::RpcError(e)
                        }
                    })
            }).await
        })
        .retry(&Self::retry_policy())
        .await
        .map_err(|e| {
            error!("Failed to get slot after maximum retries: {}", e);
            e
        })
    }
    
    /// Get block time for a specific slot
    pub async fn get_block_time(&self, slot: u64) -> Result<Option<i64>, ClientError> {
        (move || async move {
            self.rate_limited(|| {
                match self.rpc_client.get_block_time(slot) {
                    Ok(time) => Ok(Some(time)),
                    Err(e) => {
                        if e.to_string().contains("not available for slot") {
                            Ok(None)
                        } else if e.to_string().contains("timed out") || 
                                e.to_string().contains("connection refused") {
                            Err(ClientError::NetworkError(e.to_string()))
                        } else {
                            Err(ClientError::RpcError(e))
                        }
                    }
                }
            }).await
        })
        .retry(&Self::retry_policy())
        .await
    }
    
    /// Get transaction details by signature
    pub async fn get_transaction(&self, signature_str: &str) -> Result<EncodedConfirmedTransactionWithStatusMeta, ClientError> {
        // Parse signature
        let signature = Signature::from_str(signature_str)
            .map_err(|_| ClientError::SignatureError(signature_str.to_string()))?;
        
        // Set up config to get parsed transaction with metadata
        let config = RpcTransactionConfig {
            encoding: Some(solana_transaction_status::UiTransactionEncoding::JsonParsed),
            commitment: Some(self.commitment),
            max_supported_transaction_version: Some(0),
        };
        
        // Get transaction with rate limiting and retry
        (move || async move {
            self.rate_limited(|| {
                match self.rpc_client.get_transaction_with_config(&signature, config) {
                    Ok(tx) => Ok(tx),
                    Err(e) => {
                        if e.to_string().contains("timed out") || 
                           e.to_string().contains("connection refused") {
                            Err(ClientError::NetworkError(e.to_string()))
                        } else {
                            Err(ClientError::RpcError(e))
                        }
                    }
                }
            }).await
        })
        .retry(&Self::retry_policy())
        .await
    }
    
    /// Get signatures for address
    pub async fn get_signatures_for_address(
        &self,
        address: &str,
        before: Option<&str>,
        until: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>, ClientError> {
        // Parse address to pubkey
        let pubkey = Pubkey::from_str(address)
            .map_err(|_| ClientError::PubkeyError(address.to_string()))?;
        
        // Parse before/until signatures
        let before_sig = if let Some(sig) = before {
            Some(Signature::from_str(sig)
                .map_err(|_| ClientError::SignatureError(sig.to_string()))?)
        } else {
            None
        };
        
        let until_sig = if let Some(sig) = until {
            Some(Signature::from_str(sig)
                .map_err(|_| ClientError::SignatureError(sig.to_string()))?)
        } else {
            None
        };
        
        // Clone the signatures for use in the closure
        let before_sig_clone = before_sig.clone();
        let until_sig_clone = until_sig.clone();
        
        // Get signatures with rate limiting and retry
        (move || async move {
            self.rate_limited(|| {
                match self.rpc_client.get_signatures_for_address_with_config(
                    &pubkey,
                    GetConfirmedSignaturesForAddress2Config {
                        before: before_sig_clone.clone(),
                        until: until_sig_clone.clone(),
                        limit,
                        commitment: Some(self.commitment),
                    },
                ) {
                    Ok(sigs) => Ok(sigs),
                    Err(e) => {
                        if e.to_string().contains("timed out") || 
                           e.to_string().contains("connection refused") {
                            Err(ClientError::NetworkError(e.to_string()))
                        } else {
                            Err(ClientError::RpcError(e))
                        }
                    }
                }
            }).await
        })
        .retry(&Self::retry_policy())
        .await
    }
    
    /// Check if an address exists (has been used on-chain)
    pub async fn address_exists(&self, address: &str) -> Result<bool, ClientError> {
        let pubkey = Pubkey::from_str(address)
            .map_err(|_| ClientError::PubkeyError(address.to_string()))?;
            
        // Minimal config to just check existence
        let config = RpcAccountInfoConfig {
            encoding: None,
            data_slice: None,
            commitment: Some(self.commitment),
            min_context_slot: None,
        };
        
        // Clone the config before using it in the closure
        let config_clone = config.clone();
        
        (move || {
            let config = config_clone.clone(); // Clone it each time the closure is called
            async move {
                self.rate_limited(|| {
                    match self.rpc_client.get_account_with_config(&pubkey, config) {
                        Ok(_) => Ok(true),
                        Err(e) => {
                            if e.to_string().contains("account not found") {
                                Ok(false)
                            } else if e.to_string().contains("timed out") || 
                                    e.to_string().contains("connection refused") {
                                Err(ClientError::NetworkError(e.to_string()))
                            } else {
                                Err(ClientError::RpcError(e))
                            }
                        }
                    }
                }).await
            }
        })
        .retry(&Self::retry_policy())
        .await
    }
}

