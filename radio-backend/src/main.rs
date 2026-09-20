#![allow(dead_code)]

/// RakurakuMusicWorld - Rust 业务后端
///
/// 处理 HTTP API、WebSocket 广播、设备身份验证（基于 Cookie）、
/// 队列管理、歌词解析，并内嵌音频引擎。
mod app;
mod auth;
mod config;
mod db;
mod error;
mod http;
mod lyrics;
mod models;
mod physical;
mod routes;
mod services;
mod websocket;
mod world;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::bootstrap::run().await
}
