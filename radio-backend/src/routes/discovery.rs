use std::sync::Arc;
use axum::{routing::{get, post}, Json, Router};
use serde_json::json;

use crate::app::state::AppState;
use crate::error::AppError;

pub fn discovery_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/worlds", get(list_discovered_worlds))
        .route("/scan", post(scan_lan_worlds))
}

/// GET /api/discovery/worlds — 返回局域网内当前探测到的对端 World 列表
async fn list_discovered_worlds(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let worlds = match &state.discovery {
        Some(svc) => svc.get_discovered_worlds(),
        None => Vec::new(),
    };

    Ok(Json(json!({
        "success": true,
        "data": worlds,
        "discovery_enabled": state.discovery.is_some(),
    })))
}

/// POST /api/discovery/scan — 触发局域网广播探测
async fn scan_lan_worlds(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    match &state.discovery {
        Some(svc) => {
            svc.probe().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to broadcast LAN probe: {}", e))
            })?;
            Ok(Json(json!({
                "success": true,
                "data": "Probe broadcasted",
            })))
        }
        None => Err(AppError::BadRequest("LAN discovery is disabled".into())),
    }
}
