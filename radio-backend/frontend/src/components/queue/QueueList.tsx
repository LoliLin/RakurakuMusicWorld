import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogClose,
} from '@appica/ui-react/alert-dialog'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from '@appica/ui-react/dropdown-menu'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { Skeleton } from '@appica/ui-react/skeleton'
import { ChevronDown, Library, Loader, Music, Trash } from '@appica/icons-react'
import { fetchQueue, moveQueueItem, removeQueueItem, coverUrl } from '@/api'
import { SongArtwork } from '@/components/SongArtwork'
import { loadSongIndex, resolveSong } from '@/lib/songIndex'
import { useStore, type Toast } from '@/store'
import type { QueueItemDisplay } from '@/types'
import { formatDateTime } from '@/lib/format'

export interface QueueListProps {
  /** Override the data source; defaults to the store's queue. */
  items?: QueueItemDisplay[]
  /** Called after any mutation-driven refetch settles. */
  onChanged?: () => void
}

const STATUS_META: Record<QueueItemDisplay['status'], { label: string; variant: 'soft' | 'primary' | 'light' }> = {
  pending: { label: '等待中', variant: 'soft' },
  playing: { label: '播放中', variant: 'primary' },
  played: { label: '已播放', variant: 'light' },
  skipped: { label: '已跳过', variant: 'light' },
}

function itemTitle(item: QueueItemDisplay): string {
  return item.song?.title ?? '未知歌曲'
}

/**
 * Queue list. Renders the store's queue (or an explicit `items` override).
 * 管理员的排序/移除操作收纳在 Context Menu（右键/长按）中。
 */
