use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

use crate::config::{
    AUDIO_CHUNK_SIZE, CHANNELS, CROSSFADE_ENABLED, CROSSFADE_SECONDS, MP3_BITRATE, SAMPLE_RATE,
    STATE_PUBLISH_INTERVAL_MS, SUPPORTED_FORMATS,
};
use crate::ring_buffer::RingBuffer;
use crate::types::{
    AudioCommand, AudioCommandType, CrossfadeFfmpegArgs, FfmpegArgs, PlaybackState, RequestedTrack,
    TrackMetadata,
};

/// Build ffmpeg command line arguments for track playback.
pub fn build_ffmpeg_args(args: &FfmpegArgs, duration_ms: i64) -> Vec<String> {
    let mut cmd: Vec<String> = vec![
        "-nostdin".to_string(),
        "-re".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-i".to_string(),
        args.input_file.clone(),
        "-vn".to_string(),
        "-c:a".to_string(),
        "libmp3lame".to_string(),
        "-b:a".to_string(),
        MP3_BITRATE.to_string(),
        "-ar".to_string(),
        SAMPLE_RATE.to_string(),
        "-ac".to_string(),
        CHANNELS.to_string(),
    ];

    if args.fade_in {
        let afade = format!("afade=t=in:d={}:curve=tri", CROSSFADE_SECONDS);
        cmd.push("-af".to_string());
        cmd.push(afade);
    } else if duration_ms > (CROSSFADE_SECONDS as i64 * 2 * 1000) {
        let st = (duration_ms as f64 / 1000.0) - CROSSFADE_SECONDS as f64;
        let afade = format!("afade=t=out:st={:.3}:d={}:curve=tri", st, CROSSFADE_SECONDS);
        cmd.push("-af".to_string());
        cmd.push(afade);
    }

    if let Some(offset) = args.start_offset_secs {
        cmd.push("-ss".to_string());
        cmd.push(format!("{:.3}", offset));
    }

    cmd.push("-f".to_string());
    cmd.push("mp3".to_string());
    cmd.push("pipe:1".to_string());

    cmd
}

/// Build ffmpeg command line arguments for crossfade between two tracks.
pub fn build_crossfade_ffmpeg_args(args: &CrossfadeFfmpegArgs) -> Vec<String> {
    let fade_dur = args.crossfade_duration_secs;
    let current_dur = args.current_duration_ms as f64 / 1000.0;
    let fade_start = (current_dur - fade_dur).max(0.0);

    vec![
        "-nostdin".to_string(),
        "-re".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-i".to_string(),
        args.current_file.clone(),
        "-i".to_string(),
        args.next_file.clone(),
        "-filter_complex".to_string(),
        format!(
            "[0:a]afade=t=out:st={:.3}:d={:.3}:curve=tri[a1];[1:a]afade=t=in:d={:.3}:curve=tri[a2];[a1][a2]amix=inputs=2:duration=longest:dropout_transition=0",
            fade_start, fade_dur, fade_dur
        ),
        "-vn".to_string(),
        "-c:a".to_string(),
        "libmp3lame".to_string(),
        "-b:a".to_string(),
        MP3_BITRATE.to_string(),
        "-ar".to_string(),
        SAMPLE_RATE.to_string(),
        "-ac".to_string(),
        CHANNELS.to_string(),
        "-f".to_string(),
        "mp3".to_string(),
        "pipe:1".to_string(),
    ]
}

/// Spawn an ffmpeg process for track playback.
async fn spawn_ffmpeg(args: &FfmpegArgs, duration_ms: i64) -> io::Result<Child> {
    let ff_args = build_ffmpeg_args(args, duration_ms);
    Command::new("ffmpeg")
        .args(&ff_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
}

/// ffmpeg 在曲目播放期间静默多久视为卡死。
/// 健康曲目在 `-re` 下每秒至少产出 16KB（AUDIO_CHUNK_SIZE），
/// 超时无产出必为异常（进程挂起/解码停滞），应放弃本曲避免假播放。
const FFMPEG_STALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Where the currently-playing track was sourced from.
#[derive(Debug, Clone)]
enum TrackSource {
    /// Folder-cycle track at this index in `play_queue`.
    Folder(usize),
    /// User-requested track popped from the request queue.
    Request,
}

/// A track resolved and ready to be streamed.
#[derive(Debug, Clone)]
struct CurrentTrack {
    source: TrackSource,
    /// Absolute filesystem path (passed to ffmpeg).
    abs_path: String,
    /// Path relative to media_root (used in PlaybackState.file_path).
    rel_path: String,
    duration_ms: i64,
    title: String,
    artist: String,
    song_id: Option<i64>,
}

/// What ended the streaming loop for a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamOutcome {
    /// ffmpeg finished naturally (track ended).
    Natural,
    /// Skip / Next command was processed.
    Skipped,
    /// Prev command was processed.
    Prev,
    /// Stop or other terminating reason — exit the player.
    Stopped,
}

