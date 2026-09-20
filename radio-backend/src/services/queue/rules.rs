//! Logical playlist domain rules and service orchestration.
//!
//! Handles submission rate limits, capacity constraints, queue reordering rules,
//! and coordinates with physical persistence, the audio engine, and WebSocket broadcasting.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::app::state::AppState;
use crate::error::AppError;
use crate::models::{QueueItemDisplay, SongSummary};
use super::persistence;

/// 检查设备用户是否处于点歌冷却中。
pub async fn check_cooldown(
    db: &SqlitePool,
    device_user_id: i64,
    cooldown_secs: u64,
) -> Result<(), AppError> {
    if cooldown_secs == 0 {
        return Ok(());
    }

    let elapsed = persistence::get_request_cooldown_elapsed(db, device_user_id, cooldown_secs).await?;

    if let Some(elapsed) = elapsed {
        let remaining = cooldown_secs.saturating_sub(elapsed.max(0) as u64);
        return Err(AppError::RateLimited(format!(
            "Cooldown active: please wait {} seconds before requesting another song",
            remaining
        )));
    }

    Ok(())
}

/// 检查设备用户是否超出队列提交的速率限制。
pub async fn check_rate_limit(
    db: &SqlitePool,
    device_user_id: i64,
    window_secs: u64,
    max_subs: usize,
) -> Result<bool, AppError> {
    let cutoff = chrono::Utc::now() - chrono::Duration::seconds(window_secs as i64);
    let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

    let count = persistence::count_user_submissions_since(db, device_user_id, &cutoff_str).await?;

    Ok(count >= max_subs)
}

/// 获取当前队列大小（pending + playing 项目）。
pub async fn queue_size(db: &SqlitePool) -> Result<usize, AppError> {
    persistence::count_active_queue_items(db).await
}

