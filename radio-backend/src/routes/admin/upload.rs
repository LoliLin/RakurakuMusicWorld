/// 上传路由（管理员）。
use crate::app::state::AppState;
use crate::error::AppError;
use crate::models::ApiResponse;
use crate::routes::admin::get_admin;
use crate::services::metadata::{find_cover, read_local_metadata, sanitize_filename};
use crate::world::{WorldCommand, WorldRuntime};
use axum::{
    extract::{Multipart, State},
    http::HeaderMap,
    Json,
};
use std::sync::Arc;

/// POST /api/admin/upload — 上传音乐文件到媒体目录
pub async fn upload_song(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let admin = get_admin(&state, &headers).await?;

    let media_path = std::path::PathBuf::from(&state.config.audio_engine.media_path);
    std::fs::create_dir_all(&media_path)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Create media dir error: {}", e)))?;

    let mut uploaded_filename = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("file").to_string();
        if name != "file" {
            continue;
        }

        let filename = field.file_name().unwrap_or("unknown.mp3").to_string();

        let safe_name = sanitize_filename(&filename);

        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("读取上传数据失败: {}", e)))?;

        if data.is_empty() {
            return Err(AppError::BadRequest("文件为空".into()));
        }

        let max_size = 100 * 1024 * 1024;
        if data.len() > max_size {
            return Err(AppError::BadRequest("文件大小超过 100MB 限制".into()));
        }

        let dest_path = media_path.join(&safe_name);
        std::fs::write(&dest_path, &data)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("写入文件失败: {}", e)))?;

        uploaded_filename = safe_name.clone();

        let metadata = read_local_metadata(&dest_path);
        let rel_str = safe_name.clone();
        let cover_path = find_cover(&dest_path, &media_path);
        let lrc_path = dest_path.with_extension("lrc");
        let lyrics_path = if lrc_path.exists() {
            lrc_path
                .strip_prefix(&media_path)
                .unwrap_or(&lrc_path)
                .to_string_lossy()
                .to_string()
        } else {
            String::new()
        };

        // 同一连接执行 INSERT + last_insert_rowid（连接级状态，避免并发池
        // 拿错 id）。
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
        .bind(data.len() as i64)
        .execute(&mut *conn)
        .await?;

        // 后台预热封面缓存（探测/提取），避免首个 cover 请求触发 30s ffmpeg。
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
    }

    if uploaded_filename.is_empty() {
        return Err(AppError::BadRequest("未找到上传文件字段".into()));
    }

    sqlx::query("INSERT INTO admin_log (admin_id, action, details) VALUES (?, 'upload_song', ?)")
        .bind(admin.id)
        .bind(format!("Uploaded {}", uploaded_filename))
        .execute(&state.db)
        .await?;

    // 让引擎重扫媒体目录，否则空文件夹起服务时上传后引擎 play_queue 仍然是空的。
    WorldRuntime::new(state.clone()).dispatch(WorldCommand::ReloadQueue);

    Ok(Json(ApiResponse::ok(format!(
        "上传成功: {}",
        uploaded_filename
    ))))
}
