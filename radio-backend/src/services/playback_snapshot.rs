//! Converts engine playback state into enriched WebSocket playback messages.

use crate::app::state::AppState;
use crate::models::{LyricsLineDto, WsMessage};
use crate::world::WorldRuntime;
use std::sync::Arc;

struct CachedSong {
    db_song_id: i64,
    title: String,
    artist: String,
    lyrics_lines: Option<Vec<LyricsLineDto>>,
    metadata_revision: i64,
}

pub(crate) struct PlaybackSnapshotCache {
    last_file_path: String,
    cached: Option<CachedSong>,
    /// 记录已向客户端发送过全量歌词的歌曲 ID，避免重复克隆。
    lyrics_broadcast_song_id: Option<i64>,
    /// 最近一次含全量歌词的序列化消息 — 新 WS 连接补发用。
    last_full_message: Option<String>,
    observed_revision_signal: u64,
}

impl PlaybackSnapshotCache {
    pub(crate) fn new() -> Self {
        Self {
            last_file_path: String::new(),
            cached: None,
            lyrics_broadcast_song_id: None,
            last_full_message: None,
            observed_revision_signal: 0,
        }
    }

    /// 最近一条含全量歌词的 playback_state 消息（若有）。
    pub(crate) fn last_full_message(&self) -> Option<String> {
        self.last_full_message.clone()
    }

    /// 构建 500ms 播放帧。曲目切换时额外返回 TrackChanged 事件
    /// （Stage 2：只增事件，playback_state 契约不变）。
    pub(crate) async fn build_message(
        &mut self,
        state: &Arc<AppState>,
        ps: &radio_engine::types::PlaybackState,
    ) -> (WsMessage, Option<WsMessage>) {
        self.refresh_on_song_change(state, ps).await;

        // 优先用 DB songs 里的 title/artist；查不到时回退到引擎自带的
        // 元数据（PlaybackState.title / artist），这样文件夹里手动塞的、
        // 或还没入库的歌也能正常显示，不会一直"等待播放"。
        let (song_id, title, artist) = match self.cached.as_ref() {
            Some(c) => (c.db_song_id, c.title.clone(), c.artist.clone()),
            None => {
                let id = ps.song_id.unwrap_or(-1);
                (id, ps.title.clone(), ps.artist.clone())
            }
        };

        let lyrics_lines_ref = self.cached.as_ref().and_then(|c| c.lyrics_lines.as_ref());
        let lyrics_line = lyrics_lines_ref.and_then(|lines| {
            lines
                .iter()
                .enumerate()
                .rev()
                .find(|(_, l)| l.time_ms.is_some_and(|time_ms| time_ms <= ps.position_ms))
                .map(|(idx, _)| idx)
        });

        // 全量歌词只在歌曲切换后的首条消息中发送，后续只发行索引。
        let should_send_full_lyrics = self.lyrics_broadcast_song_id != Some(song_id);
        let lyrics_lines_payload = if should_send_full_lyrics {
            self.lyrics_broadcast_song_id = Some(song_id);
            // Always send a value on song change: Some(vec) with lyrics or
            // Some(vec![]) when there are none, so the frontend can distinguish
            // "no lyrics for this song" from "not resending lyrics this tick".
            Some(lyrics_lines_ref.cloned().unwrap_or_default())
        } else {
            None
        };
        let full = WsMessage::PlaybackState {
            song_id,
            title: title.clone(),
            artist: artist.clone(),
            position_ms: ps.position_ms,
            duration_ms: ps.duration_ms,
            lyrics_line,
            lyrics_lines: lyrics_lines_payload,
            status: ps.status.clone(),
            stream_url: state.config.audio_engine.resolve_stream_url(
                None,
                state.config.server.port,
                &state.config.server.base_path,
            ),
            file_url: if song_id > 0 {
                Some(
                    state
                        .config
                        .audio_engine
                        .resolve_file_url(song_id, &state.config.server.base_path),
                )
            } else {
                None
            },
            cover_url: if song_id > 0 {
                Some(format!(
                    "{}?v={}",
                    state
                        .config
                        .audio_engine
                        .resolve_cover_url(song_id, &state.config.server.base_path),
                    self.cached
                        .as_ref()
                        .map(|song| song.metadata_revision)
                        .unwrap_or(0)
                ))
            } else {
                None
            },
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };

        let track_changed = should_send_full_lyrics.then(|| {
            WsMessage::TrackChanged {
                song_id,
                title: title.clone(),
                artist: artist.clone(),
                duration_ms: ps.duration_ms,
                status: ps.status.clone(),
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
            }
        });

        if should_send_full_lyrics {
            self.last_full_message = Some(serde_json::to_string(&full).unwrap_or_default());
        }

        (full, track_changed)
    }

