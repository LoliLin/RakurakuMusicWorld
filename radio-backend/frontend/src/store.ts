import { create } from 'zustand'
import type {
  AuthUser,
  HistoryItem,
  ListenersUpdateWs,
  LyricsLine,
  PlaybackStateWs,
  PlaybackStatus,
  QueueItemDisplay,
  QueueUpdateWs,
  SongSummary,
  StationInfo,
} from '@/types'
import { showToast, type ToastLevel } from '@/lib/toast'
import { DEFAULT_ACCENT, dynamicAccent, type AccentTheme } from '@/lib/accents'

/** Re-exported so existing call sites (`Toast['level']`) keep compiling. */
export type Toast = { level: ToastLevel }

/** A locally-stored favorite: song snapshot + when it was added. */
export interface FavoriteSong {
  song: SongSummary
  addedAt: number
}

const FAVORITES_KEY = 'rakuraku.favorites'

function loadFavorites(): FavoriteSong[] {
  try {
    const raw = localStorage.getItem(FAVORITES_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as FavoriteSong[]
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function persistFavorites(list: FavoriteSong[]) {
  localStorage.setItem(FAVORITES_KEY, JSON.stringify(list))
}

const ACCENT_KEY = 'rakuraku.accent'

function loadAccent(): AccentTheme {
  try {
    const raw = localStorage.getItem(ACCENT_KEY)
    if (!raw) return DEFAULT_ACCENT
    const parsed = JSON.parse(raw) as AccentTheme
    if (parsed && typeof parsed.seed === 'string' && typeof parsed.light === 'string' && typeof parsed.dark === 'string') {
      return parsed
    }
    // Legacy format: a bare hex string → treat as the seed.
    if (typeof parsed === 'string') return dynamicAccent(parsed)
    return DEFAULT_ACCENT
  } catch {
    return DEFAULT_ACCENT
  }
}

function persistAccent(accent: AccentTheme) {
  localStorage.setItem(ACCENT_KEY, JSON.stringify(accent))
}

export interface Playback {
  songId: number
  title: string
  artist: string
  positionMs: number
  durationMs: number
  lyricsLine: number | null
  /** Full lyrics for the current song; null until the song-change frame arrives. */
  lyricsLines: LyricsLine[] | null
  /** [] means "no lyrics for this song" — distinct from null ("not resent"). */
  lyricsKnown: boolean
  status: PlaybackStatus
  streamUrl: string
  fileUrl: string | null
  coverUrl: string | null
  timestampMs: number
}

export type AudioStatus = 'idle' | 'connecting' | 'playing' | 'paused' | 'reconnecting' | 'error'

interface AppStore {
  station: StationInfo | null
  auth: AuthUser | null
  playback: Playback | null
  queue: QueueItemDisplay[]
  history: HistoryItem[]
  listeners: { count: number; names: string[] }
  favoriteSongs: FavoriteSong[]
  wsConnected: boolean
  audioPaused: boolean
  audioStatus: AudioStatus
  needsPlay: boolean
  accent: AccentTheme
  volume: number

  setStation: (station: StationInfo | null) => void
  setAuth: (auth: AuthUser | null) => void
  setQueue: (queue: QueueItemDisplay[]) => void
  setHistory: (history: HistoryItem[]) => void
  setWsConnected: (connected: boolean) => void
  setAudioPaused: (paused: boolean) => void
  setAudioStatus: (status: AudioStatus) => void
  setNeedsPlay: (needs: boolean) => void
  setAccent: (accent: AccentTheme) => void
  setVolume: (volume: number) => void
  addToast: (message: string, level?: ToastLevel) => void
  toggleFavorite: (song: SongSummary) => void
  applyPlaybackState: (msg: PlaybackStateWs) => void
  applyQueueUpdate: (msg: QueueUpdateWs) => void
  applyListeners: (msg: ListenersUpdateWs) => void
}

function songKey(p: Playback | null): string {
  return p ? `${p.songId}|${p.title}|${p.streamUrl}` : ''
}

export const useStore = create<AppStore>((set, get) => ({
  station: null,
  auth: null,
  playback: null,
  queue: [],
  history: [],
  listeners: { count: 0, names: [] },
  favoriteSongs: loadFavorites(),
  wsConnected: false,
  audioPaused: false,
  audioStatus: 'idle',
  needsPlay: false,
  accent: loadAccent(),
  volume: (() => {
    const stored = localStorage.getItem('rakuraku.volume')
    const raw = stored === null ? Number.NaN : Number(stored)
    return Number.isFinite(raw) && raw >= 0 && raw <= 1 ? raw : 0.8
  })(),

  setStation: (station) => set({ station }),
  setAuth: (auth) => set({ auth }),
  setQueue: (queue) => set({ queue }),
  setHistory: (history) => set({ history }),
  setWsConnected: (wsConnected) => set({ wsConnected }),
  setAudioPaused: (audioPaused) => set({ audioPaused }),
  setAudioStatus: (audioStatus) => set((state) => state.audioStatus === audioStatus ? state : { audioStatus }),
  setNeedsPlay: (needsPlay) => set({ needsPlay }),
  setAccent: (accent) => {
    persistAccent(accent)
    set({ accent })
  },

  setVolume: (volume) => {
    const clamped = Math.min(1, Math.max(0, volume))
    localStorage.setItem('rakuraku.volume', String(clamped))
    set({ volume: clamped })
  },

  addToast: (message, level = 'info') => {
    showToast(message, level)
  },

  toggleFavorite: (song) => {
    const next = (() => {
      const current = get().favoriteSongs
      const exists = current.some((f) => f.song.id === song.id)
      if (exists) return current.filter((f) => f.song.id !== song.id)
      return [{ song, addedAt: Date.now() }, ...current]
    })()
    persistFavorites(next)
    set({ favoriteSongs: next })
  },

  applyPlaybackState: (msg) => {
    const prev = get().playback
    const changedSong = songKey(prev) !== `${msg.song_id}|${msg.title}|${msg.stream_url}`
    // 后端语义：lyrics_lines 非 null 就是全量（切歌首帧，或新连接补发帧），
    // 直接更新；null 表示"本帧不重发"。同一首歌保留缓存，切歌时立即清空。
    const lyricsLines = msg.lyrics_lines !== null ? msg.lyrics_lines : changedSong ? null : prev?.lyricsLines ?? null
    const lyricsKnown = msg.lyrics_lines !== null ? true : changedSong ? false : (prev?.lyricsKnown ?? false)

    set({
      playback: {
        songId: msg.song_id,
        title: msg.title,
        artist: msg.artist,
        positionMs: msg.position_ms,
        durationMs: msg.duration_ms,
        lyricsLine: msg.lyrics_line,
        lyricsLines,
        lyricsKnown,
        status: msg.status,
        streamUrl: msg.stream_url,
        fileUrl: msg.file_url,
        coverUrl: msg.cover_url,
        timestampMs: msg.timestamp_ms,
      },
    })

    // Browser notification on song change when the tab is hidden.
    if (changedSong && document.hidden && typeof Notification !== 'undefined' && Notification.permission === 'granted') {
      try {
        new Notification(msg.title, { body: msg.artist })
      } catch {
        // some browsers reject construction without a service worker
      }
    }
  },

  applyQueueUpdate: (msg) => {
    const { queue } = get()
    if (msg.action === 'added' && msg.song_title) {
      get().addToast(`已点播：${msg.song_title}`, 'info')
      void import('@/api').then(({ fetchQueue }) => fetchQueue().then((q) => get().setQueue(q)).catch(() => undefined))
    }
    if (queue.length !== msg.queue_size) {
      void import('@/api').then(({ fetchQueue }) => fetchQueue().then((q) => get().setQueue(q)).catch(() => undefined))
    }
  },

  applyListeners: (msg) => {
    // Multiple devices can share a display name; dedupe for rendering while
    // keeping the authoritative server count.
    const names = Array.from(new Set(msg.names))
    set({ listeners: { count: msg.count, names } })
  },
}))
