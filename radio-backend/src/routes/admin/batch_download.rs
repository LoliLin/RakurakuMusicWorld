use crate::app::state::AppState;
use crate::error::AppError;
use crate::models::{
    ApiResponse, BatchDownloadRequest, BatchDownloadResponse, BatchDownloadResultItem,
    BatchDownloadStatus, DownloadEvent,
};
use crate::routes::admin::get_admin;
use crate::services::download_tasks::{
    ext_from_type, finish_task, generate_task_id, insert_task, quality_to_ncm_level, remove_task,
    sanitize_filename, subscribe_task, task_snapshot, BatchTask,
};
use crate::services::ncm::{api, NcmClient};
use crate::services::netdisk;
use crate::world::{WorldCommand, WorldRuntime};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::sse::{Event, Sse},
    Json,
};
use futures_util::stream::{unfold, Stream};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

const MAX_BATCH_ITEMS: usize = 200;

struct NcmBatchContext {
    client: NcmClient,
    quality: String,
    lyrics_mode: String,
    media_path: String,
    db: sqlx::SqlitePool,
    import_batch_id: Option<String>,
}

fn ncm_item_label(item: &crate::models::BatchDownloadItem) -> String {
    if let Some(title) = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return match item
            .artist
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(artist) => format!("{} {}", artist, title),
            None => title.to_string(),
        };
    }

    item.id
        .as_deref()
        .or(item.url.as_deref())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn extract_ncm_song_id(item: &crate::models::BatchDownloadItem) -> Option<i64> {
    let song_link = regex::Regex::new(r"(?:song\?id=|/song/)(\d+)").ok()?;
    for value in [item.id.as_deref(), item.url.as_deref()]
        .into_iter()
        .flatten()
    {
        let value = value.trim();
        if let Ok(id) = value.parse() {
            return Some(id);
        }
        if let Some(captures) = song_link.captures(value) {
            return captures.get(1)?.as_str().parse().ok();
        }
    }
    None
}

fn launch_ncm_batch(
    state: Arc<AppState>,
    task: BatchTask,
    items: Vec<crate::models::BatchDownloadItem>,
    quality: String,
    lyrics_mode: String,
    import_batch_id: Option<String>,
) {
    let media_path = state.config.audio_engine.media_path.clone();
    let device_id =
        (!state.config.ncm.device_id.is_empty()).then(|| state.config.ncm.device_id.clone());
    let ncm_cookie = crate::routes::admin::ncm::read_admin_ncm_cookie();
    let client = NcmClient::new(device_id, ncm_cookie);
    let db = state.db.clone();
    let world = WorldRuntime::new(state);
    let context = NcmBatchContext {
        client,
        quality,
        lyrics_mode,
        media_path,
        db,
        import_batch_id,
    };

    tokio::spawn(async move {
        run_ncm_batch(task, items, context).await;
        world.dispatch(WorldCommand::ReloadQueue);
    });
}

/// Start an exact-ID NCM download batch from another admin workflow, such as
/// playlist import, while using the same task registry and metadata pipeline.
pub(crate) fn spawn_ncm_batch_job(
    state: Arc<AppState>,
    items: Vec<crate::models::BatchDownloadItem>,
    quality: String,
    lyrics_mode: String,
    import_batch_id: Option<String>,
) -> Result<BatchDownloadResponse, AppError> {
    if items.is_empty() {
        return Err(AppError::BadRequest("下载列表不能为空".into()));
    }
    let task_id = generate_task_id();
    let total = items.len();
    let task = BatchTask::new("ncm".into(), total);
    if !insert_task(task_id.clone(), task.clone()) {
        return Err(AppError::RateLimited(
            "同时运行的下载任务过多，请稍后重试".into(),
        ));
    }
    launch_ncm_batch(state, task, items, quality, lyrics_mode, import_batch_id);
    Ok(BatchDownloadResponse { task_id, total })
}

