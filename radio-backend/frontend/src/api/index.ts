// Typed endpoints — every call site should import from here, not fetch directly.
// Contract source: local://api-contract.md (verified against radio-backend source).

import { apiFetch, appUrl } from './client'
import type {
  AdminLogEntry,
  AdminStats,
  AdminUser,
  AuthUser,
  BatchDownloadItem,
  BatchDownloadResponse,
  BatchDownloadStatus,
  DownloadStatus,
  HistoryItem,
  ImportPlaylistResponse,
  ListenerInfo,
  NcmLoginResult,
  NcmStatus,
  NowPlaying,
  Paginated,
  QueueItemDisplay,
  Song,
  SongSummary,
  StationInfo,
  StationSettings,
  MetadataJob,
  MetadataJobItem,
  MetadataCandidate,
  DiscoveredWorld,
} from '@/types'

// ── station / misc ─────────────────────────────────────────

export function fetchStation(): Promise<StationInfo> {
  return apiFetch<StationInfo>('/api/station')
}

export function fetchNowPlaying(): Promise<NowPlaying> {
  return apiFetch<NowPlaying>('/api/now-playing')
}

export function fetchListeners(): Promise<{ count: number; listeners: ListenerInfo[] }> {
  return apiFetch('/api/listeners')
}

// ── auth ───────────────────────────────────────────────────

export function fetchMe(): Promise<AuthUser> {
  return apiFetch<AuthUser>('/api/auth/me')
}

export function setDisplayName(display_name: string): Promise<string> {
  return apiFetch<string>('/api/auth/name', { method: 'POST', body: JSON.stringify({ display_name }) })
}

export function claimAdmin(admin_setup_token: string): Promise<string> {
  return apiFetch<string>('/api/auth/claim-admin', { method: 'POST', body: JSON.stringify({ admin_setup_token }) })
}

export function adminLogout(): Promise<unknown> {
  return apiFetch('/api/admin/logout', { method: 'POST' })
}

// ── queue ──────────────────────────────────────────────────

export function fetchQueue(): Promise<QueueItemDisplay[]> {
  return apiFetch<QueueItemDisplay[]>('/api/queue')
}

export function addToQueue(song_id: number): Promise<unknown> {
  return apiFetch('/api/queue', { method: 'POST', body: JSON.stringify({ song_id }) })
}

export function removeQueueItem(id: number): Promise<unknown> {
  return apiFetch(`/api/queue/${id}`, { method: 'DELETE' })
}

export function moveQueueItem(id: number, new_position: number): Promise<unknown> {
  return apiFetch(`/api/queue/${id}/move`, { method: 'POST', body: JSON.stringify({ new_position }) })
}

export function skipCurrent(): Promise<unknown> {
  return apiFetch('/api/queue/skip', { method: 'POST' })
}

export function fetchHistory(): Promise<HistoryItem[]> {
  return apiFetch<HistoryItem[]>('/api/queue/history')
}

// ── songs ──────────────────────────────────────────────────

export function searchSongs(q: string, limit = 50, offset = 0): Promise<Paginated<SongSummary>> {
  const params = new URLSearchParams()
  if (q) params.set('q', q)
  params.set('limit', String(limit))
  params.set('offset', String(offset))
  return apiFetch<Paginated<SongSummary>>(`/api/songs?${params}`)
}

export function fetchSong(id: number): Promise<Song> {
  return apiFetch<Song>(`/api/songs/${id}`)
}

export function coverUrl(songId: number, revision?: number): string {
  const suffix = revision === undefined ? '' : `?v=${revision}`
  return appUrl(`/api/songs/${songId}/cover${suffix}`)
}

export function songStreamUrl(songId: number): string {
  return appUrl(`/api/songs/${songId}/stream`)
}

export function downloadSongUrl(songId: number): string {
  return appUrl(`/api/songs/${songId}/download`)
}

// ── ncm (device) ───────────────────────────────────────────

export function fetchNcmStatus(): Promise<NcmStatus> {
  return apiFetch<NcmStatus>('/api/ncm')
}

export function saveNcmCookie(cookie: string): Promise<unknown> {
  return apiFetch('/api/ncm', { method: 'POST', body: JSON.stringify({ cookie }) })
}

export function testNcmLogin(): Promise<NcmLoginResult> {
  return apiFetch<NcmLoginResult>('/api/ncm/test', { method: 'POST' })
}

// ── admin ──────────────────────────────────────────────────

export function fetchAdminUsers(): Promise<AdminUser[]> {
  return apiFetch<AdminUser[]>('/api/admin/users')
}

export function banUser(id: number): Promise<unknown> {
  return apiFetch(`/api/admin/users/${id}/ban`, { method: 'POST' })
}

export function unbanUser(id: number): Promise<unknown> {
  return apiFetch(`/api/admin/users/${id}/unban`, { method: 'POST' })
}

export function setUserRole(id: number, role: 'admin' | 'user'): Promise<unknown> {
  return apiFetch(`/api/admin/users/${id}/role`, { method: 'PUT', body: JSON.stringify({ role }) })
}

export function fetchAdminStats(): Promise<AdminStats> {
  return apiFetch<AdminStats>('/api/admin/stats')
}

export function fetchAdminLogs(): Promise<AdminLogEntry[]> {
  return apiFetch<AdminLogEntry[]>('/api/admin/logs')
}

export function rescanSongs(): Promise<MetadataJob> {
  return apiFetch<MetadataJob>('/api/admin/rescan-songs', { method: 'POST' })
}

export function enrichSongMetadata(): Promise<MetadataJob> {
  return apiFetch<MetadataJob>('/api/admin/enrich-song-metadata', { method: 'POST' })
}

