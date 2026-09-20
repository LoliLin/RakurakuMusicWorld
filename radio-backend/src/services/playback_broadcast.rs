//! Periodically receives engine playback progress and broadcasts WebSocket updates,
//! maintaining the authoritative Logical PlaybackState in AppState.

use crate::app::state::AppState;
use crate::services::playback_snapshot::PlaybackSnapshotCache;
use std::sync::Arc;

/// 启动播放状态聚合与广播轮询器。
/// 读取内嵌引擎的物理执行进度，结合 DB 元数据聚合为 Logical 权威状态，并派发 WebSocket 更新。
pub fn start_engine_state_poller(state: Arc<AppState>) {
    let state_clone = state.clone();

    tokio::spawn(async move {
        let mut snapshot_cache = PlaybackSnapshotCache::new();

        tracing::info!("Engine state poller started");

        loop {
            let progress = state_clone.player_handle.get_progress();
            let (enriched, track_changed, now_playing) =
                snapshot_cache.build_message(&state_clone, &progress).await;

            // 持续更新 Logical 权威播放快照
            {
                let mut guard = state_clone.current_playback.write().await;
                *guard = Some(now_playing);
            }

            // 仅在有活跃订阅者时才发送广播
            if state_clone.ws_tx.receiver_count() > 0 {
                // 同步最近的全量帧（含歌词），供新连接补发。
                if let Some(full) = snapshot_cache.last_full_message() {
                    let mut guard = state_clone
                        .ws_full_snapshot
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    *guard = Some(full);
                }

                // Stage 2：曲目切换事件先于 500ms 帧发出（前端据此刻做动效）。
                if let Some(event) = track_changed {
                    let _ = state_clone
                        .ws_tx
                        .send(serde_json::to_string(&event).unwrap_or_default());
                }

                let _ = state_clone
                    .ws_tx
                    .send(serde_json::to_string(&enriched).unwrap_or_default());
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });
}
