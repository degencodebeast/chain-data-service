use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use crate::validation::ValidationError;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Invalid request: {0}")]
    BadRequest(String),
    
    #[error("Resource not found: {0}")]
    NotFound(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("Invalid address format")]
    InvalidAddress,
    
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error occurred".to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
            ApiError::InvalidAddress => (StatusCode::BAD_REQUEST, "Invalid blockchain address format".to_string()),
            ApiError::InvalidParameter(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        
        let body = Json(json!({
            "error": message,
        }));
        
        (status, body).into_response()
    }
}

// Implement From<ValidationError> for ApiError
impl From<ValidationError> for ApiError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::InvalidSolanaAddress(_) => ApiError::InvalidAddress,
            ValidationError::InvalidAction(msg) => 
                ApiError::InvalidParameter(format!("Invalid action: {}", msg)),
            ValidationError::MissingParameter(param) => 
                ApiError::BadRequest(format!("Missing parameter: {}", param)),
            ValidationError::InvalidParameter(msg) => 
                ApiError::InvalidParameter(msg),
        }
    }
}