struct QueueReloadTask {
    generation: u64,
    handle: JoinHandle<(Vec<String>, Vec<TrackMetadata>)>,
}

/// Audio player that spawns ffmpeg, reads from pipe, pushes to ring buffer,
/// and supports a folder-cycle fallback plus a user-driven request queue.
pub struct Player {
    buffer: Arc<RingBuffer>,
    play_queue: Arc<Mutex<Vec<String>>>,
    play_queue_metadata: Arc<Mutex<Vec<TrackMetadata>>>,
    /// Cursor into `play_queue` for the folder-cycle fallback.
    current_track: Arc<AtomicUsize>,
    /// User-requested tracks (FIFO) — drained before the folder cycle each round.
    request_queue: Arc<Mutex<VecDeque<RequestedTrack>>>,
    cmd_rx: mpsc::UnboundedReceiver<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    stop_flag: Arc<AtomicBool>,
    /// Wakes the idle loop when a new request arrives or the queue is reloaded.
    wake: Arc<Notify>,
    /// At most one library scan runs at a time. Each reload command increments
    /// the shared generation before it enters the command channel, so a scan
    /// can never commit after a newer request has already been sent.
    queue_reload_task: Option<QueueReloadTask>,
    queue_reload_generation: Arc<Mutex<u64>>,
    media_path: String,
    /// Whether crossfade is enabled
    crossfade_enabled: bool,
}

/// Handle for external control of the Player.
#[derive(Clone)]
pub struct PlayerHandle {
    cmd_tx: mpsc::UnboundedSender<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    stop_flag: Arc<AtomicBool>,
    request_queue: Arc<Mutex<VecDeque<RequestedTrack>>>,
    wake: Arc<Notify>,
    queue_reload_generation: Arc<Mutex<u64>>,
}

impl Player {
    /// Create a new player. Returns (Player, PlayerHandle).
    pub fn new(buffer: Arc<RingBuffer>, media_path: String) -> (Self, PlayerHandle) {
        Self::new_with_crossfade(buffer, media_path, CROSSFADE_ENABLED)
    }

