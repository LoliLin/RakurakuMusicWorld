pub mod admin;
/// 电台后端 HTTP API 的路由模块。
pub mod auth;
pub mod favorites;
pub mod ncm;
pub mod playlist;
pub mod queue;
pub mod songs;
pub mod station;

use crate::app::state::AppState;
use crate::config::join_base_path;
use axum::{http::StatusCode, routing::get, Json, Router};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

/// 构建组合的应用程序路由器。
pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        .nest("/auth", auth::auth_routes())
        .nest("/songs", songs::song_routes())
        .nest("/playlists", playlist::playlist_routes())
        .nest("/queue", queue::queue_routes())
        .nest("/admin", admin::admin_routes())
        .nest("/ncm", ncm::ncm_routes())
        .nest("/favorites", favorites::favorites_routes())
        .route("/station", get(station::station_info))
        .route("/now-playing", get(queue::now_playing))
        .route("/listeners", get(get_listeners))
        .fallback(api_not_found);

    let mut app_routes = Router::new()
        .route("/ws", get(crate::websocket::ws_handler))
        .route("/stream", get(crate::http::stream::stream_handler))
        .route("/manifest.json", get(station::manifest))
        .route("/site-icon", get(admin::settings::site_icon))
        .nest("/api", api_routes);

    if !state.config.server.headless {
        app_routes = app_routes.fallback_service(
            ServeDir::new("static").fallback(ServeFile::new("static/index.html")),
        );
    } else {
        app_routes = app_routes
            .route("/", get(dedicated_server_root))
            .fallback(api_not_found);
    }

    let base_path = state.config.server.base_path.clone();
    let router = if base_path == "/" {
        app_routes
    } else {
        Router::new()
            .nest(&base_path, app_routes)
            .route("/", get(redirect_to_base_path))
    };

    router.with_state(state)
}

async fn redirect_to_base_path(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::response::Redirect {
    axum::response::Redirect::temporary(&join_base_path(&state.config.server.base_path, "/"))
}

async fn api_not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "success": false,
            "error": "API endpoint not found",
        })),
    )
}

/// Headless / Dedicated Server 模式下的根路径响应。
async fn dedicated_server_root(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let station = state.station.read().unwrap_or_else(|e| e.into_inner());
    Json(serde_json::json!({
        "service": "RakurakuMusicWorld Dedicated Server",
        "mode": "headless",
        "world_id": state.world_id,
        "station_name": station.name,
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "api": join_base_path(&state.config.server.base_path, "/api"),
            "ws": join_base_path(&state.config.server.base_path, "/ws"),
            "stream": join_base_path(&state.config.server.base_path, "/stream"),
        }
    }))
}

/// 获取当前在线听众列表
async fn get_listeners(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let count = state.listeners.len();
    let listeners: Vec<serde_json::Value> = state.listeners.iter().map(|entry| {
        serde_json::json!({
            "display_name": entry.value().display_name,
            "connected_at": entry.value().connected_at.to_rfc3339(),
        })
    }).collect();

    Json(serde_json::json!({
        "success": true,
        "count": count,
        "listeners": listeners,
    }))
}
