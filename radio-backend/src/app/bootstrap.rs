//! Application bootstrap: config loading, engine startup, shared state,
//! router layers, and TCP listener setup.

use crate::app::state::AppState;
use axum::{
    http::{header, Method},
    middleware,
};
use radio_engine::config::BUFFER_CAPACITY;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = crate::config::AppConfig::load_default()?;
    tracing::info!(
        "Starting {} on port {} with base path {}",
        config.station.name,
        config.server.port,
        config.server.base_path
    );

    // 部署安全检查：提权令牌未配置或仍为默认值时告警。
    let setup_token = config.device.admin_setup_token.trim();
    if setup_token.is_empty() {
        tracing::warn!(
            "ADMIN SETUP TOKEN IS NOT CONFIGURED — admin claim is disabled. \
             Set [device] admin_setup_token in config.toml to enable admin setup."
        );
    } else if setup_token == "change-me-in-production" {
        tracing::warn!(
            "admin_setup_token is still the DEFAULT 'change-me-in-production' — \
             anyone who knows the default can claim admin! Change it in config.toml \
             before exposing this instance publicly."
        );
    }

    // 初始化音频引擎
    let media_path = config.audio_engine.media_path.clone();
    let crossfade_enabled = config.audio_engine.crossfade_enabled;
    let ring_buffer = radio_engine::ring_buffer::RingBuffer::new(BUFFER_CAPACITY);
    let (mut player, player_handle) =
        radio_engine::player::Player::new_with_crossfade(
            ring_buffer.clone(),
            media_path.clone(),
            crossfade_enabled,
        );

    // 初始化播放队列并启动播放器
    player.init_play_queue().await;
    tokio::spawn(async move { player.run().await });

    tracing::info!("Audio engine started, media path: {}", media_path);

    // 初始化应用状态
    let state = Arc::new(AppState::new(config, ring_buffer, player_handle).await?);

    // 把 DB 里 status='pending' 的曲子重新装回引擎请求队列（重启续播）。
    if let Err(e) = crate::world::WorldRuntime::new(state.clone())
        .rehydrate_playlist()
        .await
    {
        tracing::error!("Failed to rehydrate engine queue from DB: {:?}", e);
    }

    // 启动引擎状态轮询器，将播放状态转发给 WebSocket 客户端
    crate::services::playback_broadcast::start_engine_state_poller(state.clone());

    // 定期清理僵尸设备用户（启动时一次，之后每 24h 一次）
    {
        let db = state.db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 3600));
            loop {
                match crate::services::cleanup::prune_inactive_default_users(&db).await {
                    Ok(n) if n > 0 => {
                        tracing::info!("Pruned {} inactive default device users", n);
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Device cleanup task failed: {:?}", e),
                }
                interval.tick().await;
            }
        });
    }

    // 构建路由
    let app = crate::routes::build_router(state.clone())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::http::middleware::device_cookie_middleware,
        ))
        .layer(cors_layer(&state.config.server));

    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    tracing::info!("HTTP server listening on http://{}", addr);

    let listener = bind_with_keepalive(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn cors_layer(config: &crate::config::ServerConfig) -> CorsLayer {
    let custom_token_header = header::HeaderName::from_static("x-device-token");
    let allowed_methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::OPTIONS,
    ];
    let allowed_headers = [
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::AUTHORIZATION,
        custom_token_header.clone(),
    ];

    match config.allowed_origins.as_deref() {
        Some(origins) if !origins.is_empty() => {
            let origins: Vec<axum::http::HeaderValue> = origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .expose_headers([custom_token_header])
                .allow_credentials(true)
        }
        _ => CorsLayer::new()
            .allow_methods(allowed_methods)
            .allow_headers(allowed_headers)
            .expose_headers([custom_token_header]),
    }
}

/// Bind a TCP listener with TCP keepalive enabled on the listening socket.
///
/// Why: hyper, when serving an HTTP/1.1 streaming response that hasn't tried
/// to write yet, doesn't actively poll the TCP read side, so an abrupt client
/// disappearance (e.g. `timeout 1 curl`) leaves the server-side socket stuck
/// in ESTABLISHED forever. The tokio task spawned per stream stays parked in
/// `wait_for_data`, the ring-buffer reader is never dropped, and the fd is
/// never released — fd/conn/task accumulate indefinitely under load.
///
/// On Linux SO_KEEPALIVE plus the per-socket TCP_KEEPIDLE/INTVL/CNT options
/// are inherited by accepted sockets, so setting them on the listening socket
/// is enough. With these settings the kernel kills idle dead connections
/// within ~50s, which lets hyper drop the body and our task exit cleanly.
async fn bind_with_keepalive(addr: &str) -> anyhow::Result<tokio::net::TcpListener> {
    use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
    use std::net::SocketAddr;
    use std::time::Duration;

    let socket_addr: SocketAddr = addr.parse()?;
    let domain = if socket_addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_address(true)?;

    #[allow(unused_mut)]
    let mut keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(20))
        .with_interval(Duration::from_secs(10));
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        keepalive = keepalive.with_retries(3);
    }
    socket.set_tcp_keepalive(&keepalive)?;

    socket.bind(&socket_addr.into())?;
    socket.listen(1024)?;

    let std_listener: std::net::TcpListener = socket.into();
    let listener = tokio::net::TcpListener::from_std(std_listener)?;
    Ok(listener)
}
