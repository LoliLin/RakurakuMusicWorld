use serde::{Deserialize, Serialize};

/// 歌词行 DTO（用于 WebSocket 序列化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLineDto {
    pub time_ms: Option<i64>,
    pub text: String,
}

/// 播放状态枚举（从引擎 re-export）。
pub use radio_engine::types::PlaybackStatus;

/// 发送给已连接浏览器的 WebSocket 消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "playback_state")]
    PlaybackState {
        song_id: i64,
        title: String,
        artist: String,
        position_ms: i64,
        duration_ms: i64,
        lyrics_line: Option<usize>,
        lyrics_lines: Option<Vec<LyricsLineDto>>,
        status: PlaybackStatus,
        stream_url: String,
        file_url: Option<String>,
        cover_url: Option<String>,
        timestamp_ms: i64,
    },
    #[serde(rename = "queue_update")]
    QueueUpdate {
        action: String,
        song_title: Option<String>,
        requested_by: Option<String>,
        queue_size: usize,
    },
    #[serde(rename = "notice")]
    Notice {
        message: String,
        level: String, // info | warning | error
    },
    #[serde(rename = "ping")]
    Ping { timestamp: i64 },
    #[serde(rename = "listeners_update")]
    ListenersUpdate { count: usize, names: Vec<String> },

    /// Stage 2 事件：曲目切换（song_change 发生时，与 playback_state 全量帧
    /// 同拍广播）。字段是 playback_state 的子集——旧前端 default 分支忽略，
    /// 新前端可据此做曲目切换动效。不会取代 playback_state。
    #[serde(rename = "track_changed")]
    TrackChanged {
        song_id: i64,
        title: String,
        artist: String,
        duration_ms: i64,
        status: PlaybackStatus,
        timestamp_ms: i64,
    },
}
