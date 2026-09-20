/// 批量下载路由 — 使用原生 Rust NCM 客户端 + SSE 实时推送。
use crate::app::state::AppState;
use crate::error::AppError;
use crate::models::{ApiResponse, DownloadEvent, DownloadRequest};
use crate::routes::admin::get_admin;
use crate::services::ncm::{run_download, DownloadRuntime, NcmClient};
use crate::world::{WorldCommand, WorldRuntime};
use axum::{
    extract::State,
    http::HeaderMap,
    response::sse::{Event, Sse},
    Json,
};
use futures_util::stream::{unfold, Stream};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::broadcast;

/// 全局下载运行标志
fn download_running() -> &'static Mutex<bool> {
    static RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(false))
}

/// 全局日志广播 channel（发送端）
fn log_broadcast() -> &'static broadcast::Sender<DownloadEvent> {
    static TX: OnceLock<broadcast::Sender<DownloadEvent>> = OnceLock::new();
    TX.get_or_init(|| broadcast::channel(512).0)
}

/// 内部 helper：从歌单文本启动下载任务（可被 NCM import 复用）。
pub fn spawn_download_job(
    state: Arc<AppState>,
    playlist: String,
    quality: Option<String>,
    format: Option<String>,
) -> Result<(), AppError> {
    {
        let running = download_running().lock().unwrap_or_else(|e| e.into_inner());
        if *running {
            return Err(AppError::BadRequest("已有下载任务在运行中".into()));
        }
    }

    let quality = quality.unwrap_or_else(|| "exhigh".into());
    let _format = format.unwrap_or_else(|| "mp3".into());
    let media_path = state.config.audio_engine.media_path.clone();
    let device_id = if state.config.ncm.device_id.is_empty() {
        None
    } else {
        Some(state.config.ncm.device_id.clone())
    };
    let ncm_cookie = crate::routes::admin::ncm::read_admin_ncm_cookie();
    let concurrency = state.config.ncm.download_concurrency.max(1);
    let client = NcmClient::new(device_id, ncm_cookie);
    let world = WorldRuntime::new(state.clone());

    {
        let mut running = download_running().lock().unwrap_or_else(|e| e.into_inner());
        *running = true;
    }

    // 获取广播发送端
    let broadcast_tx = log_broadcast().clone();

    tokio::spawn(async move {
        let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(256);

        // 转发内部日志到广播 channel
        let forwarder = tokio::spawn(async move {
            while let Some(line) = log_rx.recv().await {
                let ev = DownloadEvent {
                    log: line,
                    done: false,
                    task_id: None,
                };
                let _ = broadcast_tx.send(ev);
            }
        });

        let result = run_download(
            client,
            playlist,
            quality,
            _format,
            DownloadRuntime {
                db: state.db.clone(),
                output_dir: media_path.clone(),
                concurrency,
            },
            log_tx,
        )
        .await;

        // 等待日志转发完成
        forwarder.await.ok();

        match result {
            Ok((success, failed)) => {
                tracing::info!("Download complete: success={}, failed={}", success, failed);
            }
            Err(e) => {
                tracing::error!("Download error: {}", e);
                let _ = log_broadcast().send(DownloadEvent {
                    log: format!("❌ 下载任务异常: {}", e),
                    done: true,
                    task_id: None,
                });
            }
        }

        // 发送完成标记
        let _ = log_broadcast().send(DownloadEvent {
            log: "下载任务已结束".to_string(),
            done: true,
            task_id: None,
        });

        // 触发播放队列重载
        world.dispatch(WorldCommand::ReloadQueue);
        tracing::info!("Triggered play queue reload after download");

        let mut running = download_running().lock().unwrap_or_else(|e| e.into_inner());
        *running = false;
    });

    Ok(())
}

/// POST /api/admin/download — 启动批量下载
pub async fn start_download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<DownloadRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let _admin = get_admin(&state, &headers).await?;

    let playlist = body.playlist.trim().to_string();
    if playlist.is_empty() {
        return Err(AppError::BadRequest("歌单内容不能为空".into()));
    }

    spawn_download_job(state, playlist, body.quality, body.format)?;

    Ok(Json(ApiResponse::ok("下载任务已启动".into())))
}

/// GET /api/admin/download/stream — SSE 实时下载日志
pub async fn download_stream(
    State(_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    let _admin = get_admin(&_state, &headers).await?;

    let rx = log_broadcast().subscribe();

    let stream = unfold(rx, |mut rx| async {
        match rx.recv().await {
            Ok(ev) => {
                let data = serde_json::to_string(&ev).unwrap_or_default();
                Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(data)),
                    rx,
                ))
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                // 错过了一些消息，继续接收
                Some((
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
                ))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    Ok(Sse::new(stream))
}

/// GET /api/admin/download/status — 兼容旧版轮询（保留但降级）
pub async fn download_status(
    State(_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<crate::models::DownloadStatus>>, AppError> {
    let _admin = get_admin(&_state, &headers).await?;

    let running = {
        let running = download_running().lock().unwrap_or_else(|e| e.into_inner());
        *running
    };

    Ok(Json(ApiResponse::ok(crate::models::DownloadStatus {
        running,
        log: "请使用 /api/admin/download/stream 获取实时进度".to_string(),
    })))
}