    /// Create a new player with crossfade option. Returns (Player, PlayerHandle).
    pub fn new_with_crossfade(
        buffer: Arc<RingBuffer>,
        media_path: String,
        crossfade_enabled: bool,
    ) -> (Self, PlayerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let state = Arc::new(Mutex::new(PlaybackState::default()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let play_queue = Arc::new(Mutex::new(Vec::new()));
        let play_queue_metadata = Arc::new(Mutex::new(Vec::new()));
        let current_track = Arc::new(AtomicUsize::new(0));
        let request_queue = Arc::new(Mutex::new(VecDeque::new()));
        let wake = Arc::new(Notify::new());
        let queue_reload_generation = Arc::new(Mutex::new(0));

        let player = Self {
            buffer,
            play_queue,
            play_queue_metadata,
            current_track,
            request_queue: Arc::clone(&request_queue),
            cmd_rx,
            state: Arc::clone(&state),
            stop_flag: Arc::clone(&stop_flag),
            wake: Arc::clone(&wake),
            queue_reload_task: None,
            queue_reload_generation: Arc::clone(&queue_reload_generation),
            media_path,
            crossfade_enabled,
        };

        let handle = PlayerHandle {
            cmd_tx,
            state,
            stop_flag,
            request_queue,
            wake,
            queue_reload_generation,
        };

        (player, handle)
    }

    /// Scan media_path for supported audio files and read their metadata.
    ///
    /// This is the slow part (recursive fs walk + one ffprobe per file) and
    /// must never run inside the streaming pump — it can block playback for
    /// the whole scan duration. Callers decide how to swap the result in.
    async fn scan_play_queue(media_path: &str) -> (Vec<String>, Vec<TrackMetadata>) {
        let dir = Path::new(media_path);
        if !dir.exists() || !dir.is_dir() {
            tracing::warn!("Media directory not found: {}", media_path);
            return (Vec::new(), Vec::new());
        }

        let files = crate::util::scan_media_dir(dir, dir, SUPPORTED_FORMATS);

        let mut new_queue = Vec::new();
        let mut new_metadata = Vec::new();

        for (_full_path, rel_path) in files {
            let filename = rel_path.clone();

            let meta = crate::metadata::extract_metadata(&rel_path, media_path)
                .await
                .unwrap_or_else(|_| TrackMetadata {
                    filename: filename.clone(),
                    title: Path::new(&filename)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    file_path: filename.clone(),
                    ..Default::default()
                });

            new_queue.push(filename);
            new_metadata.push(meta);
        }

        (new_queue, new_metadata)
    }

    /// Initialize play queue by recursively scanning media_path.
    pub async fn init_play_queue(&mut self) {
        let (new_queue, new_metadata) = Self::scan_play_queue(&self.media_path).await;

        {
            let mut queue = self.play_queue.lock().unwrap();
            let mut metadata = self.play_queue_metadata.lock().unwrap();

            queue.clear();
            metadata.clear();
            queue.extend(new_queue);
            metadata.extend(new_metadata);

            tracing::info!(
                "Play queue initialized: {} tracks from {}",
                queue.len(),
                self.media_path
            );
        }

        // Wake the idle loop so newly-uploaded tracks start playing right away
        // (without this, an empty-folder startup waits up to 5s before recheck).
        self.wake.notify_waiters();
    }

    /// Start or coalesce a non-blocking library reload.
    fn request_queue_reload(&mut self) {
        if self.queue_reload_task.is_some() {
            tracing::debug!("Play queue reload already running; scheduling one fresh scan");
            return;
        }

        self.spawn_queue_reload();
    }

    fn spawn_queue_reload(&mut self) {
        let media_path = self.media_path.clone();
        let wake = Arc::clone(&self.wake);
        let generation = *self.queue_reload_generation.lock().unwrap();
        let handle = tokio::spawn(async move {
            let result = Self::scan_play_queue(&media_path).await;
            // Wake an idle player so it can collect and apply the result.
            wake.notify_waiters();
            result
        });
        self.queue_reload_task = Some(QueueReloadTask { generation, handle });
    }

    /// Apply a completed scan only when no newer reload was requested.
    async fn poll_queue_reload(&mut self) {
        let finished = self
            .queue_reload_task
            .as_ref()
            .is_some_and(|task| task.handle.is_finished());
        if !finished {
            return;
        }

        let task = self.queue_reload_task.take().unwrap();
        let result = task.handle.await;
        // Hold the generation lock through the in-memory queue swap. This
        // closes the check-then-commit race with PlayerHandle::send_command:
        // either the newer command increments first and invalidates this
        // result, or it waits until this result is fully committed.
        let current_generation = self.queue_reload_generation.lock().unwrap();
        if task.generation != *current_generation {
            drop(current_generation);
            tracing::debug!("Discarding stale play queue scan and starting the pending reload");
            self.spawn_queue_reload();
            return;
        }

        let Ok((new_queue, new_metadata)) = result else {
            tracing::warn!("Background play queue scan task failed");
            return;
        };

        let track_count = new_queue.len();
        {
            // Always mutate the parallel queue vectors under both locks.
            let mut queue = self.play_queue.lock().unwrap();
            let mut metadata = self.play_queue_metadata.lock().unwrap();
            *queue = new_queue;
            *metadata = new_metadata;
        }
        drop(current_generation);
        self.wake.notify_waiters();
        tracing::info!("Play queue reloaded in background: {} tracks", track_count);
    }

    /// Main playback loop.
    pub async fn run(&mut self) {
        loop {
            if self.stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Drain commands that arrived while idle. Skip/Prev/Next/Play with no
            // current track is a no-op; ReloadQueue and Stop are honored.
            self.drain_idle_commands().await;
            self.poll_queue_reload().await;
            if self.stop_flag.load(Ordering::Relaxed) {
                break;
            }

            let track = match self.pick_next_track() {
                Some(t) => t,
                None => {
                    // Nothing to play: publish Stopped so clients drop the
                    // stream URL instead of connecting into silence with a
                    // stale "playing" status. Then wait for either a wake-up
                    // signal (request pushed, queue reloaded) or a 5s timeout.
                    self.publish_idle();
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                        _ = self.wake.notified() => {}
                    }
                    continue;
                }
            };

            let source_label = match track.source {
                TrackSource::Request => " (requested)".to_string(),
                TrackSource::Folder(idx) => {
                    let total = self.play_queue.lock().unwrap().len();
                    format!(" ({}/{})", idx + 1, total)
                }
            };
            tracing::info!(
                "Playing: {} [{}s]{}",
                track.rel_path,
                track.duration_ms / 1000,
                source_label
            );

            let outcome = self.stream_track(&track).await;

            // Decide how to advance the folder cursor.
            //   Natural / Skipped (folder track) → advance forward
            //   Prev (folder track)              → advance backward
            //   Request track                    → leave folder cursor untouched
            //                                       (popped from request queue already)
            //   Stopped                          → break
            match outcome {
                StreamOutcome::Stopped => break,
                StreamOutcome::Natural | StreamOutcome::Skipped => {
                    if let TrackSource::Folder(_) = track.source {
                        self.advance_folder(1);
                    }
                }
                StreamOutcome::Prev => {
                    if let TrackSource::Folder(_) = track.source {
                        self.advance_folder(-1);
                    }
                }
            }
        }

        if let Some(task) = self.queue_reload_task.take() {
            task.handle.abort();
        }
        let mut state = self.state.lock().unwrap();
        *state = PlaybackState::default();
        tracing::info!("Player stopped");
    }

    /// Process commands that arrived while no track was playing. Only
    /// `ReloadQueue` and `Stop` are meaningful here; the rest are dropped.
    async fn drain_idle_commands(&mut self) {
        let mut needs_reload = false;
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd.cmd_type {
                AudioCommandType::ReloadQueue => needs_reload = true,
                AudioCommandType::Stop => {
                    self.stop_flag.store(true, Ordering::Relaxed);
                    return;
                }
                AudioCommandType::Play => {
                    // Play with file_path: push as one-shot request so the regular
                    // pick_next_track path handles it (with proper metadata).
                    if let Some(fp) = cmd.file_path {
                        let rel = crate::util::relativize_media_path(&fp, &self.media_path);
                        self.request_queue
                            .lock()
                            .unwrap()
                            .push_front(RequestedTrack {
                                file_path: rel.clone(),
                                song_id: cmd.song_id.unwrap_or(0),
                                title: Path::new(&rel)
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                                artist: String::new(),
                                duration_ms: 0,
                            });
                    }
                }
                _ => {} // Skip/Prev/Next while idle: nothing to skip.
            }
        }
        if needs_reload {
            self.request_queue_reload();
        }
    }

