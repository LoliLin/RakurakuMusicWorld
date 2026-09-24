import { useState } from 'react'
import {
  ContextMenu,
  ContextMenuTrigger,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
} from '@appica/ui-react/context-menu'
import { Spinner } from '@appica/ui-react/spinner'
import { Button } from '@appica/ui-react/button'
import { FileText, Heart, HeartFilled, PlayerPlay } from '@appica/icons-react'
import { addToQueue, coverUrl } from '@/api'
import { SongArtwork } from '@/components/SongArtwork'
import { formatTime } from '@/lib/format'
import { useStore } from '@/store'
import type { SongSummary } from '@/types'

export interface SongRowProps {
  song: SongSummary
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : '操作失败'
}

/** Song actions stay visible; the context menu remains a shortcut. */
export function SongRow({ song }: SongRowProps) {
  const addToast = useStore((s) => s.addToast)
  const favoriteSongs = useStore((s) => s.favoriteSongs)
  const toggleFavorite = useStore((s) => s.toggleFavorite)
  const favorited = favoriteSongs.some((f) => f.song.id === song.id)
  const [queuing, setQueuing] = useState(false)

  const handleQueue = async () => {
    if (queuing) return
    setQueuing(true)
    try {
      await addToQueue(song.id)
      addToast('已加入点歌队列', 'success')
    } catch (e) {
      addToast(errorMessage(e), 'error')
    } finally {
      setQueuing(false)
    }
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger className="block w-full">
        <div className="hover:bg-background-subtle flex w-full items-center gap-2.5 px-3 py-3 transition-colors sm:gap-3 sm:px-4">
          <SongArtwork hasCover={song.has_cover} coverSrc={coverUrl(song.id, song.metadata_revision)} size="md" className="shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 items-center gap-1.5">
              <p className="text-foreground-intense min-w-0 truncate text-sm font-semibold" title={song.title}>{song.title}</p>
              {song.has_lyrics && (
                <FileText aria-label="有歌词" className="size-3.5 shrink-0 text-foreground-subtle" />
              )}
            </div>
            <p className="min-w-0 truncate text-xs text-foreground-muted">
              {song.artist}
              {song.album ? ` · ${song.album}` : ''}
            </p>
          </div>
          <span className="text-foreground-muted hidden shrink-0 text-xs tabular-nums sm:block">{formatTime(song.duration_ms)}</span>
          <Button
            variant="ghost"
            size="icon-md"
            aria-label={favorited ? `取消收藏 ${song.title}` : `收藏 ${song.title}`}
            aria-pressed={favorited}
            onClick={() => toggleFavorite(song)}
            className="min-h-11 min-w-11 shrink-0"
          >
            {favorited ? <HeartFilled className="text-primary" /> : <Heart />}
          </Button>
          <Button variant="primary" size="sm" disabled={queuing} onClick={() => void handleQueue()} className="min-h-11 shrink-0">
            {queuing ? <Spinner className="size-4" currentColor /> : <PlayerPlay data-icon="start" />}
            点歌
          </Button>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        <ContextMenuItem disabled={queuing} onClick={() => void handleQueue()}>
          {queuing ? <Spinner className="size-4" currentColor /> : <PlayerPlay data-icon="start" />}
          点歌
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onClick={() => toggleFavorite(song)}>
          {favorited ? <HeartFilled data-icon="start" className="text-error" /> : <Heart data-icon="start" />}
          {favorited ? '取消收藏' : '收藏'}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}
