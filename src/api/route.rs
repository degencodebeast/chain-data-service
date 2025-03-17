use crate::{
    api::{
        error::ApiError,
        response::{ApiResponse, with_total_count},
    },
    db::{address, transaction},
    state::AppState,
    cache,
    validation::{validate_solana_address, validate_action, validate_timestamp, validate_offset, validate_limit},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};

// POST /track endpoint query parameters
#[derive(Deserialize)]
pub struct TrackQuery {
    address: String,
    action: String,
}

// GET /transactions endpoint query parameters
#[derive(Deserialize)]
pub struct TransactionsQuery {
    address: String,
    start_time: String,
    end_time: String,
    offset: String,
    limit: String,
}

// Create router with all routes
pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/track", post(track_address))
        .route("/transactions", get(get_transactions))
        .with_state(app_state)
}

// POST /track handler
async fn track_address(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TrackQuery>,
) -> Result<Response, ApiError> {
    info!("Processing track request for address: {}, action: {}", params.address, params.action);
    
    // Validate address format
    validate_solana_address(&params.address)
        .map_err(|_| ApiError::InvalidAddress)?;
    
    // Validate action
    let action = params.action.to_lowercase();
    if action != "add" && action != "remove" {
        return Err(ApiError::InvalidParameter(
            "Action must be either 'add' or 'remove'".to_string()
        ));
    }
    
    // Execute the appropriate database operation
    let db_pool = &state.db_pool;
    
    match action.as_str() {
        "add" => {
            address::add_address(db_pool, &params.address).await?;
            info!("Added address {} to tracking", params.address);
            Ok((StatusCode::CREATED, "Address added to tracking").into_response())
        }
        "remove" => {
            let removed = address::remove_address(db_pool, &params.address).await?;
            if removed {
                info!("Removed address {} from tracking", params.address);
                Ok((StatusCode::OK, "Address removed from tracking").into_response())
            } else {
                info!("Address {} was not being tracked", params.address);
                Ok((StatusCode::OK, "Address was not being tracked").into_response())
            }
        }
        _ => unreachable!() // We've already validated the action above
    }
}

// GET /transactions handler
async fn get_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TransactionsQuery>,
) -> Result<Response, ApiError> {
    // Validate all parameters
    validate_solana_address(&params.address)
        .map_err(|_| ApiError::InvalidAddress)?;
    
    let start_time = validate_timestamp(&params.start_time)?;
    let end_time = validate_timestamp(&params.end_time)?;
    let offset = validate_offset(&params.offset)?;
    let limit = validate_limit(&params.limit)?;
    
    // Validate that start_time comes before end_time
    if start_time >= end_time {
        return Err(ApiError::InvalidParameter(
            "start_time must be less than end_time".to_string()
        ));
    }
    
    // Check if the address is being tracked
    let is_tracked = address::is_address_tracked(&state.db_pool, &params.address).await?;
    if !is_tracked {
        return Err(ApiError::NotFound("Address is not being tracked".to_string()));
    }
    
    info!("Fetching transactions for address: {}, time range: {}-{}, offset: {}, limit: {}", 
          params.address, start_time, end_time, offset, limit);
    
    // Generate cache key
    let cache_key = cache::generate_cache_key(&params.address, start_time, end_time);
    
    // Try to get from cache first
    let cache_lock = state.cache.lock().await;
    if let Some(cached_transactions) = cache_lock.get(&cache_key).await {
        info!("Cache hit for key: {}", cache_key);
        
        // Apply pagination to cached result
        let total_count = cached_transactions.len() as i64;
        let paginated_transactions = cached_transactions
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect::<Vec<_>>();
        
        drop(cache_lock); // Release the lock
        
        // Return response with total count header
        return Ok(with_total_count(paginated_transactions, total_count));
    }
    
    // Cache miss, release lock and query database
    drop(cache_lock);
    info!("Cache miss for key: {}", cache_key);
    
    // Query database
    let (transactions, total_count) = transaction::get_transactions(
        &state.db_pool,
        &params.address,
        start_time,
        end_time,
        offset,
        limit,
    ).await?;
    
    // Store in cache for future requests if we have results and it's not a paginated result
    if offset == 0 && transactions.len() as i64 == total_count && !transactions.is_empty() {
        // Acquire lock again for cache update
        let mut cache_lock = state.cache.lock().await;
        cache_lock.insert(cache_key.clone(), transactions.clone()).await;
        info!("Added results to cache with key: {}", cache_key);
    }
    
    // Return response with total count header
    Ok(with_total_count(transactions, total_count))
}