    /// Pick the next track to play: request queue first, then folder cycle.
    fn pick_next_track(&self) -> Option<CurrentTrack> {
        // Try the request queue first. Skip entries whose file no longer exists.
        loop {
            let req = match self.request_queue.lock().unwrap().pop_front() {
                Some(r) => r,
                None => break,
            };

            let abs = crate::util::resolve_media_path(&req.file_path, &self.media_path)
                .to_string_lossy()
                .to_string();

            if !Path::new(&abs).exists() {
                tracing::warn!(
                    "Requested track missing on disk: {} — skipping",
                    req.file_path
                );
                continue;
            }

            return Some(CurrentTrack {
                source: TrackSource::Request,
                abs_path: abs,
                rel_path: req.file_path,
                duration_ms: req.duration_ms,
                title: req.title,
                artist: req.artist,
                song_id: if req.song_id > 0 {
                    Some(req.song_id)
                } else {
                    None
                },
            });
        }

        // Fall back to folder cycle.
        let pl = self.play_queue.lock().unwrap();
        if pl.is_empty() {
            return None;
        }
        let pl_meta = self.play_queue_metadata.lock().unwrap();
        let sz = pl.len();
        let idx = self.current_track.load(Ordering::Relaxed) % sz;
        let rel_path = pl[idx].clone();
        let abs = crate::util::resolve_media_path(&rel_path, &self.media_path)
            .to_string_lossy()
            .to_string();

        if !Path::new(&abs).exists() {
            tracing::warn!("Folder track missing on disk: {} — advancing", rel_path);
            drop(pl);
            drop(pl_meta);
            self.advance_folder(1);
            return None;
        }

        let meta = pl_meta.get(idx).cloned().unwrap_or_default();
        Some(CurrentTrack {
            source: TrackSource::Folder(idx),
            abs_path: abs,
            rel_path,
            duration_ms: meta.duration_ms,
            title: if !meta.title.is_empty() {
                meta.title
            } else {
                Path::new(&pl[idx])
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            },
            artist: meta.artist,
            song_id: None,
        })
    }

