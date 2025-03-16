use bs58;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    
    #[error("Invalid action: {0}. Must be 'add' or 'remove'")]
    InvalidAction(String),
    
    #[error("Invalid Solana address format: {0}")]
    InvalidSolanaAddress(String),
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