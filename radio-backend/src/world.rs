//! World state model and Logical Side entry point.
//!
//! Stage 1 introduced `WorldState` as a translation/aggregation view over
//! the existing engine, SQLite, and listener authorities. Stage 3 adds
//! `WorldRuntime`, the narrow Logical Side façade used by HTTP/background
//! adapters. It does not duplicate state or create a second authority.
//!
//! The Integrated Physical Side still supplies `AppState`, SQLite, the
//! embedded engine, and listener transport. The façade is intentionally
//! small so later Physical Side extraction can replace those dependencies.
use std::sync::Arc;

use crate::app::state::AppState;
use crate::models::WsMessage;

/// World 身份。由 SQLite `world_meta` 表持久化 world_id（Stage 4 落地），
/// 显示身份从 config 派生。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldIdentity {
    /// 显示名（config `[station] name`）。
    pub name: String,
    /// 短名（config `[station] short_name`）。
    pub short_name: String,
    /// 稳定 ID。由 SQLite `world_meta` 表持久化，跨重启稳定。
    pub world_id: Option<String>,
}

impl WorldIdentity {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            name: state.config.station.name.clone(),
            short_name: state.config.station.short_name.clone(),
            world_id: Some(state.world_id.clone()),
        }
    }
}

/// World 播放状态视图。
///
/// 直接镜像 `radio_engine::types::PlaybackState`（唯一权威），不做加工——
/// 避免 Stage 1 引入第二份 playback 真相。serde 结构与 WS `playback_state`
/// 帧中的核心字段对齐（`models/ws.rs:PlaybackState`）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldPlayback {
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub status: crate::models::PlaybackStatus,
}

impl WorldPlayback {
    /// 从 Logical 权威快照翻译。
    pub fn from_now_playing(np: &crate::models::NowPlaying) -> Self {
        let (song_id, title, artist) = match &np.song {
            Some(s) => (s.id, s.title.clone(), s.artist.clone()),
            None => (-1, String::new(), String::new()),
        };
        let status = if np.duration_ms > 0 && np.position_ms >= 0 {
            crate::models::PlaybackStatus::Playing
        } else {
            crate::models::PlaybackStatus::Stopped
        };
        Self {
            song_id,
            title,
            artist,
            position_ms: np.position_ms,
            duration_ms: np.duration_ms,
            status,
        }
    }

    /// 从引擎物理进度翻译（fallback 兼容）。`song_id`：folder-cycle 曲目在 WS 帧里是 -1。
    pub fn from_engine(ps: &radio_engine::types::PlaybackState) -> Self {
        Self {
            song_id: ps.song_id.unwrap_or(-1),
            title: ps.title.clone(),
            artist: ps.artist.clone(),
            position_ms: ps.position_ms,
            duration_ms: ps.duration_ms,
            status: ps.status.clone(),
        }
    }
}

/// World 队列状态视图。
///
/// 只反映 DB 权威（`queue_items` pending/playing），不镜像引擎内存请求队列。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldPlaylist {
    /// pending + playing 数量（`services/queue.rs:queue_size` 同口径）。
    pub size: usize,
}

/// World 在线玩家状态视图。
///
/// 来源：`AppState.listeners`（WS 连接注册表，`websocket.rs:handle_socket`）。
/// count 为服务器权威数字；names 与前端一样做去重展示。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldPlayers {
    pub count: usize,
    pub names: Vec<String>,
}

/// 聚合后的 World 状态。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldState {
    pub identity: WorldIdentity,
    pub playback: WorldPlayback,
    pub playlist: WorldPlaylist,
    pub players: WorldPlayers,
}

impl WorldState {
    /// 从各权威来源聚合一次快照。
    pub async fn snapshot(state: &Arc<AppState>) -> Self {
        let playback = {
            let guard = state.current_playback.read().await;
            if let Some(np) = guard.as_ref() {
                WorldPlayback::from_now_playing(np)
            } else {
                let ps = state.player_handle.get_state();
                WorldPlayback::from_engine(&ps)
            }
        };

        let playlist = WorldPlaylist {
            size: crate::services::queue::queue_size(&state.db)
                .await
                .unwrap_or(0),
        };

        let mut names: Vec<String> = state
            .listeners
            .iter()
            .map(|entry| entry.value().display_name.clone())
            .collect();
        names.sort();
        names.dedup();
        let players = WorldPlayers {
            count: state.listeners.len(),
            names,
        };

        Self {
            identity: WorldIdentity::from_state(state),
            playback,
            playlist,
            players,
        }
    }
}

