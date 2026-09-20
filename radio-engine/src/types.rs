use serde::{Deserialize, Serialize};

/// Track metadata for a single audio file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub filename: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: String,
    pub track_number: String,
    /// Duration in milliseconds (aligned with backend Song model).
    pub duration_ms: i64,
    /// Embedded cover art binary data (if present in file tags).
    pub cover_data: Vec<u8>,
    /// Embedded lyrics text (if present in file tags).
    pub embedded_lyrics: String,
    /// File path relative to media_root.
    pub file_path: String,
}

impl Default for TrackMetadata {
    fn default() -> Self {
        Self {
            filename: String::new(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            year: String::new(),
            track_number: String::new(),
            duration_ms: 0,
            cover_data: Vec::new(),
            embedded_lyrics: String::new(),
            file_path: String::new(),
        }
    }
}

impl TrackMetadata {
    /// Get display name (artist - title if available, otherwise filename)
    pub fn get_display_name(&self) -> String {
        if !self.artist.is_empty() && !self.title.is_empty() {
            format!("{} - {}", self.artist, self.title)
        } else if !self.title.is_empty() {
            self.title.clone()
        } else {
            self.filename.clone()
        }
    }
}

/// Current playback status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    Playing,
    Stopped,
    Crossfading,
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        PlaybackStatus::Stopped
    }
}

/// Physical playback progress reported by the audio engine executor.
pub type EngineProgress = PlaybackState;

/// Playback state (published every 500ms).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    /// Index into the play_queue vector (not a DB song id).
    pub playlist_index: i64,
    /// File path relative to media_root.
    pub file_path: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub status: PlaybackStatus,
    pub total_bytes_sent: u64,
    pub track_start_timestamp_ms: i64,
    /// Title from track metadata (filename stem if no tag).
    /// Used as fallback when the file is not registered in the DB songs table.
    #[serde(default)]
    pub title: String,
    /// Artist from track metadata (empty if unknown).
    #[serde(default)]
    pub artist: String,
    /// DB song id of the currently-playing track if it came from the request queue,
    /// or None if it came from the folder cycle.
    #[serde(default)]
    pub song_id: Option<i64>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playlist_index: 0,
            file_path: String::new(),
            position_ms: 0,
            duration_ms: 0,
            status: PlaybackStatus::Stopped,
            total_bytes_sent: 0,
            track_start_timestamp_ms: 0,
            title: String::new(),
            artist: String::new(),
            song_id: None,
        }
    }
}

/// A track requested by a user via the DB queue. Pushed onto the engine's
/// request queue by the backend; consumed by the player's main loop, which
/// prefers requests over the folder cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestedTrack {
    /// Path relative to media_root.
    pub file_path: String,
    /// DB song id (so the poller / mark_playing can find the row).
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
}

/// Type of command sent to the audio engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCommandType {
    Skip,
    Next,
    Prev,
    Play,
    Stop,
    /// Re-scan the media directory and rebuild the play queue.
    ReloadQueue,
}

/// Command sent to the audio engine (from backend/frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCommand {
    #[serde(rename = "type")]
    pub cmd_type: AudioCommandType,
    pub song_id: Option<i64>,
    pub file_path: Option<String>,
}

/// FFmpeg arguments for a single track playback
#[derive(Debug, Clone)]
pub struct FfmpegArgs {
    pub input_file: String,
    pub fade_in: bool,
    pub start_offset_secs: Option<f64>,
}

/// FFmpeg arguments for crossfade between two tracks
#[derive(Debug, Clone)]
pub struct CrossfadeFfmpegArgs {
    pub current_file: String,
    pub next_file: String,
    pub current_duration_ms: i64,
    pub crossfade_duration_secs: f64,
}
