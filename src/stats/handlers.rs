use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::proxy::AppState;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    100
}

pub async fn get_summary(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, crate::error::ProxyError> {
    let stats = crate::db::get_summary_stats(&state.db).await?;
    Ok(Json(json!(stats)))
}

pub async fn get_by_model(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, crate::error::ProxyError> {
    let stats = crate::db::get_model_stats(&state.db).await?;
    Ok(Json(json!({ "models": stats })))
}

pub async fn get_recent(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<serde_json::Value>, crate::error::ProxyError> {
    let limit = params.limit.min(1000).max(1); // Cap at 1000
    let requests = crate::db::get_recent_requests(&state.db, limit).await?;
    Ok(Json(json!({ "requests": requests })))
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "token_counter_proxy"
    }))
}
