use crate::app::state::AppState;
/// 队列路由：查看、添加、移动、移除、跳过、历史记录、正在播放。
use crate::auth;
use crate::error::AppError;
use crate::models::{AddToQueueRequest, ApiResponse, MoveQueueItemRequest, NowPlaying};
use crate::world::WorldRuntime;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;

pub fn queue_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_queue).post(add_to_queue))
        .route("/:id", delete(remove_queue_item))
        .route("/:id/move", post(move_queue_item))
        .route("/skip", post(skip_current))
        .route("/history", get(get_history))
}

/// GET /api/queue — 获取当前队列（公开，普通用户脱敏点歌人）
async fn get_queue(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::QueueItemDisplay>>>, AppError> {
    let is_admin = auth::optional_device_auth(&headers, &state.db)
        .await
        .map(|user| user.role == "admin")
        .unwrap_or(false);
    let mut items = WorldRuntime::new(state.clone()).queue_display().await?;
    if !is_admin {
        for item in &mut items {
            item.requested_by = "匿名".into();
        }
    }
    Ok(Json(ApiResponse::ok(items)))
}

/// POST /api/queue — 添加歌曲到队列（需要认证）
async fn add_to_queue(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AddToQueueRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let device = auth::require_device_auth(&headers, &state.db).await?;
    let item_id = WorldRuntime::new(state.clone())
        .add_track(req.song_id, device.id, &device.display_name)
        .await?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "queue_item_id": item_id,
        "message": "Song added to queue",
    }))))
}

/// DELETE /api/queue/{id} — 从队列中移除项目（仅限管理员）
async fn remove_queue_item(
    State(state): State<Arc<AppState>>,
    Path(item_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let device = auth::require_device_auth(&headers, &state.db).await?;
    auth::require_admin(&device)?;

    WorldRuntime::new(state.clone())
        .remove_track(item_id)
        .await?;

    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'remove_queue', ?)")
        .bind(device.id)
        .bind(format!("Removed queue item {}", item_id))
        .execute(&state.db)
        .await?;

    Ok(Json(ApiResponse::ok("Queue item removed".into())))
}

/// POST /api/queue/{id}/move — 移动队列项目到新位置（仅限管理员）
async fn move_queue_item(
    State(state): State<Arc<AppState>>,
    Path(item_id): Path<i64>,
    headers: HeaderMap,
    Json(req): Json<MoveQueueItemRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let device = auth::require_device_auth(&headers, &state.db).await?;
    auth::require_admin(&device)?;

    WorldRuntime::new(state.clone())
        .move_track(item_id, req.new_position)
        .await?;

    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'move_queue', ?)")
        .bind(device.id)
        .bind(format!(
            "Moved item {} to position {}",
            item_id, req.new_position
        ))
        .execute(&state.db)
        .await?;

    Ok(Json(ApiResponse::ok("Queue item moved".into())))
}

/// POST /api/queue/skip — 跳过当前歌曲（仅限管理员）
async fn skip_current(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let device = auth::require_device_auth(&headers, &state.db).await?;
    auth::require_admin(&device)?;

    WorldRuntime::new(state.clone()).skip_track().await?;

    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'skip_track', 'Skipped current track')")
        .bind(device.id)
        .execute(&state.db)
        .await?;

    Ok(Json(ApiResponse::ok("Track skipped".into())))
}

/// GET /api/queue/history — 最近播放历史（仅管理员）
async fn get_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let device = auth::require_device_auth(&headers, &state.db).await?;
    auth::require_admin(&device)?;

    let history = WorldRuntime::new(state.clone()).history(20).await?;
    Ok(Json(ApiResponse::ok(history)))
}

/// GET /api/now-playing — 获取当前播放曲目与状态
pub async fn now_playing(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse<NowPlaying>>, AppError> {
    let np = WorldRuntime::new(state).now_playing(Some(&headers)).await;
    Ok(Json(ApiResponse::ok(np)))
}

