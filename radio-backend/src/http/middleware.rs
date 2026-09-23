//! HTTP middleware that is not tied to a specific route group.

use crate::app::state::AppState;
use axum::{
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

/// 中间件：确保每个浏览器都有一个 device_token Cookie。
///
/// 不在这里创建数据库用户。该中间件也会处理静态资源、公开 API 和
/// 机器人请求；把这些请求登记为用户会让设备用户表无上限增长。
pub async fn device_cookie_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let is_secure = request_is_secure(request.headers());
    let device_token = crate::auth::extract_device_token(request.headers());
    let new_token = if device_token.is_none() {
        let new_token = crate::auth::generate_device_token();

        let mut cookie_header = request
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if !cookie_header.is_empty() {
            cookie_header.push_str("; ");
        }
        cookie_header.push_str("device_token=");
        cookie_header.push_str(&new_token);

        if let Ok(val) = header::HeaderValue::from_str(&cookie_header) {
            request.headers_mut().insert(header::COOKIE, val);
        }

        Some(new_token)
    } else {
        None
    };

    // 检查请求是否来自本机回环地址（本地房主/OP判定）
    let is_loopback = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().is_loopback())
        .unwrap_or(false);

    let is_really_local = is_loopback && {
        if let Some(forwarded) = request.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            forwarded.split(',').all(|ip_str| {
                ip_str.trim().parse::<std::net::IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
            })
        } else {
            true
        }
    };

    // 若为本机回环访问 /api/*，自动确保该设备令牌在数据库中登记为最高管理员（房主）。
    let effective_token_str = new_token.as_deref().or(device_token.as_deref()).map(|s| s.to_string());
    if is_really_local && request.uri().path().starts_with("/api") {
        if let Some(token) = &effective_token_str {
            let _ = crate::auth::ensure_local_admin(&state.db, token).await;
        }
    }

    let mut response = next.run(request).await;

    if let Some(new_token) = new_token.as_ref() {
        let max_age = state.config.device.cookie_max_age_days * 86400;
        let cookie_path = state.config.server.base_path.as_str();
        let secure_attr = if is_secure { "; Secure" } else { "" };
        let cookie_value = format!(
            "device_token={}; Path={}; HttpOnly; SameSite=Lax; Max-Age={}{}",
            new_token, cookie_path, max_age, secure_attr
        );

        if let Ok(val) = header::HeaderValue::from_str(&cookie_value) {
            response.headers_mut().insert(header::SET_COOKIE, val);
        }
    }

    // 将生效的 device_token 暴露在响应头中，供跨域或无 Cookie 环境（如移动端/原生客户端）存储与复用。
    let effective_token = new_token.as_deref().or(device_token.as_deref());
    if let Some(token) = effective_token {
        if let Ok(val) = header::HeaderValue::from_str(token) {
            response.headers_mut().insert("x-device-token", val);
        }
    }

    response
}

fn request_is_secure(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}