/// POST /api/admin/download/batch — 启动批量下载任务
pub async fn start_batch_download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<BatchDownloadRequest>,
) -> Result<Json<ApiResponse<BatchDownloadResponse>>, AppError> {
    let _admin = get_admin(&state, &headers).await?;

    if body.items.is_empty() {
        return Err(AppError::BadRequest("下载列表不能为空".into()));
    }
    if body.items.len() > MAX_BATCH_ITEMS {
        return Err(AppError::BadRequest(format!(
            "单个下载任务最多包含 {} 项",
            MAX_BATCH_ITEMS
        )));
    }
    if !matches!(
        body.lyrics_save_mode.as_str(),
        "" | "none" | "separate" | "overwrite"
    ) {
        return Err(AppError::BadRequest("不支持的歌词保存模式".into()));
    }

    let source = body.source.clone();
    let task_id = generate_task_id();
    let total = body.items.len();

    let task = BatchTask::new(source.clone(), total);

    if !insert_task(task_id.clone(), task.clone()) {
        return Err(AppError::RateLimited(
            "同时运行的下载任务过多，请稍后重试".into(),
        ));
    }

    let media_path = state.config.audio_engine.media_path.clone();
    let world = WorldRuntime::new(state.clone());

    match source.as_str() {
        "ncm" => {
            let quality = body.quality.unwrap_or_else(|| "exhigh".into());
            let lyrics_mode = match body.lyrics_save_mode.as_str() {
                "" | "overwrite" => "separate".to_string(),
                value => value.to_string(),
            };
            launch_ncm_batch(state, task, body.items, quality, lyrics_mode, None);
        }
        "netdisk" => {
            tokio::spawn(async move {
                run_netdisk_batch(task, body.items, media_path).await;
                world.dispatch(WorldCommand::ReloadQueue);
            });
        }
        "spotify" => {
            remove_task(&task_id);
            return Err(AppError::BadRequest("Spotify 下载尚未实现".into()));
        }
        _ => {
            remove_task(&task_id);
            return Err(AppError::BadRequest(format!("不支持的下载源: {}", source)));
        }
    }

    Ok(Json(ApiResponse::ok(BatchDownloadResponse {
        task_id,
        total,
    })))
}

#[derive(Debug, Deserialize)]
pub struct BatchStreamQuery {
    pub task_id: String,
}

