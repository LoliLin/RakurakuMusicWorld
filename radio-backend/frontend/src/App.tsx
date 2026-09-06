import { useEffect } from 'react'
import { Outlet } from 'react-router-dom'
import { useTheme } from '@appica/ui-react/hooks/use-theme'
import { fetchMe, fetchStation } from '@/api'
import { connectWebSocket, startPollers } from '@/api/ws'
import { useStore } from '@/store'
import { MainNav } from '@/components/layout/MainNav'
import { ThemeToggle } from '@/components/layout/ThemeToggle'
import { MiniPlayer } from '@/components/layout/MiniPlayer'
import { Toasts } from '@/components/layout/Toasts'
import { StreamPlayer } from '@/components/StreamPlayer'

export default function App() {
  const station = useStore((s) => s.station)
  const accent = useStore((s) => s.accent)
  const { mounted, resolvedTheme } = useTheme()

  // Bootstrap: station info, identity, websocket, fallback pollers.
  useEffect(() => {
    let cancelled = false
    void fetchStation()
      .then((st) => {
        if (!cancelled) useStore.getState().setStation(st)
      })
      .catch(() => undefined)
    void fetchMe()
      .then((me) => {
        if (!cancelled) useStore.getState().setAuth(me)
      })
      .catch(() => undefined)
    connectWebSocket()
    const stop = startPollers()
    return () => {
      cancelled = true
      stop()
    }
  }, [])

  // Brand accent → Material dynamic color pair (light/dark) applied as CSS
  // variables; index.css maps them onto --accent per theme.
  useEffect(() => {
    document.documentElement.style.setProperty('--accent-light', accent.light)
    document.documentElement.style.setProperty('--accent-dark', accent.dark)
  }, [accent])

  // Title follows the station name.
  useEffect(() => {
    document.title = station?.name ? `${station.name} · Rakuraku` : 'RakurakuMusicWorld'
  }, [station?.name])

  return (
    <div className="flex h-full flex-col">
      <header className="border-border-muted bg-background/85 sticky top-0 z-40 border-b backdrop-blur">
        <div className="mx-auto flex h-14 w-full max-w-6xl items-center gap-2 px-4">
          <MainNav />
          <div className="ms-auto flex shrink-0 items-center gap-1">
            {mounted && <ThemeToggle resolvedTheme={resolvedTheme} />}
          </div>
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-y-auto">
        <Outlet />
      </main>
      <MiniPlayer />
      <StreamPlayer />
      <Toasts />
    </div>
  )
}
