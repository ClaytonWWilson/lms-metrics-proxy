use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("LM Studio connection error: {0}")]
    LmStudioConnection(String),

    #[error("Invalid response from LM Studio: {0}")]
    InvalidResponse(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ProxyError::LmStudioConnection(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ProxyError::InvalidResponse(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ProxyError::Database(_) => {
                tracing::error!("Database error: {}", self);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            ProxyError::Http(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ProxyError::Json(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ProxyError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
                "type": "proxy_error",
            }
        }));

        (status, body).into_response()
    }
}