/// Logical command 到 Integrated Physical audio executor 的最小适配器。
///
/// 后台 worker 可能只持有 `PlayerHandle` 等执行依赖，不能构造完整
/// `WorldRuntime`；它们仍必须通过这个类型提交 World command。
#[derive(Clone)]
pub struct WorldCommandDispatcher {
    player_handle: radio_engine::player::PlayerHandle,
}

impl WorldCommandDispatcher {
    pub fn new(player_handle: radio_engine::player::PlayerHandle) -> Self {
        Self { player_handle }
    }

    pub fn dispatch(&self, command: WorldCommand) {
        self.player_handle
            .send_command(radio_engine::types::AudioCommand {
                cmd_type: command.into(),
                song_id: None,
                file_path: None,
            });
    }
}

/// Logical Side 的运行时入口。
///
/// `WorldRuntime` 只持有 Integrated Physical Side 提供的依赖，
/// 对外暴露的是 World command/query，而不是 `PlayerHandle` 或
/// `services::queue` 的实现细节。当前实现仍复用旧 service 和 engine；
/// 这个边界让后续迁移可以先替换逻辑实现，再抽 Physical Side。
#[derive(Clone)]
pub struct WorldRuntime {
    state: Arc<AppState>,
}

impl WorldRuntime {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// 聚合一次当前 World 快照。
    pub async fn snapshot(&self) -> WorldState {
        WorldState::snapshot(&self.state).await
    }

    /// 向 Physical Side 的播放执行器提交一个 World command。
    pub fn dispatch(&self, command: WorldCommand) {
        self.state.world_commands.dispatch(command);
    }

    /// 查询 Logical Side 的队列展示。
    pub async fn queue_display(
        &self,
    ) -> Result<Vec<crate::models::QueueItemDisplay>, crate::error::AppError> {
        crate::services::queue::get_queue_display(&self.state.db).await
    }

    /// 处理加入 World playlist 的请求。
    pub async fn add_track(
        &self,
        song_id: i64,
        device_user_id: i64,
        display_name: &str,
    ) -> Result<i64, crate::error::AppError> {
        crate::services::queue::add_to_queue(&self.state, song_id, device_user_id, display_name)
            .await
    }

    /// 处理管理员调整 World playlist 顺序的请求。
    pub async fn move_track(
        &self,
        item_id: i64,
        new_position: i32,
    ) -> Result<(), crate::error::AppError> {
        crate::services::queue::move_queue_item(&self.state, item_id, new_position).await
    }

    /// 处理管理员移除 World playlist 项目的请求。
    pub async fn remove_track(&self, item_id: i64) -> Result<(), crate::error::AppError> {
        crate::services::queue::remove_queue_item(&self.state, item_id).await
    }

    /// 处理管理员跳过当前曲目的请求。
    pub async fn skip_track(&self) -> Result<(), crate::error::AppError> {
        crate::services::queue::skip_current(&self.state).await
    }

    /// 查询 World 的最近播放历史。
    pub async fn history(
        &self,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>, crate::error::AppError> {
        crate::services::queue::get_history(&self.state.db, limit).await
    }

    /// 把持久化 playlist 重新装入音频执行器的请求队列。
    pub async fn rehydrate_playlist(&self) -> Result<(), crate::error::AppError> {
        crate::services::queue::rehydrate_engine_queue(&self.state).await
    }

    /// 将当前播放曲目的状态回写到 World playlist。
    pub async fn mark_track_playing(&self, song_id: i64) -> Result<(), crate::error::AppError> {
        crate::services::queue::mark_playing(&self.state.db, song_id).await
    }

    /// 从系统与队列中彻底清除歌曲，并同步更新底层音频执行队列。
    pub async fn purge_song(&self, song_id: i64) -> Result<(), crate::error::AppError> {
        crate::services::queue::purge_song(&self.state, song_id).await
    }

    /// 查询当前 Logical 权威播放状态。
    pub async fn now_playing(
        &self,
        headers: Option<&axum::http::HeaderMap>,
    ) -> crate::models::NowPlaying {
        let snapshot = {
            let guard = self.state.current_playback.read().await;
            guard.clone()
        };

        if let Some(mut np) = snapshot {
            np.stream_url = self.state.config.audio_engine.resolve_stream_url(
                headers,
                self.state.config.server.port,
                &self.state.config.server.base_path,
            );
            np
        } else {
            let progress = self.state.player_handle.get_progress();
            let stream_url = self.state.config.audio_engine.resolve_stream_url(
                headers,
                self.state.config.server.port,
                &self.state.config.server.base_path,
            );
            crate::models::NowPlaying {
                song: None,
                position_ms: progress.position_ms,
                duration_ms: progress.duration_ms,
                lyrics_line: None,
                lyrics_text: None,
                started_at: None,
                stream_url,
                file_url: None,
                cover_url: None,
            }
        }
    }
}

/// World 命令枚举 —— Stage 1 只翻译现有 `AudioCommandType`，
/// 不发明新命令（Seek 等留到有真实实现时再加）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldCommand {
    Skip,
    Next,
    Prev,
    Play,
    Stop,
    ReloadQueue,
}

