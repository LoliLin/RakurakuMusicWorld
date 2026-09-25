import { useEffect, useRef } from 'react'
import type { Playback } from '@/store'
import { activeLyricsIndex } from '@/hooks/usePlaybackClock'
import { cn } from '@/lib/cn'

export interface LyricsPanelProps {
  /** Current playback snapshot (store.playback). */
  playback: Playback | null
  /** Smoothed playback position in ms (usePlaybackClock output). */
  positionMs: number
  /** Applied to the scroll container — use to cap/override the height. */
  className?: string
}

/**
 * Karaoke-style lyrics panel. The active line is highlighted via the
 * `.lyrics-active` / `.lyrics-inactive` classes (index.css) and auto-scrolled
 * into view (smooth, only when the active index changes).
 *
 * States: `lyricsLines === null` → subtle placeholder (lyrics not delivered
 * yet); `[]` → "no lyrics" empty state; otherwise the line list.
 */
export function LyricsPanel({ playback, positionMs, className }: LyricsPanelProps) {
  const lines = playback?.lyricsLines ?? null
  const activeIndex = activeLyricsIndex(lines, positionMs)
  const containerRef = useRef<HTMLDivElement>(null)
  const songKey = playback ? `${playback.songId}|${playback.title}|${playback.streamUrl}` : ''
  const lastActiveRef = useRef<{ key: string; index: number }>({ key: '', index: -2 })

  useEffect(() => {
    if (activeIndex < 0) return
    const prev = lastActiveRef.current
    if (prev.key === songKey && prev.index === activeIndex) return
    lastActiveRef.current = { key: songKey, index: activeIndex }
    const line = containerRef.current?.querySelector<HTMLElement>('[data-lyrics-active="true"]')
    // scrollIntoView walks up to whichever ancestor actually scrolls
    // (the panel's own overflow container, or a wrapping ScrollArea).
    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    line?.scrollIntoView({ block: 'center', behavior: reduceMotion ? 'auto' : 'smooth' })
  }, [activeIndex, songKey])

  if (!lines) {
    // lines === null: the full lyrics frame hasn't arrived (or the server
    // never re-sends it after a WS reconnect for the same song). If the song
    // change was already observed (lyricsKnown), treat it as no-lyrics.
    const noLyrics = playback?.lyricsKnown === true
    return (
      <p className={cn('text-foreground-subtle py-10 text-center text-sm', className)}>
        {noLyrics ? '暂无歌词' : playback ? '歌词加载中…' : '播放开始后，歌词会显示在这里。'}
      </p>
    )
  }

  if (lines.length === 0) {
    return <p className={cn('text-foreground-subtle py-10 text-center text-sm', className)}>暂无歌词</p>
  }

  return (
    <div
      ref={containerRef}
      role="region"
      aria-label="歌词"
      className={cn('max-h-[45vh] overflow-y-auto px-1 py-1', className)}
    >
      {lines.map((line, i) => (
        <p
          key={i}
          data-lyrics-active={i === activeIndex}
          aria-current={i === activeIndex || undefined}
          className={cn(
            'py-2 text-base leading-relaxed transition-colors',
            i === activeIndex
              ? 'lyrics-active text-foreground-intense font-medium'
              : 'lyrics-inactive',
          )}
        >
          {line.text || '\u00a0'}
        </p>
      ))}
    </div>
  )
}
