// Singleton audio element + stream lifecycle.
//
// The /stream endpoint is a live ring buffer: on admin skip/prev the server
// resyncs readers and closes the connection, so the <audio> element fires
// 'ended'/'error' — we must reconnect to the live edge (cache-busting query
// param so the browser doesn't serve a stale buffer).
//
// Autoplay policy: browsers block sound until a user gesture. The first play()
// attempt (page load) rejects with NotAllowedError → we show a resume overlay
// (store.needsPlay). Once the user clicks it, `userAuthorized` flips and every
// subsequent reconnect calls play() directly; if the browser still rejects
// (rare), we stay silent and retry on the next sync tick instead of re-showing
// the overlay — otherwise every stream hiccup would trap the user in the
// overlay loop.

import { useStore } from '@/store'
import { appRoot, appUrl } from '@/api/client'

let audio: HTMLAudioElement | null = null
let reconnectNonce = 0
let errorRetries = 0
let userAuthorized = false
let connectionErrorNotified = false
let retryTimer: number | null = null
let lastProgressAt = 0
let lastReconnectAt = 0
let watchdogStarted = false

/** 无数据推进多久后强制重连。网络半断 / 服务端静默时元素既不会 error 也不会 ended，
 *  会一直卡在 waiting —— 这是"掉线后不能快速重连"的主因。 */
const STALL_RECONNECT_MS = 8000
/** 错误重试退避：0.5s → 1s → 2s → 4s → 8s 封顶；成功(playing)后重置。 */
const BASE_RETRY_MS = 500
const MAX_RETRY_MS = 8000

/** origin + pathname comparison — ignores the ?r= reconnect nonce. */
function urlBase(u: string): string {
  try {
    const parsed = new URL(u, appRoot())
    return `${parsed.origin}${parsed.pathname}`
  } catch {
    return u
  }
}

function ensureAudio(): HTMLAudioElement {
  if (audio) return audio
  const el = new Audio()
  el.preload = 'none'
  // 恢复持久化的音量（默认 0.8）。
  const raw = Number(localStorage.getItem('rakuraku.volume'))
  el.volume = Number.isFinite(raw) && raw >= 0 && raw <= 1 ? raw : 0.8
  el.addEventListener('ended', () => {
    // Server closed the stream (skip/prev resync or idle timeout) → rejoin live edge.
    reconnect()
  })
  el.addEventListener('error', () => {
    errorRetries += 1
    if (errorRetries > 12 && !connectionErrorNotified) {
      connectionErrorNotified = true
      useStore.getState().addToast('直播流连接不稳定，正在自动重试…', 'warning')
    }
    // 永不停止重试：指数退避后继续。旧的实现 12 次后停 60s，网络/服务端
    // 在这 60s 内恢复也会保持静默。
    scheduleRetry()
  })
  el.addEventListener('playing', () => {
    cancelRetry()
    errorRetries = 0
    connectionErrorNotified = false
    lastProgressAt = Date.now()
  })
  el.addEventListener('timeupdate', () => {
    lastProgressAt = Date.now()
  })
  audio = el
  lastProgressAt = Date.now()
  startWatchdog()
  return el
}

function desiredStreamUrl(): string | null {
  const p = useStore.getState().playback
  if (!p || p.status === 'stopped') return null
  const url = p.streamUrl
  if (!url) return null
  if (url.startsWith('http://') || url.startsWith('https://')) {
    return url
  }
  return appUrl(url.startsWith('/') ? url : `/${url}`)
}

export function reconnect() {
  const url = desiredStreamUrl()
  if (!url) return
  // 去重：ended 与 error 可能在同一 tick 内先后触发，避免重复重连。
  const now = Date.now()
  if (now - lastReconnectAt < 250) return
  // An accepted reconnect supersedes a retry left by the previous element
  // state. Keep the timer when this call is deduped, or an immediate
  // error+ended burst could otherwise cancel its only recovery attempt.
  cancelRetry()
  lastReconnectAt = now
  reconnectNonce += 1
  lastProgressAt = now
  const el = ensureAudio()
  const sep = url.includes('?') ? '&' : '?'
  el.src = `${url}${sep}r=${reconnectNonce}`
  void tryPlay(el)
}

