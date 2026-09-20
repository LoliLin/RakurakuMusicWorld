/// 歌曲管理路由。
use crate::app::state::AppState;
use crate::error::AppError;
use crate::models::ApiResponse;
use crate::routes::admin::get_admin;
use crate::services::metadata::{find_cover, read_local_metadata};
use crate::world::{WorldCommand, WorldRuntime};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;

/// GET /api/admin/songs — 获取所有歌曲
pub async fn list_all_songs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::Song>>>, AppError> {
    let _admin = get_admin(&state, &headers).await?;

    let songs =
        sqlx::query_as::<_, crate::models::Song>("SELECT * FROM songs ORDER BY created_at DESC")
            .fetch_all(&state.db)
            .await?;

    Ok(Json(ApiResponse::ok(songs)))
}

/// POST /api/admin/enrich-song-metadata — 匿名从网易云补全专辑和封面
pub async fn enrich_song_metadata(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<crate::services::metadata_jobs::MetadataJob>>, AppError> {
    let admin = get_admin(&state, &headers).await?;
    let job = state
        .metadata_jobs
        .create(
            &state.db,
            crate::services::metadata_jobs::CreateMetadataJob {
                kind: "full".into(),
                scope: "library".into(),
                song_ids: Vec::new(),
                force: false,
            },
        )
        .await?;

    sqlx::query(
        "INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'enrich_song_metadata', ?)",
    )
    .bind(admin.id)
    .bind(format!("metadata job {} started", job.id))
    .execute(&state.db)
    .await?;

    Ok(Json(ApiResponse::ok(job)))
}

/// DELETE /api/admin/songs/{id} — 删除歌曲
pub async fn delete_song(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let admin = get_admin(&state, &headers).await?;

    let song = sqlx::query_as::<_, crate::models::Song>("SELECT * FROM songs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Song not found".into()))?;

    let file_path =
        std::path::Path::new(&state.config.audio_engine.media_path).join(&song.file_path);
    if !song.file_path.is_empty() && file_path.exists() {
        std::fs::remove_file(&file_path).ok();
    }

    if !song.lyrics_path.is_empty() {
        let lrc_path =
            std::path::Path::new(&state.config.audio_engine.media_path).join(&song.lyrics_path);
        if lrc_path.exists() {
            std::fs::remove_file(&lrc_path).ok();
        }
    }

    if !song.cover_path.is_empty() {
        let cover_path =
            std::path::Path::new(&state.config.audio_engine.media_path).join(&song.cover_path);
        if cover_path.exists() && cover_path != file_path {
            std::fs::remove_file(&cover_path).ok();
        }
    }

    {
        let _queue_guard = state.queue_sync.lock().await;

        sqlx::query("DELETE FROM playlist_songs WHERE song_id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;
        sqlx::query("DELETE FROM queue_items WHERE song_id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;
        state.player_handle.remove_request_by_song_id(id);
        sqlx::query("DELETE FROM favorites WHERE song_id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;
        sqlx::query("DELETE FROM songs WHERE id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'delete_song', ?)")
        .bind(admin.id)
        .bind(format!("Deleted song {}: {}", id, song.title))
        .execute(&state.db)
        .await?;

    WorldRuntime::new(state.clone()).dispatch(WorldCommand::ReloadQueue);

    Ok(Json(ApiResponse::ok(format!("已删除: {}", song.title))))
}

/// POST /api/admin/rescan-songs — 重新扫描媒体目录
#[allow(unreachable_code)]
pub async fn rescan_songs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<crate::services::metadata_jobs::MetadataJob>>, AppError> {
    let admin = get_admin(&state, &headers).await?;

    let media_path = std::path::Path::new(&state.config.audio_engine.media_path);
    if !media_path.exists() {
        return Err(AppError::BadRequest("Media directory not found".into()));
    }

    // Compatibility endpoint: all discovery and repair work is now handled by
    // the persistent worker so the HTTP request returns immediately.
    let job = state
        .metadata_jobs
        .create(
            &state.db,
            crate::services::metadata_jobs::CreateMetadataJob {
                kind: "full".into(),
                scope: "library".into(),
                song_ids: Vec::new(),
                force: true,
            },
        )
        .await?;
    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'rescan_songs', ?)")
        .bind(admin.id)
        .bind(format!("Metadata repair job {} started", job.id))
        .execute(&state.db)
        .await?;
    return Ok(Json(ApiResponse::ok(job)));

    let new_songs = 0;

    fn walk_dir(dir: &std::path::Path, exts: &[&str]) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(walk_dir(&path, exts));
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if exts.iter().any(|f| *f == ext_lower) {
                        files.push(path);
                    }
                }
            }
        }
        files
    }

    let audio_files = walk_dir(media_path, radio_engine::config::SUPPORTED_FORMATS);

    for file_path in &audio_files {
        let relative = file_path.strip_prefix(media_path).unwrap_or(file_path);
        let rel_str = relative.to_string_lossy().to_string();

        let existing = sqlx::query_as::<_, (i64,)>("SELECT id FROM songs WHERE file_path = ?")
            .bind(&rel_str)
            .fetch_optional(&state.db)
            .await?;

        if existing.is_none() {
            let metadata = read_local_metadata(file_path);

            let lrc_path = file_path.with_extension("lrc");
            let lyrics_path = if lrc_path.exists() {
                lrc_path
                    .strip_prefix(media_path)
                    .unwrap_or(&lrc_path)
                    .to_string_lossy()
                    .to_string()
            } else {
                String::new()
            };

            let cover_path = find_cover(&file_path, media_path);

            // 同一连接执行 INSERT + last_insert_rowid（连接级状态，避免并发
            // 池拿错 id）。
            let mut conn = state.db.acquire().await?;
            sqlx::query(
                "INSERT INTO songs (title, artist, album, file_path, lyrics_path, cover_path, duration_ms, filesize) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&metadata.title)
            .bind(&metadata.artist)
            .bind(&metadata.album)
            .bind(&rel_str)
            .bind(&lyrics_path)
            .bind(&cover_path)
            .bind(metadata.duration_ms)
            .bind(0_i64)
            .execute(&mut *conn)
            .await?;

            // 后台预热封面缓存（探测/提取），避免首个 cover 请求触发慢 ffmpeg。
            let song_id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
                .fetch_one(&mut *conn)
                .await?;
            let db = state.db.clone();
            let media_path_owned = media_path.to_path_buf();
            let file_path = rel_str.clone();
            let cover_path_owned = cover_path.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::services::metadata::ensure_cover_cached(
                    &db,
                    song_id,
                    &file_path,
                    &cover_path_owned,
                    &media_path_owned,
                )
                .await
                {
                    tracing::warn!("Cover pre-cache failed for song {}: {:?}", song_id, e);
                }
            });

            // The compatibility endpoint now delegates the actual work to
            // the persistent full repair job below. Keeping this legacy scan
            // path is harmless for clients that still expect its message.
        }
    }

    let job = state
        .metadata_jobs
        .create(
            &state.db,
            crate::services::metadata_jobs::CreateMetadataJob {
                kind: "full".into(),
                scope: "library".into(),
                song_ids: Vec::new(),
                force: true,
            },
        )
        .await?;

    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'rescan_songs', ?)")
        .bind(admin.id)
        .bind(format!(
            "Metadata repair job started; legacy scan saw {} existing entries",
            new_songs
        ))
        .execute(&state.db)
        .await?;

    if new_songs > 0 {
        WorldRuntime::new(state.clone()).dispatch(WorldCommand::ReloadQueue);
    }

    Ok(Json(ApiResponse::ok(job)))
}