export function startMetadataJob(body: { kind?: 'local' | 'online' | 'full'; scope?: 'library' | 'songs'; song_ids?: number[]; force?: boolean }): Promise<MetadataJob> {
  return apiFetch<MetadataJob>('/api/admin/metadata/jobs', { method: 'POST', body: JSON.stringify(body) })
}

export function fetchMetadataJob(id: string): Promise<MetadataJob> { return apiFetch<MetadataJob>(`/api/admin/metadata/jobs/${encodeURIComponent(id)}`) }
export function fetchMetadataJobItems(id: string, status?: string): Promise<MetadataJobItem[]> {
  const query = status ? `?status=${encodeURIComponent(status)}` : ''
  return apiFetch<MetadataJobItem[]>(`/api/admin/metadata/jobs/${encodeURIComponent(id)}/items${query}`)
}
export function cancelMetadataJob(id: string): Promise<string> { return apiFetch<string>(`/api/admin/metadata/jobs/${encodeURIComponent(id)}/cancel`, { method: 'POST' }) }
export function retryMetadataJob(id: string): Promise<MetadataJob> { return apiFetch<MetadataJob>(`/api/admin/metadata/jobs/${encodeURIComponent(id)}/retry`, { method: 'POST' }) }
export function fetchMetadataCandidates(songId: number): Promise<MetadataCandidate[]> { return apiFetch<MetadataCandidate[]>(`/api/admin/songs/${songId}/metadata/candidates`) }
export function applyMetadataCandidate(songId: number, candidate: MetadataCandidate): Promise<string> { return apiFetch<string>(`/api/admin/songs/${songId}/metadata/candidates/apply`, { method: 'POST', body: JSON.stringify({ candidate }) }) }
export function updateSongMetadata(id: number, body: { title: string; artist: string; album: string; lock_title?: boolean; lock_artist?: boolean; lock_album?: boolean }): Promise<string> {
  return apiFetch<string>(`/api/admin/songs/${id}/metadata`, { method: 'PATCH', body: JSON.stringify(body) })
}

export function fetchAdminSongs(): Promise<Song[]> {
  return apiFetch<Song[]>('/api/admin/songs')
}

export function deleteSongAdmin(id: number): Promise<unknown> {
  return apiFetch(`/api/admin/songs/${id}`, { method: 'DELETE' })
}

export function uploadSongFile(file: File): Promise<unknown> {
  const form = new FormData()
  form.append('file', file)
  return apiFetch('/api/admin/upload', { method: 'POST', body: form })
}

export function fetchStationSettings(): Promise<StationSettings> {
  return apiFetch<StationSettings>('/api/admin/settings')
}

export function saveStationSettings(patch: {
  station_name?: string
  short_name?: string
  subtitle?: string
  description?: string
  icon_url?: string
}): Promise<unknown> {
  return apiFetch('/api/admin/settings', { method: 'POST', body: JSON.stringify(patch) })
}

export function uploadStationIcon(file: File): Promise<unknown> {
  const form = new FormData()
  form.append('file', file)
  return apiFetch('/api/admin/settings/icon', { method: 'POST', body: form })
}

export function adminSkipNext(): Promise<unknown> {
  return apiFetch('/api/admin/playlist/next', { method: 'POST' })
}

export function adminSkipPrev(): Promise<unknown> {
  return apiFetch('/api/admin/playlist/prev', { method: 'POST' })
}

export function startLegacyDownload(playlist: string, quality?: string, format?: string): Promise<unknown> {
  return apiFetch('/api/admin/download', { method: 'POST', body: JSON.stringify({ playlist, quality, format }) })
}

export function fetchLegacyDownloadStatus(): Promise<DownloadStatus> {
  return apiFetch<DownloadStatus>('/api/admin/download/status')
}

export function startBatchDownload(payload: {
  source: string
  items: BatchDownloadItem[]
  lyrics_save_mode?: 'none' | 'separate'
  quality?: 'standard' | 'high' | 'exhigh' | 'lossless'
  format?: string
}): Promise<BatchDownloadResponse> {
  return apiFetch<BatchDownloadResponse>('/api/admin/download/batch', { method: 'POST', body: JSON.stringify(payload) })
}

export function fetchBatchDownloadStatus(taskId: string): Promise<BatchDownloadStatus> {
  return apiFetch<BatchDownloadStatus>(`/api/admin/download/batch/status?task_id=${encodeURIComponent(taskId)}`)
}

export function fetchAdminNcmStatus(): Promise<NcmStatus> {
  return apiFetch<NcmStatus>('/api/admin/ncm')
}

export function saveAdminNcmCookie(cookie: string): Promise<unknown> {
  return apiFetch('/api/admin/ncm', { method: 'POST', body: JSON.stringify({ cookie }) })
}

export function testAdminNcmLogin(): Promise<NcmLoginResult> {
  return apiFetch<NcmLoginResult>('/api/admin/ncm/test', { method: 'POST' })
}

export function importNcmPlaylist(link: string): Promise<ImportPlaylistResponse> {
  return apiFetch<ImportPlaylistResponse>('/api/admin/ncm/playlist', { method: 'POST', body: JSON.stringify({ link }) })
}

export function startNcmImport(batch_id: string): Promise<BatchDownloadResponse> {
  return apiFetch<BatchDownloadResponse>('/api/admin/ncm/import', { method: 'POST', body: JSON.stringify({ batch_id }) })
}

// ── LAN Discovery ──────────────────────────────────────────

export function fetchDiscoveredWorlds(): Promise<DiscoveredWorld[]> {
  return apiFetch<DiscoveredWorld[]>('/api/discovery/worlds')
}

export function scanLanWorlds(): Promise<void> {
  return apiFetch('/api/discovery/scan', { method: 'POST' })
}

export { apiFetch, apiBlob, appUrl, consumeSse } from './client'

