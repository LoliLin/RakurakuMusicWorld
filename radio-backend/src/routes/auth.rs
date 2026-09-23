use crate::app::state::AppState;
/// 设备认证路由：获取当前设备身份、设置显示名称。
use crate::auth;
use crate::error::AppError;
use crate::models::{ApiResponse, SetDisplayNameRequest};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

pub fn auth_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/me", get(get_me))
        .route("/name", post(set_display_name))
}

/// GET /api/auth/me — 获取当前设备信息。
/// 本地回环（127.0.0.1）访问者自动确认为房主/管理员（OP）；
/// 远程连接者自动确认为普通玩家/听众。
async fn get_me(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let device_token = auth::extract_device_token(&headers).ok_or(AppError::Unauthorized)?;

    let is_loopback = addr.ip().is_loopback();
    let is_really_local = is_loopback && {
        if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            forwarded.split(',').all(|ip_str| {
                ip_str.trim().parse::<std::net::IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
            })
        } else {
            true
        }
    };

    let user = if is_really_local {
        auth::ensure_local_admin(&state.db, &device_token).await?
    } else {
        auth::ensure_device_user(&state.db, &device_token).await?
    };

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "id": user.id,
        "display_name": user.display_name,
        "role": user.role,
        "is_local": is_really_local,
    }))))
}

/// POST /api/auth/name — 设置当前设备的显示名称
async fn set_display_name(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SetDisplayNameRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let user = auth::require_device_auth(&headers, &state.db).await?;

    let name = req.display_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Display name cannot be empty".into()));
    }
    if name.len() > 32 {
        return Err(AppError::BadRequest(
            "Display name must be 32 characters or less".into(),
        ));
    }

    sqlx::query("UPDATE device_users SET display_name = ? WHERE id = ?")
        .bind(name)
        .bind(user.id)
        .execute(&state.db)
        .await?;

    Ok(Json(ApiResponse::ok(format!(
        "Display name set to '{}'",
        name
    ))))
}
