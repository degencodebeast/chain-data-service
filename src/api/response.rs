use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let json = match serde_json::to_string(&self) {
            Ok(json) => json,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );

        (StatusCode::OK, headers, json).into_response()
    }
}

pub fn with_total_count<T: Serialize>(data: T, count: i64) -> Response {
    let json = match serde_json::to_string(&ApiResponse { data }) {
        Ok(json) => json,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    headers.insert(
        "X-Total-Count",
        count.to_string().parse().unwrap(),
    );

    (StatusCode::OK, headers, json).into_response()
}