/// GET /api/admin/download/batch/stream — SSE 批量下载进度（按 task_id）
pub async fn batch_download_stream(
    State(_state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<BatchStreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    let _admin = get_admin(&_state, &headers).await?;

    let rx = subscribe_task(&query.task_id)
        .ok_or_else(|| AppError::BadRequest("任务不存在或已结束".into()))?;

    let stream = unfold(rx, |mut rx| async {
        match rx.recv().await {
            Ok(ev) => {
                let data = serde_json::to_string(&ev).unwrap_or_default();
                Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(data)),
                    rx,
                ))
            }
            Err(broadcast::error::RecvError::Lagged(_)) => Some((
                Ok::<_, std::convert::Infallible>(
                    Event::default().data(
                        serde_json::to_string(&DownloadEvent {
                            log: "...".to_string(),
                            done: false,
                            task_id: None,
                        })
                        .unwrap_or_default(),
                    ),
                ),
                rx,
            )),
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    Ok(Sse::new(stream))
}

/// GET /api/admin/download/batch/status — 获取批量下载状态
pub async fn batch_download_status(
    State(_state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<BatchStreamQuery>,
) -> Result<Json<ApiResponse<BatchDownloadStatus>>, AppError> {
    let _admin = get_admin(&_state, &headers).await?;

    let snapshot = task_snapshot(&query.task_id)
        .ok_or_else(|| AppError::BadRequest("任务不存在或已结束".into()))?;

    Ok(Json(ApiResponse::ok(BatchDownloadStatus {
        task_id: query.task_id,
        running: snapshot.running,
        source: snapshot.source,
        total: snapshot.total,
        success: snapshot.success,
        failed: snapshot.failed,
        items: snapshot.items,
    })))
}

// ─── NCM 批量下载 ───────────────────────────────────────────────

async fn run_ncm_batch(
    task: BatchTask,
    items: Vec<crate::models::BatchDownloadItem>,
    context: NcmBatchContext,
) {
    let total = items.len();

    let _ = task.tx.send(DownloadEvent {
        log: format!("🎵 NCM 批量下载开始，共 {} 首", total),
        done: false,
        task_id: None,
    });

    for (i, item) in items.iter().enumerate() {
        let keyword = ncm_item_label(item);

        if keyword.is_empty() {
            let result = BatchDownloadResultItem {
                id: item.id.clone(),
                title: item.title.clone(),
                artist: item.artist.clone(),
                success: false,
                error: Some("缺少关键词".into()),
                file_path: None,
            };
            task.items
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(result.clone());
            task.failed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            update_import_task_status(
                &context.db,
                context.import_batch_id.as_deref(),
                item,
                "failed",
            )
            .await;
            let _ = task.tx.send(DownloadEvent {
                log: format!("❌ [{}/{}] 缺少关键词", i + 1, total),
                done: false,
                task_id: None,
            });
            continue;
        }

        let _ = task.tx.send(DownloadEvent {
            log: format!("🔍 [{}/{}] 搜索: {}", i + 1, total, keyword),
            done: false,
            task_id: None,
        });

        match ncm_download_one(&context, item, &task, i, total).await {
            Ok(path) => {
                let result = BatchDownloadResultItem {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    artist: item.artist.clone(),
                    success: true,
                    error: None,
                    file_path: Some(path),
                };
                task.items
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(result);
                task.success
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                update_import_task_status(
                    &context.db,
                    context.import_batch_id.as_deref(),
                    item,
                    "done",
                )
                .await;
            }
            Err(e) => {
                let result = BatchDownloadResultItem {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    artist: item.artist.clone(),
                    success: false,
                    error: Some(e.to_string()),
                    file_path: None,
                };
                task.items
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(result);
                task.failed
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                update_import_task_status(
                    &context.db,
                    context.import_batch_id.as_deref(),
                    item,
                    "failed",
                )
                .await;
                let _ = task.tx.send(DownloadEvent {
                    log: format!("❌ [{}/{}] 失败: {}", i + 1, total, e),
                    done: false,
                    task_id: None,
                });
            }
        }
    }

    let success = task.success.load(std::sync::atomic::Ordering::SeqCst);
    let failed = task.failed.load(std::sync::atomic::Ordering::SeqCst);
    let _ = task.tx.send(DownloadEvent {
        log: format!("🎉 NCM 批量下载完成! 成功: {}, 失败: {}", success, failed),
        done: true,
        task_id: None,
    });

    finish_task(&task);
}

async fn update_import_task_status(
    db: &sqlx::SqlitePool,
    batch_id: Option<&str>,
    item: &crate::models::BatchDownloadItem,
    status: &str,
) {
    let (Some(batch_id), Some(song_id)) = (batch_id, extract_ncm_song_id(item)) else {
        return;
    };
    if let Err(error) = sqlx::query(
        "UPDATE ncm_import_tasks SET status = ?, updated_at = datetime('now') WHERE batch_id = ? AND song_id = ?",
    )
    .bind(status)
    .bind(batch_id)
    .bind(song_id)
    .execute(db)
    .await
    {
        tracing::warn!(batch_id, song_id, status, ?error, "更新网易云导入任务状态失败");
    }
}

async fn ncm_download_one(
    context: &NcmBatchContext,
    item: &crate::models::BatchDownloadItem,
    task: &BatchTask,
    idx: usize,
    total: usize,
) -> anyhow::Result<String> {
    let song_id = if let Some(song_id) = extract_ncm_song_id(item) {
        song_id
    } else {
        if item
            .url
            .as_deref()
            .is_some_and(|url| !url.trim().is_empty())
        {
            anyhow::bail!("无法解析网易云单曲链接");
        }

        let keyword = ncm_item_label(item);
        let results = api::search_song(&context.client, &keyword, 5).await?;
        let song = results
            .first()
            .ok_or_else(|| anyhow::anyhow!("未找到歌曲"))?;
        song.id
    };
    let detail = api::get_song_detail(&context.client, &[song_id])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("未找到歌曲详情"))?;
    let song_name = detail.name.clone();
    let artist_name = detail
        .ar
        .iter()
        .map(|artist| artist.name.trim())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let _ = task.tx.send(DownloadEvent {
        log: format!(
            "✅ [{}/{}] 找到: {} - {} (ID: {})",
            idx + 1,
            total,
            artist_name,
            song_name,
            song_id
        ),
        done: false,
        task_id: None,
    });

    // 2. Get download URL
    let level = quality_to_ncm_level(&context.quality);
    let urls = api::get_song_url(&context.client, &[song_id], level).await?;
    let url_data = urls
        .first()
        .ok_or_else(|| anyhow::anyhow!("网易云未返回下载信息"))?;
    let download_url = url_data
        .url
        .as_deref()
        .filter(|url| !url.is_empty())
        .ok_or_else(|| anyhow::anyhow!("无法获取下载链接 (code={})", url_data.code))?;
    let ext = ext_from_type(
        url_data.file_type.as_deref().unwrap_or_default(),
        download_url,
    );

    // 3. Download file
    let safe_artist = sanitize_filename(&artist_name);
    let safe_title = sanitize_filename(&song_name);
    let filename = if let Some(ref save_as) = item.save_as {
        if save_as.contains('.') {
            sanitize_filename(save_as)
        } else {
            format!("{}.{}", sanitize_filename(save_as), ext)
        }
    } else {
        format!("{} - {}.{}", safe_artist, safe_title, ext)
    };

    let media_root = PathBuf::from(&context.media_path);
    let output_dir = media_root.join("downloads");
    tokio::fs::create_dir_all(&output_dir).await.ok();
    let filepath = output_dir.join(&filename);

    let _ = task.tx.send(DownloadEvent {
        log: format!(
            "⬇️ [{}/{}] 下载: {} ({} bytes)",
            idx + 1,
            total,
            filename,
            url_data.size
        ),
        done: false,
        task_id: None,
    });

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let resp = http.get(download_url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("音频文件下载失败: HTTP {}", status);
    }
    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        anyhow::bail!("音频文件下载失败: 返回空文件");
    }

    // MD5 check
    if let Some(expected_md5) = url_data.md5.as_deref().filter(|md5| !md5.is_empty()) {
        use md5::{Digest, Md5};
        let mut hasher = Md5::new();
        hasher.update(&bytes);
        let file_md5 = format!("{:x}", hasher.finalize());
        if file_md5 != expected_md5 {
            let _ = task.tx.send(DownloadEvent {
                log: format!(
                    "⚠️ [{}/{}] MD5 校验失败 (期望 {}, 实际 {})",
                    idx + 1,
                    total,
                    expected_md5,
                    file_md5
                ),
                done: false,
                task_id: None,
            });
        } else {
            let _ = task.tx.send(DownloadEvent {
                log: format!("✅ [{}/{}] MD5 校验通过", idx + 1, total),
                done: false,
                task_id: None,
            });
        }
    }

    tokio::fs::write(&filepath, &bytes).await?;
    let _ = task.tx.send(DownloadEvent {
        log: format!("✅ [{}/{}] 已保存: {}", idx + 1, total, filename),
        done: false,
        task_id: None,
    });

    // 4. Download lyrics (unless override_lyrics is true)
    let mut saved_lyrics_path = None;
    if !item.override_lyrics && context.lyrics_mode != "none" {
        match api::get_song_lyric(&context.client, song_id).await {
            Ok(Some(lyric)) if !lyric.is_empty() => {
                let lrc_path = filepath.with_extension("lrc");
                if let Err(e) = tokio::fs::write(&lrc_path, lyric).await {
                    let _ = task.tx.send(DownloadEvent {
                        log: format!("⚠️ 歌词保存失败: {}", e),
                        done: false,
                        task_id: None,
                    });
                } else {
                    saved_lyrics_path = Some(lrc_path.clone());
                    let _ = task.tx.send(DownloadEvent {
                        log: format!("📝 歌词已保存: {}", lrc_path.display()),
                        done: false,
                        task_id: None,
                    });
                }
            }
            Ok(None) | Ok(Some(_)) => {}
            Err(e) => {
                let _ = task.tx.send(DownloadEvent {
                    log: format!("⚠️ 歌词获取失败: {}", e),
                    done: false,
                    task_id: None,
                });
            }
        }
    }

    let library_song_id = crate::services::ncm::metadata::sync_downloaded_song(
        &context.db,
        &media_root,
        &filepath,
        saved_lyrics_path.as_deref(),
        &detail,
        bytes.len() as i64,
    )
    .await?;
    let _ = task.tx.send(DownloadEvent {
        log: format!(
            "📚 [{}/{}] 已同步到曲库 (歌曲 ID: {})",
            idx + 1,
            total,
            library_song_id
        ),
        done: false,
        task_id: None,
    });

    Ok(filepath.to_string_lossy().to_string())
}

