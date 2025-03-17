use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiMessage, UiInstruction, UiTransactionStatusMeta,
    EncodedTransaction,
    UiParsedMessage,
};
use serde_json::Value;
use crate::models::Transaction;
use tracing::warn;
use solana_sdk::system_program::ID as SYSTEM_PROGRAM_ID;
use spl_token::ID as TOKEN_PROGRAM_ID;

/// Extract a Solana blockchain transaction into our database model
pub fn extract_transaction(
    signature: &str,
    tx_data: &EncodedConfirmedTransactionWithStatusMeta,
) -> Option<Transaction> {
    // Extract basic info
    let slot = tx_data.slot as i64;
    let block_time = tx_data.block_time.unwrap_or(0);
    
    // Extract transaction and meta
    let transaction_with_meta = &tx_data.transaction;
    
    // Access the transaction field and match on its type
    let transaction = match &transaction_with_meta.transaction {
        EncodedTransaction::Json(tx) => tx,
        _ => {
            warn!("Unsupported transaction encoding for {}", signature);
            return None;
        }
    };
    
    // Extract metadata
    let meta = match &transaction_with_meta.meta {
        Some(meta) => meta,
        None => {
            warn!("Transaction {} has no metadata", signature);
            return None;
        }
    };
    
    // Extract message
    let message = match &transaction.message {
        UiMessage::Parsed(message) => message,
        UiMessage::Raw(_) => {
            warn!("Transaction {} has unparsed message", signature);
            return None;
        }
    };
    
    // Basic transaction info
    let success = meta.err.is_none();
    
    // Extract source address (first signer)
    let source_address = if !message.account_keys.is_empty() {
        message.account_keys[0].pubkey.clone()
    } else {
        warn!("Transaction {} has no account keys", signature);
        return None;
    };
    
    // Try to extract destination address and amount from instructions
    let (destination_address, amount, program_id) = extract_transfer_details(message, meta);
    
    Some(Transaction::new(
        signature.to_string(),
        block_time,
        slot,
        source_address,
        destination_address,
        amount,
        program_id,
        success,
    ))
}

fn extract_transfer_details(
    message: &UiParsedMessage,
    meta: &UiTransactionStatusMeta,
) -> (Option<String>, Option<f64>, Option<String>) {
    // Default values
    let mut destination = None;
    let mut amount = None;
    let mut program_id = None;
    
    // Check the instructions
    if !message.instructions.is_empty() {
        let first_instr = &message.instructions[0];
        
        // Extract based on instruction type
        match first_instr {
            UiInstruction::Parsed(parsed_instr) => {
                // For parsed instructions, access as JSON object
                let instr_value = serde_json::to_value(parsed_instr).unwrap_or(serde_json::Value::Null);
                
                if let Value::Object(obj) = instr_value {
                    // Try to extract program ID
                    if let Some(prog_id) = obj.get("programId") {
                        if let Some(id_str) = prog_id.as_str() {
                            program_id = Some(id_str.to_string());
                            
                            // System Program transfers
                            if id_str == SYSTEM_PROGRAM_ID.to_string() {
                                if let Some(parsed) = obj.get("parsed") {
                                    if let Some(typ) = parsed.get("type").and_then(|t| t.as_str()) {
                                        if typ == "transfer" || typ == "transferWithSeed" {
                                            if let Some(info) = parsed.get("info") {
                                                // Extract destination
                                                if let Some(dest) = info.get("destination").and_then(|d| d.as_str()) {
                                                    destination = Some(dest.to_string());
                                                }
                                                
                                                // Extract amount
                                                if let Some(lamports) = info.get("lamports").and_then(|l| l.as_u64()) {
                                                    amount = Some((lamports as f64) / 1_000_000_000.0);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Token Program transfers
                            else if id_str == TOKEN_PROGRAM_ID.to_string() {
                                if let Some(parsed) = obj.get("parsed") {
                                    if let Some(typ) = parsed.get("type").and_then(|t| t.as_str()) {
                                        if typ == "transfer" || typ == "transferChecked" {
                                            if let Some(info) = parsed.get("info") {
                                                // Extract destination
                                                if let Some(dest) = info.get("destination").and_then(|d| d.as_str()) {
                                                    destination = Some(dest.to_string());
                                                }
                                                
                                                // Extract amount
                                                if let Some(amt_str) = info.get("amount").and_then(|a| a.as_str()) {
                                                    if let Ok(amt_val) = amt_str.parse::<f64>() {
                                                        amount = Some(amt_val);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            UiInstruction::Compiled(compiled) => {
                // Just extract program ID from compiled instruction index
                if compiled.program_id_index < message.account_keys.len() as u8 {
                    let idx = compiled.program_id_index as usize;
                    program_id = Some(message.account_keys[idx].pubkey.clone());
                }
            }
        }
    }
    
    // If we still couldn't extract the details, try to infer from balance changes
    if destination.is_none() && !meta.post_balances.is_empty() && !meta.pre_balances.is_empty() {
        // This is a simple heuristic and may not work in all cases
        let mut largest_decrease = 0i64;
        let mut largest_increase = 0i64;
        let mut src_idx = 0;
        let mut dst_idx = 0;
        
        for (i, (pre, post)) in meta.pre_balances.iter().zip(meta.post_balances.iter()).enumerate() {
            let change = *post as i64 - *pre as i64;
            if change < -largest_decrease {
                largest_decrease = -change;
                src_idx = i;
            } else if change > largest_increase {
                largest_increase = change;
                dst_idx = i;
            }
        }
        
        if largest_decrease > 0 && largest_increase > 0 && src_idx != dst_idx {
            // We found a potential transfer
            if dst_idx < message.account_keys.len() {
                destination = Some(message.account_keys[dst_idx].pubkey.clone());
                amount = Some((largest_increase as f64) / 1_000_000_000.0); // Convert lamports to SOL
            }
        }
    }
    
    (destination, amount, program_id)
}