/** 指数退避间隔，errorRetries 在 playing 时归零。 */
function retryDelayMs(): number {
  const exp = Math.min(errorRetries, 4)
  return Math.min(BASE_RETRY_MS * 2 ** exp, MAX_RETRY_MS)
}

/** 单飞重试：同一时刻最多一个待触发的重连，避免多个错误事件叠加。 */
function scheduleRetry() {
  if (retryTimer !== null) return
  retryTimer = window.setTimeout(() => {
    retryTimer = null
    reconnect()
  }, retryDelayMs())
}

function cancelRetry() {
  if (retryTimer === null) return
  window.clearTimeout(retryTimer)
  retryTimer = null
}

/** 停滞看门狗：连接存活但长时间无数据推进（半断网、服务端静默）时主动重连。 */
function startWatchdog() {
  if (watchdogStarted) return
  watchdogStarted = true
  window.setInterval(() => {
    const el = audio
    if (!el || !el.src) return
    const { audioPaused, needsPlay, playback } = useStore.getState()
    if (audioPaused || needsPlay || playback?.status !== 'playing') return
    if (Date.now() - lastProgressAt > STALL_RECONNECT_MS) {
      lastProgressAt = Date.now() // 每个窗口最多触发一次
      reconnect()
    }
  }, 1000)
}

async function tryPlay(el: HTMLAudioElement) {
  const { audioPaused, needsPlay } = useStore.getState()
  if (audioPaused || needsPlay) return
  try {
    await el.play()
  } catch (e) {
    if (!userAuthorized) {
      // First contact: browser blocks sound until a user gesture.
      useStore.getState().setNeedsPlay(true)
      return
    }
    const notAllowed = e instanceof DOMException && e.name === 'NotAllowedError'
    if (notAllowed) {
      // Autoplay gate (unlikely after first unlock): stay silent; the next
      // sync tick / watchdog retries.
      return
    }
    // Stream-level failure on a dead element: rejoin the live edge by
    // re-pointing the src (play() alone keeps rejecting on an errored
    // element, which is what made reconnects after network drops dead).
    scheduleRetry()
  }
}

/** Called on every store change: keeps the element pointed at the right URL. */
export function syncAudio() {
  const el = ensureAudio()
  const url = desiredStreamUrl()
  const { playback, audioPaused, needsPlay } = useStore.getState()

  if (!url) {
    cancelRetry()
    el.pause()
    if (el.src) {
      el.removeAttribute('src')
      el.load()
    }
    return
  }

  const target = urlBase(url)
  const current = el.src ? urlBase(el.src) : ''

  if (current !== target) {
    cancelRetry()
    reconnectNonce += 1
    const sep = url.includes('?') ? '&' : '?'
    el.src = `${url}${sep}r=${reconnectNonce}`
    void tryPlay(el)
    return
  }

  if (el.paused && !audioPaused && !needsPlay && playback?.status !== 'stopped') {
    void tryPlay(el)
  }
}

/** User gesture path: unblock autoplay and resume. */
export function resumeAudio() {
  cancelRetry()
  userAuthorized = true
  useStore.getState().setNeedsPlay(false)
  useStore.getState().setAudioPaused(false)
  const el = ensureAudio()
  void tryPlay(el)
}

/** User explicitly pauses the stream locally. */
export function pauseAudio() {
  cancelRetry()
  const el = ensureAudio()
  el.pause()
  useStore.getState().setAudioPaused(true)
}

export function isAudioPlaying(): boolean {
  return audio !== null && !audio.paused && !audio.ended
}

/** 应用音量（0–1）到音频元素。 */
export function setAudioVolume(volume: number) {
  const el = ensureAudio()
  el.volume = Math.min(1, Math.max(0, volume))
}

export function isUserAuthorized(): boolean {
  return userAuthorized
}