// ─── 网盘批量下载 ───────────────────────────────────────────────

async fn run_netdisk_batch(
    task: BatchTask,
    items: Vec<crate::models::BatchDownloadItem>,
    media_path: String,
) {
    let total = items.len();

    let _ = task.tx.send(DownloadEvent {
        log: format!("📦 网盘批量下载开始，共 {} 个链接", total),
        done: false,
        task_id: None,
    });

    for (i, item) in items.iter().enumerate() {
        let url = item.url.clone().unwrap_or_default();
        if url.is_empty() {
            let result = BatchDownloadResultItem {
                id: item.id.clone(),
                title: item.title.clone(),
                artist: item.artist.clone(),
                success: false,
                error: Some("缺少分享链接".into()),
                file_path: None,
            };
            task.items
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(result);
            task.failed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            continue;
        }

        let _ = task.tx.send(DownloadEvent {
            log: format!("🔍 [{}/{}] 解析分享链接: {}", i + 1, total, url),
            done: false,
            task_id: None,
        });

        match netdisk_download_one(&url, &media_path, &task, i, total).await {
            Ok(paths) => {
                let file_names: Vec<String> = paths
                    .iter()
                    .map(|p| {
                        std::path::Path::new(p)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| p.clone())
                    })
                    .collect();
                let result = BatchDownloadResultItem {
                    id: item.id.clone(),
                    title: Some(file_names.join(", ")),
                    artist: item.artist.clone(),
                    success: true,
                    error: None,
                    file_path: Some(paths.join("\n")),
                };
                task.items
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(result);
                task.success
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            Err(e) => {
                let result = BatchDownloadResultItem {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    artist: item.artist.clone(),
                    success: false,
                    error: Some(e.to_string()),
                    file_path: None,
                };
                task.items
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(result);
                task.failed
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = task.tx.send(DownloadEvent {
                    log: format!("❌ [{}/{}] 失败: {}", i + 1, total, e),
                    done: false,
                    task_id: None,
                });
            }
        }
    }

    let success = task.success.load(std::sync::atomic::Ordering::SeqCst);
    let failed = task.failed.load(std::sync::atomic::Ordering::SeqCst);
    let _ = task.tx.send(DownloadEvent {
        log: format!("🎉 网盘批量下载完成! 成功: {}, 失败: {}", success, failed),
        done: true,
        task_id: None,
    });

    finish_task(&task);
}