export function QueueList({ items, onChanged }: QueueListProps) {
  const storeQueue = useStore((s) => s.queue)
  const isAdmin = useStore((s) => s.auth?.role === 'admin')
  const queue = items ?? storeQueue
  const [loaded, setLoaded] = useState(false)
  const [busyId, setBusyId] = useState<number | null>(null)
  const [indexReady, setIndexReady] = useState(false)
  const [removeTarget, setRemoveTarget] = useState<QueueItemDisplay | null>(null)
  const [removing, setRemoving] = useState(false)
  const [error, setError] = useState(false)

  // 兜底：后端尚未部署真实 song.id 修复时（id 恒为 0），按 title|artist
  // 从曲库索引解析真实 id 以加载封面；新后端 id>0 时 resolveSong 直接返回。
  useEffect(() => {
    let cancelled = false
    loadSongIndex()
      .then(() => {
        if (!cancelled) setIndexReady(true)
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [])

  // First load (store mode only — an `items` prop implies the owner fetches).
  useEffect(() => {
    if (items) {
      setLoaded(true)
      return
    }
    let cancelled = false
    fetchQueue()
      .then((q) => {
        if (!cancelled) useStore.getState().setQueue(q)
      })
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) setLoaded(true)
      })
    return () => {
      cancelled = true
    }
  }, [items])

  const refresh = () => {
    void fetchQueue()
      .then((q) => { useStore.getState().setQueue(q); setError(false) })
      .catch(() => { setError(true); useStore.getState().addToast('刷新队列失败', 'error') })
      .finally(() => onChanged?.())
  }

  const handleMove = async (item: QueueItemDisplay, delta: number) => {
    setBusyId(item.id)
    try {
      await moveQueueItem(item.id, item.position + delta)
      useStore.getState().addToast('已调整顺序', 'success' as Toast['level'])
      refresh()
    } catch (err) {
      useStore.getState().addToast(err instanceof Error ? err.message : '调整顺序失败', 'error')
    } finally {
      setBusyId(null)
    }
  }

  const confirmRemove = async () => {
    if (!removeTarget || removing) return
    setRemoving(true)
    try {
      // NOTE: backend quirk — item.song.id is always 0; use item.id.
      await removeQueueItem(removeTarget.id)
      useStore.getState().addToast('已从队列移除', 'success' as Toast['level'])
      refresh()
    } catch (err) {
      useStore.getState().addToast(err instanceof Error ? err.message : '移除失败', 'error')
    } finally {
      setRemoving(false)
      setRemoveTarget(null)
    }
  }

  if (!loaded && queue.length === 0) {
    return (
      <ul
        aria-label="点歌队列"
        className="divide-border-muted divide-y overflow-hidden rounded-xl border border-border-muted bg-background"
      >
        {Array.from({ length: 4 }, (_, i) => (
          <li key={i} className="flex items-center gap-3 px-3 py-3 sm:px-4">
            <Skeleton className="size-4 shrink-0 rounded" />
            <Skeleton className="size-9 shrink-0 rounded-lg" />
            <div className="min-w-0 flex-1 space-y-2">
              <Skeleton className="h-3.5 w-2/3 rounded" />
              <Skeleton className="h-3 w-1/3 rounded" />
            </div>
            <Skeleton className="h-5 w-16 shrink-0 rounded-full" />
          </li>
        ))}
      </ul>
    )
  }

  if (queue.length === 0) {
    if (error) return <div className="app-panel flex flex-wrap items-center justify-between gap-3 p-5" role="alert"><p className="text-error text-sm">队列加载失败，请检查连接。</p><Button variant="outline" size="sm" onClick={refresh}>重试</Button></div>
    return (
      <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border-muted bg-background-subtle/40 px-6 py-14 text-center">
        <Music className="text-foreground-muted size-10" aria-hidden="true" />
        <p className="text-foreground-muted text-sm">队列空，去曲库点歌吧</p>
        <Button render={<Link to="/library" />} nativeButton={false} variant="primary" size="sm">
          <Library data-icon="start" />
          去曲库点歌
        </Button>
      </div>
    )
  }

  return (
    <>
      {error && <div className="app-panel mb-3 flex flex-wrap items-center justify-between gap-3 p-3" role="alert"><p className="text-error text-sm">无法刷新队列，以下显示上次结果。</p><Button variant="outline" size="sm" onClick={refresh}>重试</Button></div>}
      <ul
        aria-label="点歌队列"
        className="divide-border-muted divide-y overflow-hidden rounded-xl border border-border-muted bg-background"
      >
        {queue.map((item, index) => {
          const meta = STATUS_META[item.status] ?? STATUS_META.pending
          // 后端对非管理员点歌将 requested_by 置为 "匿名"——不展示。
          const requester = item.requested_by && item.requested_by !== '匿名' ? item.requested_by : ''
          const secondary = item.song?.artist ?? ''
          // 统一封面行为：id>0（新后端）直接用；id=0（旧后端）用索引匹配。
          const realSong = indexReady ? resolveSong(item.song) : null
          const artworkSong = realSong ?? item.song
          return (
            <li key={item.id} className="px-3 py-3 sm:px-4">
              <div className="flex min-w-0 items-center gap-2.5">
                <span className="text-foreground-muted w-4 shrink-0 text-center text-xs font-medium tabular-nums">{item.position}</span>
                <SongArtwork
                  hasCover={artworkSong?.has_cover ?? false}
                  coverSrc={artworkSong && artworkSong.id > 0 ? coverUrl(artworkSong.id, artworkSong.metadata_revision) : undefined}
                  size="sm"
                  className="shrink-0"
                />
                <div className="min-w-0 flex-1">
                  <p className="text-foreground-intense truncate text-sm font-semibold" title={itemTitle(item)}>{itemTitle(item)}</p>
                  {secondary && <p className="text-foreground-muted truncate text-xs">{secondary}</p>}
                </div>
              </div>
              <div className="mt-2 flex min-w-0 items-center justify-between gap-2 ps-7">
                <p className="text-foreground-muted min-w-0 truncate text-xs">
                  {requester ? `点歌人 ${requester}` : formatDateTime(item.added_at)}
                </p>
                <div className="flex shrink-0 items-center gap-1">
                  <Badge variant={meta.variant} size="sm">{meta.label}</Badge>
                {isAdmin && (
                  <DropdownMenu>
                    <DropdownMenuTrigger render={<Button variant="ghost" size="sm" aria-label={`管理 ${itemTitle(item)}`} />}>
                      管理 <ChevronDown data-icon="end" className="size-3.5" />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="w-40">
                    <DropdownMenuItem
                      disabled={busyId !== null || index === 0}
                      onClick={() => void handleMove(item, -1)}
                    >
                      上移
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      disabled={busyId !== null || index === queue.length - 1}
                      onClick={() => void handleMove(item, 1)}
                    >
                      下移
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      className="text-error-emphasis! data-highlighted:before:bg-error-subtle!"
                      onClick={() => setRemoveTarget(item)}
                    >
                      <Trash data-icon="start" />
                      移除
                    </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                )}
                </div>
              </div>
            </li>
          )
        })}
      </ul>

      <AlertDialog open={removeTarget !== null} onOpenChange={(open) => !open && setRemoveTarget(null)}>
        <AlertDialogContent className="sm:w-110">
          <AlertDialogHeader>
            <AlertDialogTitle>移除歌曲？</AlertDialogTitle>
            <AlertDialogDescription>
              确定要将「{removeTarget ? itemTitle(removeTarget) : ''}」从队列中移除吗？
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogClose render={<Button variant="outline" disabled={removing}>取消</Button>} />
            <Button variant="destructive" onClick={() => void confirmRemove()} disabled={removing}>
              {removing ? <Loader data-icon="start" className="animate-spin" /> : <Trash data-icon="start" />}
              移除
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
