// Shared API/WS types — mirrors radio-backend DTOs exactly (see local://api-contract.md).

export interface ApiResponse<T> {
  success: boolean
  data: T | null
  error: string | null
}

// ── station ────────────────────────────────────────────────

export interface StationInfo {
  name: string
  short_name: string
  subtitle: string
  description: string
  icon_url: string
  manifest_url: string
  stream_url: string
  ws_url: string
  needs_setup: boolean
  world_id?: string
}

// ── songs ──────────────────────────────────────────────────

export interface SongSummary {
  id: number
  title: string
  artist: string
  album: string
  duration_ms: number
  has_lyrics: boolean
  has_cover: boolean
  metadata_revision?: number
}

export interface Song extends SongSummary {
  genre: string
  year: number
  file_path: string
  lyrics_path: string
  cover_path: string
  filesize: number
  created_at: string
  ncm_song_id?: number | null
  musicbrainz_recording_id?: string | null
  metadata_source?: string
  metadata_revision?: number
}

export interface MetadataJob {
  id: string
  kind: 'local' | 'online' | 'full'
  status: 'queued' | 'running' | 'completed' | 'cancelled' | 'failed'
  scope: 'library' | 'songs'
  total: number
  processed: number
  matched: number
  needs_review: number
  failed: number
  error: string
  created_at: string
  updated_at: string
  finished_at: string | null
}

export interface MetadataJobItem {
  id: number
  job_id: string
  song_id: number
  status: string
  stage: string
  message: string
  candidates_json: string
  attempts: number
  updated_at: string
}

export interface MetadataCandidate {
  provider: string
  external_id: string
  title: string
  artists: string[]
  album: string
  duration_ms: number
  score: number
  cover_url: string | null
  release_group_id: string | null
}

export interface Paginated<T> {
  data: T[]
  total: number
  limit: number
  offset: number
}

// ── queue / now playing ────────────────────────────────────

export interface QueueItemDisplay {
  id: number
  song: SongSummary | null
  requested_by: string
  status: 'pending' | 'playing' | 'played' | 'skipped'
  position: number
  added_at: string
}

export interface HistoryItem {
  id: number
  /** Top-level song id — absent on old backends; prefer song.title. */
  song_id?: number
  device_user_id?: number | null
  played_at: string
  /** Embedded song summary; title/artist present on all backend versions. */
  song?: {
    id: number
    title: string
    artist: string
    album?: string | null
    duration_ms?: number
  }
}

export interface NowPlaying {
  song: SongSummary | null
  position_ms: number
  duration_ms: number
  lyrics_line: number | null
  lyrics_text: string | null
  started_at: string | null
  stream_url: string
  file_url: string | null
  cover_url: string | null
}

// ── auth / users ───────────────────────────────────────────

export interface AuthUser {
  id: number
  display_name: string
  role: 'admin' | 'user'
}

export interface AdminUser {
  id: number
  display_name: string
  role: string
  banned_until: string | null
  created_at: string
}

// ── listeners ──────────────────────────────────────────────

export interface ListenerInfo {
  display_name: string
  connected_at: string
}

// ── settings / ncm / downloads ─────────────────────────────

export interface StationSettings {
  station_name: string
  short_name: string
  subtitle: string
  description: string
  icon_url: string
  icon_path: string
  resolved_icon_url: string
}

export interface NcmStatus {
  configured: boolean
  method: string
  phone_hint: string
}

export interface NcmLoginResult {
  success: boolean
  output: string
}

export interface DownloadStatus {
  running: boolean
  log: string
}

export interface DownloadEvent {
  log: string
  done: boolean
  task_id?: string
}

export interface BatchDownloadItem {
  id?: string
  url?: string
  artist?: string
  title?: string
  save_as?: string
  override_lyrics?: boolean
}

export interface BatchDownloadResponse {
  task_id: string
  total: number
}

export interface BatchDownloadResultItem {
  id?: string
  title?: string
  artist?: string
  success: boolean
  error?: string
  file_path?: string
}

export interface BatchDownloadStatus {
  task_id: string
  running: boolean
  source: string
  total: number
  success: number
  failed: number
  items: BatchDownloadResultItem[]
}

export interface ImportPlaylistResponse {
  total: number
  batch_id: string
  message: string
}

export interface AdminLogEntry {
  id: number
  admin_id: number
  action: string
  details: string
  created_at: string
}

export interface AdminStats {
  // Defensive: read fields as numbers when present.
  [key: string]: unknown
}

// ── lyrics ─────────────────────────────────────────────────

export interface LyricsLine {
  time_ms: number | null
  text: string
}

// ── WebSocket ──────────────────────────────────────────────

export type PlaybackStatus = 'playing' | 'stopped' | 'crossfading'

export interface PlaybackStateWs {
  type: 'playback_state'
  song_id: number
  title: string
  artist: string
  position_ms: number
  duration_ms: number
  lyrics_line: number | null
  lyrics_lines: LyricsLine[] | null
  status: PlaybackStatus
  stream_url: string
  file_url: string | null
  cover_url: string | null
  timestamp_ms: number
}

export interface QueueUpdateWs {
  type: 'queue_update'
  action: string
  song_title: string | null
  requested_by: string | null
  queue_size: number
}

export interface NoticeWs {
  type: 'notice'
  message: string
  level: 'info' | 'warning' | 'error'
}

export interface PingWs {
  type: 'ping'
  timestamp: number
}

export interface ListenersUpdateWs {
  type: 'listeners_update'
  count: number
  names: string[]
}

export type WsMessage = PlaybackStateWs | QueueUpdateWs | NoticeWs | PingWs | ListenersUpdateWs

// ── LAN Discovery ──────────────────────────────────────────

export interface DiscoveredWorld {
  world_id: string
  station_name: string
  short_name: string
  host: string
  port: number
  base_path: string
  url: string
  version: string
  is_headless: boolean
  last_seen_ms: number
}