async fn netdisk_download_one(
    url: &str,
    media_path: &str,
    task: &BatchTask,
    idx: usize,
    total: usize,
) -> anyhow::Result<Vec<String>> {
    let info = netdisk::get_share_info(url).await?;

    let _ = task.tx.send(DownloadEvent {
        log: format!("📂 [{}/{}] 获取文件列表...", idx + 1, total),
        done: false,
        task_id: None,
    });

    let files = netdisk::list_share_files(&info).await?;
    if files.is_empty() {
        anyhow::bail!("分享中没有文件");
    }

    // Filter to audio files or all files if no audio found
    let audio_exts = [
        ".mp3", ".flac", ".m4a", ".wav", ".aac", ".ogg", ".opus", ".wma",
    ];
    let audio_files: Vec<_> = files
        .iter()
        .filter(|f| {
            let name_lower = f.filename.to_lowercase();
            audio_exts.iter().any(|ext| name_lower.ends_with(ext))
        })
        .collect();

    let files_to_download = if audio_files.is_empty() {
        files.iter().filter(|f| !f.is_dir).collect::<Vec<_>>()
    } else {
        audio_files
    };

    if files_to_download.is_empty() {
        anyhow::bail!("没有可下载的文件");
    }

    let _ = task.tx.send(DownloadEvent {
        log: format!(
            "📋 [{}/{}] 发现 {} 个文件",
            idx + 1,
            total,
            files_to_download.len()
        ),
        done: false,
        task_id: None,
    });

    let output_dir = PathBuf::from(media_path).join("downloads");
    tokio::fs::create_dir_all(&output_dir).await.ok();
    let mut downloaded_paths = Vec::new();

    for (fi, file) in files_to_download.iter().enumerate() {
        let _ = task.tx.send(DownloadEvent {
            log: format!(
                "⬇️ [{}/{}][{}/{}] 获取链接: {}",
                idx + 1,
                total,
                fi + 1,
                files_to_download.len(),
                file.filename
            ),
            done: false,
            task_id: None,
        });

        let link = match netdisk::get_download_link(&info, file.fs_id).await {
            Ok(l) => l,
            Err(e) => {
                let _ = task.tx.send(DownloadEvent {
                    log: format!("⚠️ 获取链接失败 {}: {}", file.filename, e),
                    done: false,
                    task_id: None,
                });
                continue;
            }
        };

        let safe_name = sanitize_filename(&file.filename);
        let output_path = output_dir.join(&safe_name);

        let _ = task.tx.send(DownloadEvent {
            log: format!(
                "⬇️ [{}/{}][{}/{}] 下载: {} ({} bytes)",
                idx + 1,
                total,
                fi + 1,
                files_to_download.len(),
                safe_name,
                file.size
            ),
            done: false,
            task_id: None,
        });

        match netdisk::download_file(&link, &output_path).await {
            Ok(size) => {
                let path_str = output_path.to_string_lossy().to_string();
                downloaded_paths.push(path_str.clone());
                let _ = task.tx.send(DownloadEvent {
                    log: format!(
                        "✅ [{}/{}][{}/{}] 已保存: {} ({} bytes)",
                        idx + 1,
                        total,
                        fi + 1,
                        files_to_download.len(),
                        safe_name,
                        size
                    ),
                    done: false,
                    task_id: None,
                });
            }
            Err(e) => {
                let _ = task.tx.send(DownloadEvent {
                    log: format!(
                        "❌ [{}/{}][{}/{}] 下载失败 {}: {}",
                        idx + 1,
                        total,
                        fi + 1,
                        files_to_download.len(),
                        safe_name,
                        e
                    ),
                    done: false,
                    task_id: None,
                });
            }
        }
    }

    if downloaded_paths.is_empty() {
        anyhow::bail!("没有文件下载成功");
    }

    Ok(downloaded_paths)
}