    async fn refresh_on_song_change(
        &mut self,
        state: &Arc<AppState>,
        ps: &radio_engine::types::PlaybackState,
    ) {
        // 切歌检测改用 file_path：playlist_index 对请求队列曲来说固定为 -1，
        // 连着两首请求曲不会换 index，但 file_path 一定不同。
        let revision_signal = state
            .metadata_revision_signal
            .load(std::sync::atomic::Ordering::SeqCst);
        let song_changed = ps.file_path != self.last_file_path && !ps.file_path.is_empty();
        let metadata_changed = revision_signal != self.observed_revision_signal;
        if !song_changed && !metadata_changed {
            return;
        }

        self.observed_revision_signal = revision_signal;

        self.last_file_path = ps.file_path.clone();
        self.cached = None;
        self.lyrics_broadcast_song_id = None;

        let song_row = sqlx::query_as::<_, (i64, String, String, String, String, i64)>(
            "SELECT id, title, artist, cover_path, lyrics_path, metadata_revision FROM songs WHERE file_path = ?",
        )
        .bind(&ps.file_path)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let Some((db_song_id, title, artist, _cover_path, lyrics_path, metadata_revision)) =
            song_row
        else {
            return;
        };

        if song_changed {
            if let Err(e) = WorldRuntime::new(state.clone())
                .mark_track_playing(db_song_id)
                .await
            {
                tracing::error!("mark_playing failed for song {}: {}", db_song_id, e);
            }
        }

        self.cached = Some(CachedSong {
            db_song_id,
            title,
            artist,
            lyrics_lines: load_lyrics_lines(state, &lyrics_path),
            metadata_revision,
        });
    }
}

fn load_lyrics_lines(state: &AppState, lyrics_path: &str) -> Option<Vec<LyricsLineDto>> {
    if lyrics_path.is_empty() {
        return None;
    }

    let lrc_full = std::path::Path::new(&state.config.audio_engine.media_path).join(lyrics_path);
    let content = decode_lrc_text(&lrc_full)?;
    let parsed = crate::lyrics::Lyrics::parse(&content);
    Some(
        parsed
            .lines
            .into_iter()
            .map(|l| LyricsLineDto {
                time_ms: l.time_ms,
                text: l.text,
            })
            .collect::<Vec<_>>(),
    )
}

/// 读取 .lrc 文本，支持常见编码：UTF-8 → GBK/GB18030 → UTF-16（按 BOM）。
/// 中文歌词文件常为 GBK 编码，`read_to_string`（仅 UTF-8）会静默失败，
/// 导致有歌词的歌曲显示"暂无歌词"。
fn decode_lrc_text(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;

    // UTF-16 BOM
    if bytes.starts_with(&[0xff, 0xfe]) {
        let (cow, _) = encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes[2..]);
        return Some(cow.into_owned());
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        let (cow, _) = encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes[2..]);
        return Some(cow.into_owned());
    }

    // UTF-8（含 BOM）
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Some(String::from_utf8_lossy(&bytes[3..]).into_owned());
    }
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Some(s.to_string());
    }

    // GBK/GB18030 回退（中文 .lrc 最常见）
    let (cow, _, _) = encoding_rs::GBK.decode(&bytes);
    Some(cow.into_owned())
}