/// 将歌曲添加到队列（尾部）。
pub async fn add_to_queue(
    state: &Arc<AppState>,
    song_id: i64,
    device_user_id: i64,
    display_name: &str,
) -> Result<i64, AppError> {
    let db = &state.db;
    let config = &state.config.queue;

    let song = persistence::find_song_by_id(db, song_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Song not found".into()))?;

    check_cooldown(db, device_user_id, config.request_cooldown_secs).await?;

    if check_rate_limit(
        db,
        device_user_id,
        config.rate_limit_window_secs,
        config.max_user_submissions,
    )
    .await?
    {
        return Err(AppError::RateLimited(format!(
            "You can only submit {} songs per {} seconds",
            config.max_user_submissions, config.rate_limit_window_secs
        )));
    }

    let current_size;
    let queue_item_id;
    {
        let _queue_guard = state.queue_sync.lock().await;

        current_size = queue_size(db).await?;
        if current_size >= config.max_size {
            return Err(AppError::BadRequest(format!(
                "Queue is full (max {} items)",
                config.max_size
            )));
        }

        let max_pos = persistence::get_max_active_position(db).await?;
        let next_position = max_pos.map(|p| p + 1).unwrap_or(0);

        queue_item_id =
            persistence::insert_pending_queue_item(db, song_id, device_user_id, next_position).await?;

        persistence::record_user_request_timestamp(db, device_user_id).await?;

        // Push the request onto the engine queue so the player picks it up next.
        state
            .player_handle
            .enqueue_request(radio_engine::types::RequestedTrack {
                file_path: song.file_path.clone(),
                song_id: song.id,
                title: song.title.clone(),
                artist: song.artist.clone(),
                duration_ms: song.duration_ms,
            });
    }

    crate::websocket::broadcast(
        state,
        crate::models::WsMessage::QueueUpdate {
            action: "added".into(),
            song_title: Some(song.title.clone()),
            requested_by: None,
            queue_size: current_size + 1,
        },
    );

    tracing::info!(
        "Device '{}' added song '{}' to queue (item #{})",
        display_name,
        song.title,
        queue_item_id
    );

    Ok(queue_item_id)
}

/// 获取带歌曲详情的队列，按 position 排序。
pub async fn get_queue_display(db: &SqlitePool) -> Result<Vec<QueueItemDisplay>, AppError> {
    let rows = persistence::fetch_queue_display_rows(db).await?;

    let display_items = rows
        .into_iter()
        .map(
            |(
                id,
                song_id,
                _device_user_id,
                status,
                position,
                added_at,
                title,
                artist,
                album,
                duration_ms,
                lyrics_path,
                cover_path,
                display_name,
            )| {
                QueueItemDisplay {
                    id,
                    song: title.map(|t| SongSummary {
                        // 真实歌曲 id（此前硬编码 0，导致前端无法按 id 加载封面）。
                        id: song_id,
                        title: t,
                        artist: artist.unwrap_or_default(),
                        album: album.unwrap_or_default(),
                        duration_ms: duration_ms.unwrap_or(0),
                        has_lyrics: !lyrics_path.unwrap_or_default().is_empty(),
                        has_cover: !cover_path.unwrap_or_default().is_empty(),
                        metadata_revision: 0,
                    }),
                    requested_by: display_name,
                    status,
                    position,
                    added_at,
                }
            },
        )
        .collect();

    Ok(display_items)
}

/// 将队列项移动到新位置（仅限管理员）。
///
/// Uses a transaction to keep position updates atomic. Validates that
/// `new_position` falls within the current pending/playing queue range.
pub async fn move_queue_item(
    state: &Arc<AppState>,
    item_id: i64,
    new_position: i32,
) -> Result<(), AppError> {
    let db = &state.db;
    let _queue_guard = state.queue_sync.lock().await;

    if new_position < 0 {
        return Err(AppError::BadRequest("Position must be non-negative".into()));
    }

    let item = persistence::find_queue_item_by_id(db, item_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Queue item not found".into()))?;

    if item.status != "pending" {
        return Err(AppError::BadRequest(
            "Only pending items can be moved".into(),
        ));
    }

    let old_position = item.position;

    if old_position == new_position {
        return Ok(());
    }

    let max_pos = persistence::fetch_max_position_coalesced(db).await?;

    if new_position > max_pos {
        return Err(AppError::BadRequest(format!(
            "Position {} exceeds max queue position {}",
            new_position, max_pos
        )));
    }

    persistence::move_queue_item_positions(db, item_id, old_position, new_position).await?;

    // Keep the embedded engine request queue in sync with the DB order.
    crate::world::WorldRuntime::new(state.clone())
        .rehydrate_playlist()
        .await?;

    Ok(())
}

/// 删除队列项（仅限管理员）。
pub async fn remove_queue_item(state: &Arc<AppState>, item_id: i64) -> Result<(), AppError> {
    let db = &state.db;
    let _queue_guard = state.queue_sync.lock().await;
    let item = persistence::find_queue_item_by_id(db, item_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Queue item not found".into()))?;

    if item.status == "playing" {
        return Err(AppError::BadRequest(
            "Cannot remove the currently playing item; use skip instead".into(),
        ));
    }

    let removed_position = item.position;
    let removed_song_id = item.song_id;

    persistence::mark_item_skipped_and_shift(db, item_id, removed_position).await?;

    // Pull it out of the engine request queue too, otherwise it'd still play.
    state
        .player_handle
        .remove_request_by_song_id(removed_song_id);

    Ok(())
}

/// 跳过当前正在播放的歌曲（仅限管理员）。
pub async fn skip_current(state: &Arc<AppState>) -> Result<(), AppError> {
    let db = &state.db;

    let playing = persistence::find_current_playing_item(db).await?;

    if let Some(item) = playing {
        persistence::mark_item_skipped_as_played(db, item.id).await?;
    }

    crate::world::WorldRuntime::new(state.clone()).dispatch(crate::world::WorldCommand::Skip);

    crate::websocket::broadcast(
        state,
        crate::models::WsMessage::Notice {
            message: "Admin skipped the current track".into(),
            level: "info".into(),
        },
    );

    Ok(())
}

/// 当音频引擎开始播放歌曲时调用。
pub async fn mark_playing(db: &SqlitePool, song_id: i64) -> Result<(), AppError> {
    persistence::advance_playing_and_record_history(db, song_id).await
}

/// 将数据库中所有 status='pending' 的队列项按 position 装回引擎请求队列。
///
/// 在服务启动时调用，让重启前用户已点的歌继续被播放。
pub async fn rehydrate_engine_queue(state: &Arc<AppState>) -> Result<(), AppError> {
    let rows = persistence::fetch_pending_tracks_for_rehydration(&state.db).await?;

    let tracks: Vec<radio_engine::types::RequestedTrack> = rows
        .into_iter()
        .map(|(file_path, song_id, title, artist, duration_ms)| {
            radio_engine::types::RequestedTrack {
                file_path,
                song_id,
                title,
                artist,
                duration_ms,
            }
        })
        .collect();

    let n = tracks.len();
    state.player_handle.replace_request_queue(tracks);
    if n > 0 {
        tracing::info!(
            "Rehydrated engine request queue with {} pending track(s)",
            n
        );
    }
    Ok(())
}

/// 获取队列头部（下一首要播放的歌曲）。
#[allow(dead_code)]
pub async fn get_next_song(db: &SqlitePool) -> Result<Option<crate::models::Song>, AppError> {
    persistence::fetch_next_pending_song(db).await
}

/// 获取最近的播放历史。
pub async fn get_history(db: &SqlitePool, limit: i64) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = persistence::fetch_play_history_rows(db, limit).await?;

    let result = rows
        .into_iter()
        .map(
            |(
                id,
                song_id,
                device_user_id,
                played_at,
                title,
                artist,
                album,
                duration_ms,
                lyrics_path,
                cover_path,
                display_name,
            )| {
                serde_json::json!({
                    "id": id,
                    // 顶层真实 song_id（此前响应只有内嵌 song.id=0，前端无法取用）。
                    "song_id": song_id,
                    "device_user_id": device_user_id,
                    "song": {
                        "id": song_id,
                        "title": title,
                        "artist": artist.unwrap_or_default(),
                        "album": album,
                        "duration_ms": duration_ms,
                        "has_lyrics": !lyrics_path.unwrap_or_default().is_empty(),
                        "has_cover": !cover_path.unwrap_or_default().is_empty(),
                    },
                    "requested_by": display_name,
                    "played_at": played_at,
                })
            },
        )
        .collect();

    Ok(result)
}

/// 彻底清除歌曲，并保持持久化队列与底层音频执行队列一致（仅限管理员）。
pub async fn purge_song(state: &Arc<AppState>, song_id: i64) -> Result<(), AppError> {
    let _queue_guard = state.queue_sync.lock().await;

    persistence::delete_song_and_relations(&state.db, song_id).await?;

    // 同步驱逐底层执行队列中的对应曲目
    state.player_handle.remove_request_by_song_id(song_id);

    Ok(())
}
