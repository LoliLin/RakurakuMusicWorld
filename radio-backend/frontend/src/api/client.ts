import type { ApiResponse } from '@/types'

const SERVER_URL_KEY = 'rakuraku.server_url'
const DEVICE_TOKEN_KEY = 'rakuraku.device_token'

export function getCustomServerUrl(): string {
  if (typeof window === 'undefined') return ''
  return localStorage.getItem(SERVER_URL_KEY)?.trim().replace(/\/+$/, '') || ''
}

export function setCustomServerUrl(url: string | null): void {
  if (typeof window === 'undefined') return
  if (!url || !url.trim()) {
    localStorage.removeItem(SERVER_URL_KEY)
  } else {
    localStorage.setItem(SERVER_URL_KEY, url.trim().replace(/\/+$/, ''))
  }
}

export function getStoredDeviceToken(): string | null {
  if (typeof window === 'undefined') return null
  return localStorage.getItem(DEVICE_TOKEN_KEY)
}

export function setStoredDeviceToken(token: string): void {
  if (typeof window === 'undefined') return
  localStorage.setItem(DEVICE_TOKEN_KEY, token)
}

/** URL root: custom server or origin + BASE_URL with no trailing slash. */
export function appRoot(): string {
  const custom = getCustomServerUrl()
  if (custom) return custom
  if (typeof window !== 'undefined' && window.location.protocol === 'file:') {
    return 'http://localhost:2241'
  }
  return (window.location.origin + import.meta.env.BASE_URL).replace(/\/+$/, '')
}

/** Absolute URL for a server path starting with "/". */
export function appUrl(path: string): string {
  return appRoot() + path
}

/** WebSocket URL for a server path starting with "/". */
export function wsUrl(path: string): string {
  const root = appRoot()
  try {
    const url = new URL(root)
    const proto = url.protocol === 'https:' ? 'wss' : 'ws'
    return `${proto}://${url.host}${url.pathname.replace(/\/+$/, '')}${path}`
  } catch {
    const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
    return `${proto}://${window.location.host}${import.meta.env.BASE_URL.replace(/\/+$/, '')}${path}`
  }
}

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

function buildHeaders(initHeaders?: HeadersInit): Headers {
  const headers = new Headers(initHeaders)
  const token = getStoredDeviceToken()
  if (token && !headers.has('x-device-token')) {
    headers.set('x-device-token', token)
  }
  return headers
}

/** Fetch a JSON endpoint, unwrap {success, data, error}, throw ApiError on failure. */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  let res: Response
  try {
    const isFormData = init?.body instanceof FormData
    const headers = buildHeaders(init?.headers)
    if (!isFormData && !headers.has('Content-Type')) {
      headers.set('Content-Type', 'application/json')
    }
    res = await fetch(appUrl(path), {
      credentials: 'include',
      ...init,
      headers,
    })
  } catch {
    throw new ApiError('网络错误，无法连接服务器', 0)
  }

  // 记录后端反馈的 device_token 以便在跨域/第三方 Cookie 受限时使用
  const returnedToken = res.headers.get('x-device-token')
  if (returnedToken) {
    setStoredDeviceToken(returnedToken)
  }

  if (res.status === 401) {
    throw new ApiError('需要登录', 401)
  }
  let body: unknown
  try {
    body = await res.json()
  } catch {
    throw new ApiError(`服务器返回了无法解析的响应 (${res.status})`, res.status)
  }
  const wrapped = body as ApiResponse<T>
  if (wrapped && typeof wrapped.success === 'boolean') {
    if (!wrapped.success) {
      throw new ApiError(wrapped.error ?? '请求失败', res.status)
    }
    return wrapped.data as T
  }
  // Some endpoints (station, now-playing, listeners) return bare JSON.
  return body as T
}

/** Fetch raw bytes (blob) for covers/downloads. */
export async function apiBlob(path: string, init?: RequestInit): Promise<Blob> {
  const headers = buildHeaders(init?.headers)
  const res = await fetch(appUrl(path), { credentials: 'include', ...init, headers })
  if (!res.ok) throw new ApiError(`请求失败 (${res.status})`, res.status)
  return res.blob()
}

/** Consume an SSE stream from the backend, invoking onEvent per data line. */
export async function consumeSse(
  path: string,
  onEvent: (data: unknown) => void,
  signal?: AbortSignal,
): Promise<void> {
  const headers = buildHeaders()
  const res = await fetch(appUrl(path), { credentials: 'include', headers, signal })
  if (!res.ok || !res.body) throw new ApiError(`SSE 连接失败 (${res.status})`, res.status)
  const reader = res.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''
    for (const line of lines) {
      const trimmed = line.trim()
      if (!trimmed.startsWith('data:')) continue
      try {
        onEvent(JSON.parse(trimmed.slice(5).trim()))
      } catch {
        // ignore malformed SSE frames
      }
    }
  }
}
