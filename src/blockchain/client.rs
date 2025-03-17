use crate::config::Config;
use solana_client::rpc_client::{RpcClient, GetConfirmedSignaturesForAddress2Config};
use solana_client::rpc_config::{RpcTransactionConfig, RpcAccountInfoConfig};
use solana_client::rpc_response::{RpcConfirmedTransactionStatusWithSignature, Response};
use solana_sdk::signature::Signature;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, warn};

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
}

pub struct SolanaClient {
    rpc_client: RpcClient,
    commitment: CommitmentConfig,
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
        
        info!("Initializing Solana client with RPC endpoint: {}, commitment: {:?}", rpc_url, commitment);
        
        let rpc_client = RpcClient::new_with_timeout_and_commitment(
            rpc_url.clone(),
            timeout,
            commitment,
        );
        
        Self {
            rpc_client,
            commitment,
        }
    }
    
    /// Get the current slot (block height)
    pub async fn get_slot(&self) -> Result<u64, ClientError> {
        let slot = self.rpc_client.get_slot_with_commitment(self.commitment)?;
        Ok(slot)
    }
    
    /// Get block time for a specific slot
    pub async fn get_block_time(&self, slot: u64) -> Result<Option<i64>, ClientError> {
        match self.rpc_client.get_block_time(slot) {
            Ok(time) => Ok(Some(time)),
            Err(e) => {
                if e.to_string().contains("not available for slot") {
                    Ok(None)
                } else {
                    Err(ClientError::RpcError(e))
                }
            }
        }
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
        
        // Get transaction
        let tx = self.rpc_client.get_transaction_with_config(&signature, config)?;
        Ok(tx)
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
        
        // Get signatures
        let signatures = self.rpc_client.get_signatures_for_address_with_config(
            &pubkey,
            GetConfirmedSignaturesForAddress2Config {
                before: before_sig,
                until: until_sig,
                limit,
                commitment: Some(self.commitment),
            },
        )?;
        
        Ok(signatures)
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
        
        match self.rpc_client.get_account_with_config(&pubkey, config) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("account not found") {
                    Ok(false)
                } else {
                    Err(ClientError::RpcError(e))
                }
            }
        }
    }
}

