import { useCallback, useEffect, useState } from 'react'
import { Input } from '@appica/ui-react/input'
import { Button } from '@appica/ui-react/button'
import { Skeleton } from '@appica/ui-react/skeleton'
import { Spinner } from '@appica/ui-react/spinner'
import { Search } from '@appica/icons-react'
import { searchSongs } from '@/api'
import { SongRow } from '@/components/library/SongRow'
import { useStore } from '@/store'
import type { SongSummary } from '@/types'

const PAGE_SIZE = 50

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : '操作失败'
}

/** 歌曲 tab：防抖搜索 + 分页加载。收藏状态由 SongRow 内部读写 localStorage。 */
export function SongSearch() {
  const addToast = useStore((s) => s.addToast)
  const [query, setQuery] = useState('')
  const [debounced, setDebounced] = useState('')
  const [songs, setSongs] = useState<SongSummary[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [retryKey, setRetryKey] = useState(0)

  // 300ms 防抖。
  useEffect(() => {
    const t = setTimeout(() => setDebounced(query.trim()), 300)
    return () => clearTimeout(t)
  }, [query])

  // 搜索（挂载时以空查询抓取全部歌曲，之后每次防抖后重新搜索）。
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    searchSongs(debounced, PAGE_SIZE, 0)
      .then((res) => {
        if (cancelled) return
        setSongs(res.data)
        setTotal(res.total)
        setError(null)
      })
      .catch((e) => {
        if (cancelled) return
        setError(errorMessage(e))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [debounced, retryKey])

  const loadMore = useCallback(async () => {
    if (loadingMore) return
    setLoadingMore(true)
    try {
      const res = await searchSongs(debounced, PAGE_SIZE, songs.length)
      setSongs((prev) => [...prev, ...res.data])
      setTotal(res.total)
    } catch (e) {
      addToast(errorMessage(e), 'error')
    } finally {
      setLoadingMore(false)
    }
  }, [debounced, songs.length, loadingMore, addToast])

  const hasMore = songs.length < total

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="搜索歌曲或艺术家…"
          aria-label="搜索歌曲"
          startSlot={<Search className="text-foreground-muted size-4" />}
          clearable
          onClear={() => setQuery('')}
          className="w-full sm:max-w-90"
        />
        <span className="text-foreground-muted text-sm tabular-nums">共 {total} 首</span>
      </div>

      {error && (
        <div className="app-panel flex flex-wrap items-center justify-between gap-3 px-4 py-3" role="alert">
          <p className="text-error text-sm">曲库加载失败：{error}{songs.length > 0 ? '。以下显示上次结果。' : ''}</p>
          <Button variant="outline" size="sm" onClick={() => setRetryKey((key) => key + 1)}>重试</Button>
        </div>
      )}

      {loading ? (
        <ul aria-label="歌曲列表" className="divide-border-muted divide-y overflow-hidden rounded-xl border border-border-muted bg-background">
          {Array.from({ length: 8 }, (_, i) => (
            <li key={i} className="flex items-center gap-3 px-3 py-2.5 sm:px-4">
              <Skeleton className="size-10 shrink-0 rounded-lg" />
              <div className="min-w-0 flex-1 space-y-2">
                <Skeleton className="h-3.5 w-2/3 rounded" />
                <Skeleton className="h-3 w-1/3 rounded" />
              </div>
            </li>
          ))}
        </ul>
      ) : error && songs.length === 0 ? null : songs.length === 0 ? (
        <p className="text-foreground-muted py-10 text-center text-sm">
          {debounced ? '没有找到匹配的歌曲' : '曲库还是空的'}
        </p>
      ) : (
        <ul aria-label="歌曲列表" className="divide-border-muted divide-y overflow-hidden rounded-xl border border-border-muted bg-background">
          {songs.map((song) => (
            <li key={song.id} className="flex">
              <SongRow song={song} />
            </li>
          ))}
        </ul>
      )}

      {hasMore && !error && (
        <Button variant="outline" size="sm" className="self-center" disabled={loadingMore} onClick={() => void loadMore()}>
          {loadingMore ? <Spinner className="size-4" currentColor /> : null}
          加载更多
        </Button>
      )}
    </div>
  )
}
