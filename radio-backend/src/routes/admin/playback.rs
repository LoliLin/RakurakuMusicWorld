/// 播放控制路由。
use crate::app::state::AppState;
use crate::error::AppError;
use crate::models::ApiResponse;
use crate::routes::admin::get_admin;
use crate::world::{WorldCommand, WorldRuntime};
use axum::{extract::State, http::HeaderMap, Json};
use std::sync::Arc;

/// POST /api/admin/playlist/next — 切到下一首
pub async fn skip_next(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let _admin = get_admin(&state, &headers).await?;

    WorldRuntime::new(state.clone()).dispatch(WorldCommand::Skip);

    Ok(Json(ApiResponse::ok("已切到下一首".into())))
}

/// POST /api/admin/playlist/prev — 切到上一首
pub async fn skip_prev(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let _admin = get_admin(&state, &headers).await?;

    WorldRuntime::new(state.clone()).dispatch(WorldCommand::Prev);

    Ok(Json(ApiResponse::ok("已切到上一首".into())))
}
