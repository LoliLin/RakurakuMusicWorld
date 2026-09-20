/// 反映 SQL 模式的数据库模型。
/// SQLx 派生 FromRow 用于查询结果映射。
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

mod ws;
pub use ws::*;

// ─── 设备用户 ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceUser {
    pub id: i64,
    pub device_token: String,
    pub display_name: String,
    pub role: String,
    pub banned_until: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

impl DeviceUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    pub fn is_banned(&self) -> bool {
        match self.banned_until {
            Some(until) => until > chrono::Utc::now().naive_utc(),
            None => false,
        }
    }

    /// 生成默认显示名称 "Listener-XXXX"
    pub fn default_display_name(id: i64) -> String {
        format!("Listener-{:04}", id % 10000)
    }
}

// ─── 歌曲 ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Song {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: i32,
    pub duration_ms: i64,
    pub file_path: String,
    pub lyrics_path: String,
    pub cover_path: String,
    pub filesize: i64,
    pub created_at: NaiveDateTime,
    pub ncm_song_id: Option<i64>,
    pub metadata_source: String,
    pub metadata_matched_at: Option<NaiveDateTime>,
    pub musicbrainz_recording_id: Option<String>,
    pub metadata_revision: i64,
}

/// 歌曲摘要，用于列表响应（出于安全考虑省略路径）。
#[derive(Debug, Clone, Serialize)]
pub struct SongSummary {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub has_lyrics: bool,
    pub has_cover: bool,
    pub metadata_revision: i64,
}

impl From<Song> for SongSummary {
    fn from(s: Song) -> Self {
        Self {
            id: s.id,
            title: s.title,
            artist: s.artist,
            album: s.album,
            duration_ms: s.duration_ms,
            has_lyrics: !s.lyrics_path.is_empty(),
            has_cover: !s.cover_path.is_empty(),
            metadata_revision: s.metadata_revision,
        }
    }
}

// ─── 播放列表 ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Playlist {
    pub id: i64,
    pub device_user_id: i64,
    pub name: String,
    pub is_public: bool,
    pub created_at: NaiveDateTime,
}

/// 包含歌曲数量的播放列表。
#[derive(Debug, Serialize)]
pub struct PlaylistWithCount {
    pub id: i64,
    pub device_user_id: i64,
    pub name: String,
    pub is_public: bool,
    pub song_count: i64,
    pub created_at: NaiveDateTime,
}

// ─── 播放列表歌曲 ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlaylistSong {
    pub id: i64,
    pub playlist_id: i64,
    pub song_id: i64,
    pub position: i32,
    pub added_at: NaiveDateTime,
}

// ─── 队列条目 ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QueueItem {
    pub id: i64,
    pub song_id: i64,
    pub device_user_id: i64,
    pub status: String, // pending | playing | played | skipped
    pub position: i32,
    pub added_at: NaiveDateTime,
    pub played_at: Option<NaiveDateTime>,
}

/// 为 API 响应丰富了歌曲和设备信息的队列条目。
#[derive(Debug, Serialize)]
pub struct QueueItemDisplay {
    pub id: i64,
    pub song: Option<SongSummary>,
    pub requested_by: String,
    pub status: String,
    pub position: i32,
    pub added_at: NaiveDateTime,
}

// ─── 播放历史 ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlayHistory {
    pub id: i64,
    pub song_id: i64,
    pub device_user_id: Option<i64>,
    pub played_at: NaiveDateTime,
}

// ─── 管理日志 ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdminLog {
    pub id: i64,
    pub admin_id: i64,
    pub action: String,
    pub details: String,
    pub created_at: NaiveDateTime,
}

// ─── 收藏 ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Favorite {
    pub id: i64,
    pub device_user_id: i64,
    pub song_id: Option<i64>,
    pub playlist_id: Option<i64>,
    pub created_at: NaiveDateTime,
}

// ─── 请求 / 响应 DTO ────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    #[serde(default)]
    pub is_public: bool,
}

#[derive(Debug, Deserialize)]
pub struct AddSongToPlaylistRequest {
    pub song_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AddToQueueRequest {
    pub song_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct MoveQueueItemRequest {
    pub new_position: i32,
}

#[derive(Debug, Deserialize)]
pub struct AdminActionRequest {
    pub device_user_id: i64,
}

/// 设备用户角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    User,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::User => write!(f, "user"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetRoleRequest {
    pub role: Role,
}

/// 设置 / 更新设备显示名称的请求
#[derive(Debug, Deserialize)]
pub struct SetDisplayNameRequest {
    pub display_name: String,
}

/// 申请管理员身份的请求
#[derive(Debug, Deserialize)]
pub struct ClaimAdminRequest {
    pub admin_setup_token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NowPlaying {
    pub song: Option<SongSummary>,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub lyrics_line: Option<usize>,
    pub lyrics_text: Option<String>,
    pub started_at: Option<String>,
    pub stream_url: String,
    pub file_url: Option<String>,
    pub cover_url: Option<String>,
}

/// 搜索查询参数
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// 分页响应包装器
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// 通用 API 响应
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

/// 用于无数据体的响应
pub type SimpleResponse = ApiResponse<serde_json::Value>;

// ─── 设置 DTO ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SaveSettingsRequest {
    pub station_name: Option<String>,
    pub short_name: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub station_name: String,
    pub short_name: String,
    pub subtitle: String,
    pub description: String,
    pub icon_url: String,
    pub icon_path: String,
    pub resolved_icon_url: String,
}

// ─── 下载状态 ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub playlist: String,
    pub quality: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatus {
    pub running: bool,
    pub log: String,
}

/// SSE event payload for real-time download progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadEvent {
    pub log: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task_id: Option<String>,
}

// ─── 批量下载 ──────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct BatchDownloadItem {
    pub id: Option<String>,
    pub url: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub save_as: Option<String>,
    #[serde(default)]
    pub override_lyrics: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchDownloadRequest {
    pub source: String,
    pub items: Vec<BatchDownloadItem>,
    #[serde(default)]
    pub lyrics_save_mode: String, // none | separate (overwrite remains a legacy alias)
    pub quality: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchDownloadResultItem {
    pub id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub success: bool,
    pub error: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchDownloadResponse {
    pub task_id: String,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchDownloadStatus {
    pub task_id: String,
    pub running: bool,
    pub source: String,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub items: Vec<BatchDownloadResultItem>,
}

// ─── 网易云账号 ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserNcm {
    pub id: i64,
    pub device_user_id: i64,
    pub ncm_cookie: String,
    pub ncm_phone: String,
    pub ncm_password: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ─── 网易云导入任务 ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NcmImportTask {
    pub id: i64,
    pub song_id: i64,
    pub name: String,
    pub artists: String,
    pub status: String,
    pub batch_id: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct ImportPlaylistRequest {
    pub link: String,
}

#[derive(Debug, Serialize)]
pub struct ImportPlaylistResponse {
    pub total: usize,
    pub batch_id: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct StartNcmImportRequest {
    pub batch_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveNcmRequest {
    pub cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NcmStatus {
    pub configured: bool,
    pub method: String,
    pub phone_hint: String,
}