impl From<WorldCommand> for radio_engine::types::AudioCommandType {
    fn from(cmd: WorldCommand) -> Self {
        match cmd {
            WorldCommand::Skip => radio_engine::types::AudioCommandType::Skip,
            WorldCommand::Next => radio_engine::types::AudioCommandType::Next,
            WorldCommand::Prev => radio_engine::types::AudioCommandType::Prev,
            WorldCommand::Play => radio_engine::types::AudioCommandType::Play,
            WorldCommand::Stop => radio_engine::types::AudioCommandType::Stop,
            WorldCommand::ReloadQueue => radio_engine::types::AudioCommandType::ReloadQueue,
        }
    }
}

/// 便捷：从最近一条 WS 广播消息提取当前播放视图（测试/调试用）。
#[allow(dead_code)]
pub fn playback_from_ws(msg: &WsMessage) -> Option<WorldPlayback> {
    match msg {
        WsMessage::PlaybackState {
            song_id,
            title,
            artist,
            position_ms,
            duration_ms,
            status,
            ..
        } => Some(WorldPlayback {
            song_id: *song_id,
            title: title.clone(),
            artist: artist.clone(),
            position_ms: *position_ms,
            duration_ms: *duration_ms,
            status: status.clone(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WorldCommand ↔ AudioCommandType 翻译必须完备且无损。
    #[test]
    fn command_translation_is_lossless() {
        // AudioCommandType 未实现 PartialEq，用 matches! 逐例断言。
        assert!(matches!(
            radio_engine::types::AudioCommandType::from(WorldCommand::Skip),
            radio_engine::types::AudioCommandType::Skip
        ));
        assert!(matches!(
            radio_engine::types::AudioCommandType::from(WorldCommand::Next),
            radio_engine::types::AudioCommandType::Next
        ));
        assert!(matches!(
            radio_engine::types::AudioCommandType::from(WorldCommand::Prev),
            radio_engine::types::AudioCommandType::Prev
        ));
        assert!(matches!(
            radio_engine::types::AudioCommandType::from(WorldCommand::Play),
            radio_engine::types::AudioCommandType::Play
        ));
        assert!(matches!(
            radio_engine::types::AudioCommandType::from(WorldCommand::Stop),
            radio_engine::types::AudioCommandType::Stop
        ));
        assert!(matches!(
            radio_engine::types::AudioCommandType::from(WorldCommand::ReloadQueue),
            radio_engine::types::AudioCommandType::ReloadQueue
        ));
    }

    /// folder-cycle 曲目（song_id: None）必须翻译为 WS 同款 -1 语义。
    #[test]
    fn playback_song_id_none_maps_to_minus_one() {
        let ps = radio_engine::types::PlaybackState {
            song_id: None,
            title: "Fallback".into(),
            ..Default::default()
        };
        let wp = WorldPlayback::from_engine(&ps);
        assert_eq!(wp.song_id, -1);
        assert_eq!(wp.title, "Fallback");
    }

    /// 请求曲目 song_id 必须原样保留。
    #[test]
    fn playback_song_id_some_preserved() {
        let ps = radio_engine::types::PlaybackState {
            song_id: Some(64),
            ..Default::default()
        };
        assert_eq!(WorldPlayback::from_engine(&ps).song_id, 64);
    }

    /// WS playback_state 帧 → WorldPlayback 的反向翻译字段一致。
    #[test]
    fn playback_from_ws_roundtrip() {
        let msg = WsMessage::PlaybackState {
            song_id: 7,
            title: "T".into(),
            artist: "A".into(),
            position_ms: 1000,
            duration_ms: 2000,
            lyrics_line: None,
            lyrics_lines: None,
            status: radio_engine::types::PlaybackStatus::Playing,
            stream_url: "/stream".into(),
            file_url: None,
            cover_url: None,
            timestamp_ms: 42,
        };
        let wp = playback_from_ws(&msg).expect("playback_state → Some");
        assert_eq!(wp.song_id, 7);
        assert_eq!(wp.position_ms, 1000);
        assert!(matches!(
            wp.status,
            crate::models::PlaybackStatus::Playing
        ));
    }
}
