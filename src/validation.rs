use bs58;
use thiserror::Error;
use crate::api::error::ApiError;
use serde::de::DeserializeOwned;
use std::str::FromStr;
use axum::extract::Query;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    
    #[error("Invalid action: {0}. Must be 'add' or 'remove'")]
    InvalidAction(String),
    
    #[error("Invalid Solana address format: {0}")]
    InvalidSolanaAddress(String),
    
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

pub fn validate_action(action: &str) -> Result<(), ValidationError> {
    match action {
        "add" | "remove" => Ok(()),
        _ => Err(ValidationError::InvalidAction(action.to_string())),
    }
}

pub fn validate_solana_address(address: &str) -> Result<(), ValidationError> {
    // Check if address is empty
    if address.trim().is_empty() {
        return Err(ValidationError::MissingParameter("address".to_string()));
    }
    
    // Decode base58 string
    let decoded = match bs58::decode(address).into_vec() {
        Ok(bytes) => bytes,
        Err(_) => return Err(ValidationError::InvalidSolanaAddress(address.to_string())),
    };
    
    // Validate length (Solana addresses are 32 bytes)
    if decoded.len() != 32 {
        return Err(ValidationError::InvalidSolanaAddress(address.to_string()));
    }
    
    Ok(())
}

pub fn validate_address_action(address: &str, action: &str) -> Result<(), ValidationError> {
    // Check if parameters are present
    if address.trim().is_empty() {
        return Err(ValidationError::MissingParameter("address".to_string()));
    }
    
    if action.trim().is_empty() {
        return Err(ValidationError::MissingParameter("action".to_string())); 
    }
    
    // Validate action
    validate_action(action)?;
    
    // Validate address
    validate_solana_address(address)?;
    
    Ok(())
}

// Validate timestamp parameter
pub fn validate_timestamp(timestamp: &str) -> Result<i64, ValidationError> {
    i64::from_str(timestamp)
        .map_err(|_| ValidationError::InvalidParameter(format!("Invalid timestamp: {}", timestamp)))
}

// Validate offset parameter
pub fn validate_offset(offset: &str) -> Result<i64, ValidationError> {
    let offset = i64::from_str(offset)
        .map_err(|_| ValidationError::InvalidParameter(format!("Invalid offset: {}", offset)))?;
    
    if offset < 0 {
        return Err(ValidationError::InvalidParameter("Offset must be non-negative".to_string()));
    }
    
    Ok(offset)
}

// Validate limit parameter
pub fn validate_limit(limit: &str) -> Result<i64, ValidationError> {
    let limit = i64::from_str(limit)
        .map_err(|_| ValidationError::InvalidParameter(format!("Invalid limit: {}", limit)))?;
    
    if limit <= 0 {
        return Err(ValidationError::InvalidParameter("Limit must be positive".to_string()));
    }
    
    if limit > 1000 {
        return Err(ValidationError::InvalidParameter("Limit cannot exceed 1000".to_string()));
    }
    
    Ok(limit)
}

// // Validate timestamp parameter
// pub fn validate_timestamp(timestamp: &str) -> Result<i64, ApiError> {
//     i64::from_str(timestamp)
//         .map_err(|_| ApiError::InvalidParameter(format!("Invalid timestamp: {}", timestamp)))
// }

// Validate offset parameter
// ... 