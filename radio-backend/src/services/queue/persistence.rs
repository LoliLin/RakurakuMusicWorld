//! Physical persistence adapter for playlist and queue storage.
//!
//! Directly interacts with SQLite database tables:
//! `queue_items`, `user_requests`, `songs`, `device_users`, `play_history`.

use crate::error::AppError;
use crate::models::{QueueItem, Song};
use sqlx::SqlitePool;

/// Row representing an item for queue display with joined song & device_user data.
pub type QueueDisplayRow = (
    i64,
    i64,
    i64,
    String,
    i32,
    chrono::NaiveDateTime,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    String,
);

/// Row representing a play history record with joined song & device_user data.
pub type PlayHistoryRow = (
    i64,
    i64,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    String,
);

/// 查询设备用户在冷却窗口内的上一次点歌已过时间（秒）。
pub async fn get_request_cooldown_elapsed(
    db: &SqlitePool,
    device_user_id: i64,
    cooldown_secs: u64,
) -> Result<Option<i64>, AppError> {
    let elapsed: Option<(i64,)> = sqlx::query_as(
        "SELECT strftime('%s', 'now') - strftime('%s', last_request_time) FROM user_requests WHERE device_user_id = ? AND last_request_time > datetime('now', '-' || ? || ' seconds')"
    )
    .bind(device_user_id)
    .bind(cooldown_secs as i64)
    .fetch_optional(db)
    .await?;

    Ok(elapsed.map(|(e,)| e))
}

/// 查询设备用户在特定时间戳后提交的点歌次数。
pub async fn count_user_submissions_since(
    db: &SqlitePool,
    device_user_id: i64,
    cutoff_str: &str,
) -> Result<usize, AppError> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM queue_items WHERE device_user_id = ? AND added_at > ?",
    )
    .bind(device_user_id)
    .bind(cutoff_str)
    .fetch_one(db)
    .await?;

    Ok(count.0 as usize)
}

/// 查询当前队列中处于 pending 或 playing 状态的曲目总数。
pub async fn count_active_queue_items(db: &SqlitePool) -> Result<usize, AppError> {
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM queue_items WHERE status IN ('pending', 'playing')")
            .fetch_one(db)
            .await?;

    Ok(count.0 as usize)
}

/// 根据 ID 查询歌曲信息。
pub async fn find_song_by_id(db: &SqlitePool, song_id: i64) -> Result<Option<Song>, AppError> {
    let song = sqlx::query_as::<_, Song>("SELECT * FROM songs WHERE id = ?")
        .bind(song_id)
        .fetch_optional(db)
        .await?;

    Ok(song)
}

/// 查询当前 active 队列中的最大位置编号。
pub async fn get_max_active_position(db: &SqlitePool) -> Result<Option<i32>, AppError> {
    let max_pos: Option<(i32,)> = sqlx::query_as(
        "SELECT MAX(position) FROM queue_items WHERE status IN ('pending', 'playing')",
    )
    .fetch_optional(db)
    .await?;

    Ok(max_pos.map(|(p,)| p))
}

/// 插入一条新的 pending 队列记录。
pub async fn insert_pending_queue_item(
    db: &SqlitePool,
    song_id: i64,
    device_user_id: i64,
    position: i32,
) -> Result<i64, AppError> {
    let result = sqlx::query(
        "INSERT INTO queue_items (song_id, device_user_id, status, position) VALUES (?, ?, 'pending', ?)"
    )
    .bind(song_id)
    .bind(device_user_id)
    .bind(position)
    .execute(db)
    .await?;

    Ok(result.last_insert_rowid())
}

/// 记录或刷新设备用户的最近点歌时间戳。
pub async fn record_user_request_timestamp(
    db: &SqlitePool,
    device_user_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT OR REPLACE INTO user_requests (device_user_id, last_request_time) VALUES (?, datetime('now'))"
    )
    .bind(device_user_id)
    .execute(db)
    .await?;

    Ok(())
}