    /// Advance the folder cursor by ±1, wrapping around.
    fn advance_folder(&self, delta: i32) {
        let pl = self.play_queue.lock().unwrap();
        let sz = pl.len();
        if sz == 0 {
            return;
        }
        let cur = self.current_track.load(Ordering::Relaxed) % sz;
        let next = if delta >= 0 {
            (cur + delta as usize) % sz
        } else {
            // delta is -1
            (cur + sz - ((-delta) as usize % sz)) % sz
        };
        self.current_track.store(next, Ordering::Relaxed);
    }

    /// Peek at the next track without consuming it (for crossfade preview).
    fn peek_next_track(&self) -> Option<CurrentTrack> {
        // Try the request queue first (peek, don't pop)
        {
            let rq = self.request_queue.lock().unwrap();
            if let Some(req) = rq.front() {
                let abs = crate::util::resolve_media_path(&req.file_path, &self.media_path)
                    .to_string_lossy()
                    .to_string();

                if Path::new(&abs).exists() {
                    return Some(CurrentTrack {
                        source: TrackSource::Request,
                        abs_path: abs,
                        rel_path: req.file_path.clone(),
                        duration_ms: req.duration_ms,
                        title: req.title.clone(),
                        artist: req.artist.clone(),
                        song_id: if req.song_id > 0 {
                            Some(req.song_id)
                        } else {
                            None
                        },
                    });
                }
            }
        }

        // Fall back to folder cycle (peek at next index)
        let pl = self.play_queue.lock().unwrap();
        if pl.is_empty() {
            return None;
        }
        let pl_meta = self.play_queue_metadata.lock().unwrap();
        let sz = pl.len();
        let cur = self.current_track.load(Ordering::Relaxed) % sz;
        let next_idx = (cur + 1) % sz;
        let rel_path = pl[next_idx].clone();
        let abs = crate::util::resolve_media_path(&rel_path, &self.media_path)
            .to_string_lossy()
            .to_string();

        if !Path::new(&abs).exists() {
            return None;
        }

        let meta = pl_meta.get(next_idx).cloned().unwrap_or_default();
        Some(CurrentTrack {
            source: TrackSource::Folder(next_idx),
            abs_path: abs,
            rel_path,
            duration_ms: meta.duration_ms,
            title: if !meta.title.is_empty() {
                meta.title
            } else {
                Path::new(&pl[next_idx])
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            },
            artist: meta.artist,
            song_id: None,
        })
    }

