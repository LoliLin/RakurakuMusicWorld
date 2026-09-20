//! Shared application state passed to Axum handlers.

use crate::config::{AppConfig, StationConfig};
use crate::world::WorldCommandDispatcher;
use dashmap::DashMap;
use radio_engine::player::PlayerHandle;
use radio_engine::ring_buffer::RingBuffer;
use sqlx::SqlitePool;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

/// 在线听众信息
#[derive(Debug, Clone)]
pub struct OnlineListener {
    pub display_name: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
}

/// 所有请求处理器共享的应用状态。
pub struct AppState {
    pub db: SqlitePool,
    pub config: AppConfig,
    pub station: RwLock<StationConfig>,
    pub ws_tx: tokio::sync::broadcast::Sender<String>,
    /// 音频引擎的环形缓冲区（用于流式传输）
    pub ring_buffer: Arc<RingBuffer>,
    /// 音频引擎的播放器句柄（用于发送命令、获取状态）
    pub player_handle: PlayerHandle,
    /// Logical Side command adapter used by background workers and routes.
    pub world_commands: WorldCommandDispatcher,
    /// Serializes DB queue mutations with embedded-engine request queue updates.
    pub queue_sync: tokio::sync::Mutex<()>,
    /// 在线听众注册表 (device_token -> OnlineListener)
    pub listeners: Arc<DashMap<String, OnlineListener>>,
    /// 最近一条含全量歌词的 playback_state 消息（JSON）。
    /// 新 WebSocket 连接建立时补发，避免重连客户端永远收不到当前歌曲的歌词。
    pub ws_full_snapshot: std::sync::RwLock<Option<String>>,
    /// Incremented whenever DB metadata/assets change, allowing the playback
    /// snapshot cache to refresh without polling the songs table every tick.
    pub metadata_revision_signal: Arc<AtomicU64>,
    pub metadata_jobs: crate::services::metadata_jobs::MetadataJobManager,
}

impl AppState {
    /// 创建包含所有已初始化组件的新 AppState。
    pub async fn new(
        config: AppConfig,
        ring_buffer: Arc<RingBuffer>,
        player_handle: PlayerHandle,
    ) -> anyhow::Result<Self> {
        let db = crate::db::init_database(&config.database).await?;
        let (ws_tx, _) = tokio::sync::broadcast::channel(1024);
        let station = RwLock::new(config.station.clone());
        let metadata_revision_signal = Arc::new(AtomicU64::new(0));
        let world_commands = WorldCommandDispatcher::new(player_handle.clone());
        let metadata_jobs = crate::services::metadata_jobs::MetadataJobManager::new(
            db.clone(),
            std::path::PathBuf::from(&config.audio_engine.media_path),
            world_commands.clone(),
            metadata_revision_signal.clone(),
        )
        .await;

        Ok(Self {
            db,
            config,
            station,
            ws_tx,
            ring_buffer,
            world_commands,
            player_handle,
            queue_sync: tokio::sync::Mutex::new(()),
            listeners: Arc::new(DashMap::new()),
            ws_full_snapshot: std::sync::RwLock::new(None),
            metadata_revision_signal,
            metadata_jobs,
        })
    }
}