/// 获取用于展示的队列详细行数据（按 position 升序）。
pub async fn fetch_queue_display_rows(
    db: &SqlitePool,
) -> Result<Vec<QueueDisplayRow>, AppError> {
    let rows = sqlx::query_as::<_, QueueDisplayRow>(
        "SELECT q.id, q.song_id, q.device_user_id, q.status, q.position, q.added_at,
                s.title, s.artist, s.album, s.duration_ms,
                s.lyrics_path, s.cover_path,
                COALESCE(d.display_name, 'unknown')
         FROM queue_items q
         LEFT JOIN songs s ON s.id = q.song_id
         LEFT JOIN device_users d ON d.id = q.device_user_id
         WHERE q.status IN ('pending', 'playing')
         ORDER BY q.position ASC",
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// 根据 ID 查询单个队列项。
pub async fn find_queue_item_by_id(
    db: &SqlitePool,
    item_id: i64,
) -> Result<Option<QueueItem>, AppError> {
    let item = sqlx::query_as::<_, QueueItem>("SELECT * FROM queue_items WHERE id = ?")
        .bind(item_id)
        .fetch_optional(db)
        .await?;

    Ok(item)
}

/// 查找当前队列中 active 项目的最大位置编号，若为空则返回 0。
pub async fn fetch_max_position_coalesced(db: &SqlitePool) -> Result<i32, AppError> {
    let max_pos: i32 = sqlx::query_as::<_, (i32,)>(
        "SELECT COALESCE(MAX(position), 0) FROM queue_items WHERE status IN ('pending', 'playing')",
    )
    .fetch_one(db)
    .await?
    .0;

    Ok(max_pos)
}

/// 在原子事务中移动队列项位置并重新排列受影响项。
pub async fn move_queue_item_positions(
    db: &SqlitePool,
    item_id: i64,
    old_position: i32,
    new_position: i32,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;

    if new_position < old_position {
        sqlx::query(
            "UPDATE queue_items SET position = position + 1 WHERE status IN ('pending', 'playing') AND position >= ? AND position < ?"
        )
        .bind(new_position)
        .bind(old_position)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "UPDATE queue_items SET position = position - 1 WHERE status IN ('pending', 'playing') AND position > ? AND position <= ?"
        )
        .bind(old_position)
        .bind(new_position)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE queue_items SET position = ? WHERE id = ?")
        .bind(new_position)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// 将队列项标记为 skipped 并将位于其后的项目位置向前移动 1 位。
pub async fn mark_item_skipped_and_shift(
    db: &SqlitePool,
    item_id: i64,
    removed_position: i32,
) -> Result<(), AppError> {
    sqlx::query("UPDATE queue_items SET status = 'skipped' WHERE id = ?")
        .bind(item_id)
        .execute(db)
        .await?;

    sqlx::query(
        "UPDATE queue_items SET position = position - 1 WHERE status IN ('pending', 'playing') AND position > ?"
    )
    .bind(removed_position)
    .execute(db)
    .await?;

    Ok(())
}

/// 查找当前正在播放（status = 'playing'）的队列项。
pub async fn find_current_playing_item(
    db: &SqlitePool,
) -> Result<Option<QueueItem>, AppError> {
    let playing = sqlx::query_as::<_, QueueItem>(
        "SELECT * FROM queue_items WHERE status = 'playing' ORDER BY position ASC LIMIT 1",
    )
    .fetch_optional(db)
    .await?;

    Ok(playing)
}

/// 将指定的队列项标记为 skipped 并更新 played_at 为当前时间。
pub async fn mark_item_skipped_as_played(
    db: &SqlitePool,
    item_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE queue_items SET status = 'skipped', played_at = datetime('now') WHERE id = ?",
    )
    .bind(item_id)
    .execute(db)
    .await?;

    Ok(())
}

/// 当音频引擎开始播放歌曲时，推进队列项状态并记入播放历史。
pub async fn advance_playing_and_record_history(
    db: &SqlitePool,
    song_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE queue_items SET status = 'played', played_at = datetime('now') WHERE status = 'playing'"
    )
    .execute(db)
    .await?;

    sqlx::query(
        "UPDATE queue_items SET status = 'playing' WHERE id = (SELECT id FROM queue_items WHERE song_id = ? AND status = 'pending' ORDER BY position ASC LIMIT 1)"
    )
    .bind(song_id)
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO play_history (song_id, device_user_id) SELECT song_id, device_user_id FROM queue_items WHERE song_id = ? AND status = 'playing' ORDER BY id DESC LIMIT 1"
    )
    .bind(song_id)
    .execute(db)
    .await?;

    Ok(())
}

/// 查询数据库中所有 pending 状态的曲目数据（按 position 升序）。
pub async fn fetch_pending_tracks_for_rehydration(
    db: &SqlitePool,
) -> Result<Vec<(String, i64, String, String, i64)>, AppError> {
    let rows = sqlx::query_as::<_, (String, i64, String, String, i64)>(
        "SELECT s.file_path, s.id, s.title, s.artist, s.duration_ms
         FROM queue_items q JOIN songs s ON s.id = q.song_id
         WHERE q.status = 'pending' ORDER BY q.position ASC",
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// 获取队列头部的下一个 pending 歌曲。
pub async fn fetch_next_pending_song(
    db: &SqlitePool,
) -> Result<Option<Song>, AppError> {
    let item = sqlx::query_as::<_, QueueItem>(
        "SELECT * FROM queue_items WHERE status = 'pending' ORDER BY position ASC LIMIT 1",
    )
    .fetch_optional(db)
    .await?;

    match item {
        Some(item) => {
            let song = sqlx::query_as::<_, Song>("SELECT * FROM songs WHERE id = ?")
                .bind(item.song_id)
                .fetch_optional(db)
                .await?;
            Ok(song)
        }
        None => Ok(None),
    }
}

/// 获取最近播放历史的行数据（按 played_at 降序）。
pub async fn fetch_play_history_rows(
    db: &SqlitePool,
    limit: i64,
) -> Result<Vec<PlayHistoryRow>, AppError> {
    let rows = sqlx::query_as::<_, PlayHistoryRow>(
        "SELECT h.id, h.song_id, h.device_user_id, h.played_at,
                s.title, s.artist, s.album, s.duration_ms,
                s.lyrics_path, s.cover_path,
                COALESCE(d.display_name, 'system')
         FROM play_history h
         LEFT JOIN songs s ON s.id = h.song_id
         LEFT JOIN device_users d ON d.id = h.device_user_id
         ORDER BY h.played_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// 在原子事务中删除歌曲记录及其关联的歌单条目、队列项和收藏。
pub async fn delete_song_and_relations(
    db: &SqlitePool,
    song_id: i64,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM playlist_songs WHERE song_id = ?")
        .bind(song_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM queue_items WHERE song_id = ?")
        .bind(song_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM favorites WHERE song_id = ?")
        .bind(song_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM songs WHERE id = ?")
        .bind(song_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