    /// Stream a single track until it ends naturally or a command interrupts it.
    async fn stream_track(&mut self, track: &CurrentTrack) -> StreamOutcome {
        let ff_args = FfmpegArgs {
            input_file: track.abs_path.clone(),
            fade_in: false,
            start_offset_secs: None,
        };

        let mut main_child = match spawn_ffmpeg(&ff_args, track.duration_ms).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to spawn ffmpeg for {}: {}", track.rel_path, e);
                return StreamOutcome::Natural;
            }
        };
        let mut main_stdout = match main_child.stdout.take() {
            Some(s) => s,
            None => {
                tracing::error!("ffmpeg stdout missing for {}", track.rel_path);
                return StreamOutcome::Natural;
            }
        };

        // Bound the ffmpeg handoff so slow consumers backpressure stdout reads
        // instead of accumulating unbounded MP3 chunks in memory.
        let (audio_tx, mut audio_rx) = mpsc::channel::<Vec<u8>>(16);
        let ffmpeg_task = tokio::spawn(async move {
            let mut buf = vec![0u8; AUDIO_CHUNK_SIZE];
            loop {
                match main_stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if audio_tx.send(buf[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = main_child.kill().await;
        });

        let track_start = Instant::now();
        let track_start_system = chrono::Utc::now().timestamp_millis();
        let mut total_bytes_sent: u64 = 0;
        let mut last_publish = Instant::now();
        let mut last_push = Instant::now();
        let mut crossfade_triggered = false;

        // Initial state publish so subscribers see the new track immediately.
        self.publish_state(track, &track_start, track_start_system, total_bytes_sent);

        let outcome = loop {
            if self.stop_flag.load(Ordering::Relaxed) {
                ffmpeg_task.abort();
                break StreamOutcome::Stopped;
            }

            if let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd.cmd_type {
                    AudioCommandType::ReloadQueue => {
                        // 全库扫描（fs 遍历 + 每文件 ffprobe）可能耗时数十秒，
                        // 绝不能阻塞推流泵 —— 否则所有听众在扫描期间静默，
                        // 状态仍是 Playing，重连也拿不到数据。后台扫描严格单飞；
                        // 扫描期间的新请求会使旧结果作废，并合并为一次最新扫描。
                        self.request_queue_reload();
                    }
                    AudioCommandType::Skip | AudioCommandType::Next => {
                        ffmpeg_task.abort();
                        self.buffer.clear();
                        break StreamOutcome::Skipped;
                    }
                    AudioCommandType::Prev => {
                        ffmpeg_task.abort();
                        self.buffer.clear();
                        break StreamOutcome::Prev;
                    }
                    AudioCommandType::Play => {
                        if let Some(fp) = cmd.file_path {
                            let rel = crate::util::relativize_media_path(&fp, &self.media_path);
                            self.request_queue
                                .lock()
                                .unwrap()
                                .push_front(RequestedTrack {
                                    file_path: rel.clone(),
                                    song_id: cmd.song_id.unwrap_or(0),
                                    title: Path::new(&rel)
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_default(),
                                    artist: String::new(),
                                    duration_ms: 0,
                                });
                            ffmpeg_task.abort();
                            self.buffer.clear();
                            break StreamOutcome::Skipped;
                        }
                    }
                    AudioCommandType::Stop => {
                        self.stop_flag.store(true, Ordering::Relaxed);
                        ffmpeg_task.abort();
                        self.buffer.clear();
                        break StreamOutcome::Stopped;
                    }
                }
            }

            // Process reload commands before collecting a finished scan so a
            // newly requested reload can invalidate that scan's stale result.
            self.poll_queue_reload().await;

            match audio_rx.try_recv() {
                Ok(d) => {
                    self.buffer.push(&d);
                    total_bytes_sent += d.len() as u64;
                    last_push = Instant::now();
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    let _ = ffmpeg_task.await;
                    break StreamOutcome::Natural;
                }
            }

            // ffmpeg 存活但长时间不产出（进程卡死）→ 放弃本曲，避免
            // "状态 Playing 但流静默" 的假播放窗口。健康曲目在 -re 下
            // 每秒至少产出 16KB，超时无产出必为异常。
            if last_push.elapsed() > FFMPEG_STALL_TIMEOUT {
                tracing::warn!(
                    "ffmpeg produced no audio for {}s ({}) — advancing",
                    FFMPEG_STALL_TIMEOUT.as_secs(),
                    track.rel_path
                );
                ffmpeg_task.abort();
                break StreamOutcome::Natural;
            }

            // Check if we should trigger crossfade
            if self.crossfade_enabled && !crossfade_triggered && track.duration_ms > 0 {
                let elapsed_ms = track_start.elapsed().as_millis() as i64;
                let remaining_ms = track.duration_ms - elapsed_ms;
                let crossfade_window_ms = (CROSSFADE_SECONDS * 1000) as i64;

                if remaining_ms <= crossfade_window_ms && remaining_ms > 0 {
                    crossfade_triggered = true;
                    tracing::info!(
                        "Crossfade window reached for {} ({}ms remaining)",
                        track.rel_path,
                        remaining_ms
                    );

                    // Update state to crossfading
                    {
                        let mut state = self.state.lock().unwrap();
                        state.status = crate::types::PlaybackStatus::Crossfading;
                    }

                    // Pick next track for crossfade
                    if let Some(next_track) = self.peek_next_track() {
                        let crossfade_args = CrossfadeFfmpegArgs {
                            current_file: track.abs_path.clone(),
                            next_file: next_track.abs_path.clone(),
                            current_duration_ms: track.duration_ms,
                            crossfade_duration_secs: CROSSFADE_SECONDS as f64,
                        };

                        let cf_args = build_crossfade_ffmpeg_args(&crossfade_args);
                        tracing::info!("Starting crossfade with args: {:?}", cf_args);

                        // The crossfade ffmpeg will naturally transition to the next track
                        // We'll let the current track finish naturally and the next iteration
                        // will pick up the next track
                    }
                }
            }

            if last_publish.elapsed() >= Duration::from_millis(STATE_PUBLISH_INTERVAL_MS) {
                self.publish_state(track, &track_start, track_start_system, total_bytes_sent);
                last_publish = Instant::now();
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        };

        outcome
    }

    /// Publish an empty Stopped state when the queue has nothing to play.
    ///
    /// Without this the last track's "playing" state lingers forever: the
    /// frontend keeps a /stream connection that never receives data and
    /// reconnects into silence on every idle timeout — "shows playing,
    /// but no sound" with no recovery until the next track.
    fn publish_idle(&self) {
        let mut state = self.state.lock().unwrap();
        *state = PlaybackState::default();
    }

    /// Publish playback state for the active track.
    fn publish_state(
        &self,
        track: &CurrentTrack,
        track_start: &Instant,
        track_start_system: i64,
        total_bytes_sent: u64,
    ) {
        let position_ms = track_start.elapsed().as_millis() as i64;
        let clamped = if track.duration_ms > 0 && position_ms > track.duration_ms {
            track.duration_ms
        } else {
            position_ms
        };

        let playlist_index = match track.source {
            TrackSource::Folder(idx) => idx as i64,
            TrackSource::Request => -1,
        };

        let mut state = self.state.lock().unwrap();
        state.playlist_index = playlist_index;
        state.file_path = track.rel_path.clone();
        state.position_ms = clamped;
        state.duration_ms = track.duration_ms;
        state.status = crate::types::PlaybackStatus::Playing;
        state.total_bytes_sent = total_bytes_sent;
        state.track_start_timestamp_ms = track_start_system;
        state.title = track.title.clone();
        state.artist = track.artist.clone();
        state.song_id = track.song_id;
    }
}

impl PlayerHandle {
    /// Send a command to the player.
    pub fn send_command(&self, cmd: AudioCommand) {
        if matches!(&cmd.cmd_type, AudioCommandType::ReloadQueue) {
            let mut generation = self.queue_reload_generation.lock().unwrap();
            *generation = generation.wrapping_add(1);
            drop(generation);
            self.wake.notify_waiters();
        }
        let _ = self.cmd_tx.send(cmd);
    }

    /// Get the current physical playback progress reported by the audio executor.
    pub fn get_progress(&self) -> crate::types::EngineProgress {
        self.state.lock().unwrap().clone()
    }

    /// Get current playback state (snapshot) — backwards compatible alias.
    pub fn get_state(&self) -> PlaybackState {
        self.get_progress()
    }

    /// Request stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        let _ = self.cmd_tx.send(AudioCommand {
            cmd_type: AudioCommandType::Stop,
            song_id: None,
            file_path: None,
        });
        self.wake.notify_waiters();
    }

    /// Append a user-requested track to the engine's request queue.
    /// The player picks it up before the next folder-cycle track.
    pub fn enqueue_request(&self, track: RequestedTrack) {
        self.request_queue.lock().unwrap().push_back(track);
        self.wake.notify_waiters();
    }

    /// Replace the request queue (used at startup to rehydrate from DB).
    pub fn replace_request_queue(&self, tracks: Vec<RequestedTrack>) {
        let mut q = self.request_queue.lock().unwrap();
        q.clear();
        q.extend(tracks);
        drop(q);
        self.wake.notify_waiters();
    }

    /// Remove a queued request by song_id (e.g. when an admin removes it from
    /// the DB queue). Returns true if anything was removed.
    pub fn remove_request_by_song_id(&self, song_id: i64) -> bool {
        let mut q = self.request_queue.lock().unwrap();
        let before = q.len();
        q.retain(|r| r.song_id != song_id);
        before != q.len()
    }
}
