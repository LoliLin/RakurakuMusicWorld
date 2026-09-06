//! World state model — Stage 1 of the Music World migration.
//!
//! 蓝图（docs/MIGRATION.md Stage 1）：把散布在 engine `PlaybackState`、
//! `queue_items` 表、`AppState.listeners` 中的"世界状态"翻译成统一的
//! `WorldState` 视图。本模块是**纯翻译/聚合层**：不拥有权威状态、
//! 不引入新字段、不改变任何现有行为。权威仍在原处：
//! - playback 权威：`radio_engine::types::PlaybackState`（engine 500ms 发布）
//! - playlist 权威：`queue_items` 表（`services/queue.rs`）
//! - players 权威：`device_users` 表 + `AppState.listeners`（WS 在线注册表）
use std::sync::Arc;

use crate::app::state::AppState;
use crate::models::WsMessage;

/// World 身份。第一阶段由 SQLite `world_meta` 表持久化 world_id（Stage 4 启用），
/// 当前从 config 派生显示身份。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldIdentity {
    /// 显示名（config `[station] name`）。
    pub name: String,
    /// 短名（config `[station] short_name`）。
    pub short_name: String,
    /// 稳定 ID。Stage 1 尚未持久化，返回 `None`；Stage 4 接入 `world_meta` 表。
    pub world_id: Option<String>,
}

impl WorldIdentity {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            name: state.config.station.name.clone(),
            short_name: state.config.station.short_name.clone(),
            world_id: None,
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
    /// 从引擎权威状态翻译。`song_id`：folder-cycle 曲目在 WS 帧里是 -1。
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
        let ps = state.player_handle.get_state();
        let playback = WorldPlayback::from_engine(&ps);

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

/// 把命令发给引擎执行器。等价于现有 `PlayerHandle::send_command` 路径，
/// 只是收敛了入口（`websocket.rs:publish_command` 的同义封装）。
pub fn execute(state: &AppState, cmd: WorldCommand) {
    let audio = radio_engine::types::AudioCommand {
        cmd_type: cmd.into(),
        song_id: None,
        file_path: None,
    };
    state.player_handle.send_command(audio);
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