#[cfg(test)]
mod tests {
    use super::{extract_ncm_song_id, update_import_task_status};
    use crate::models::BatchDownloadItem;

    fn item(id: Option<&str>, url: Option<&str>) -> BatchDownloadItem {
        BatchDownloadItem {
            id: id.map(str::to_string),
            url: url.map(str::to_string),
            artist: None,
            title: None,
            save_as: None,
            override_lyrics: false,
        }
    }

    #[test]
    fn extracts_song_ids_from_batch_items() {
        assert_eq!(extract_ncm_song_id(&item(Some("123"), None)), Some(123));
        assert_eq!(
            extract_ncm_song_id(&item(None, Some("https://music.163.com/song?id=456"))),
            Some(456)
        );
        assert_eq!(
            extract_ncm_song_id(&item(None, Some("https://music.163.com/playlist?id=789"))),
            None
        );
    }

    #[tokio::test]
    async fn import_status_update_is_scoped_to_the_requested_batch() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE ncm_import_tasks (song_id INTEGER NOT NULL, status TEXT NOT NULL, batch_id TEXT NOT NULL, updated_at DATETIME)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ncm_import_tasks (song_id, status, batch_id) VALUES (123, 'queued', 'batch-a'), (123, 'queued', 'batch-b')",
        )
        .execute(&db)
        .await
        .unwrap();

        update_import_task_status(&db, Some("batch-a"), &item(Some("123"), None), "done").await;

        let statuses: Vec<(String, String)> =
            sqlx::query_as("SELECT batch_id, status FROM ncm_import_tasks ORDER BY batch_id")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(
            statuses,
            vec![
                ("batch-a".into(), "done".into()),
                ("batch-b".into(), "queued".into())
            ]
        );
    }
}
