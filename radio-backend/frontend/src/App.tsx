import { useEffect, useState } from 'react'
import { Link, Outlet } from 'react-router-dom'
import { useTheme } from '@appica/ui-react/hooks/use-theme'
import { Globe, Music } from '@appica/icons-react'
import { fetchMe, fetchStation } from '@/api'
import { isLocalWorld } from '@/api/client'
import { connectWebSocket, startPollers } from '@/api/ws'
import { useStore } from '@/store'
import { MainNav } from '@/components/layout/MainNav'
import { ThemeToggle } from '@/components/layout/ThemeToggle'
import { MiniPlayer } from '@/components/layout/MiniPlayer'
import { Toasts } from '@/components/layout/Toasts'
import { StreamPlayer } from '@/components/StreamPlayer'
import { WorldSelectorDialog } from '@/components/layout/WorldSelectorDialog'

export default function App() {
  const station = useStore((s) => s.station)
  const auth = useStore((s) => s.auth)
  const isWsConnected = useStore((s) => s.wsConnected)
  const accent = useStore((s) => s.accent)
  const { mounted, resolvedTheme } = useTheme()
  const [worldDialogOpen, setWorldDialogOpen] = useState(false)
  const isLocal = isLocalWorld()

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
      <header className="border-border-muted bg-background/95 sticky top-0 z-40 border-b backdrop-blur">
        <div className="mx-auto flex h-16 w-full max-w-7xl items-center gap-2 px-4 sm:gap-5 sm:px-6">
          <Link to="/player" className="app-interactive text-foreground-intense flex shrink-0 items-center gap-2 rounded-lg font-semibold" aria-label="RakurakuMusicWorld 首页">
            <span className="bg-primary-subtle text-primary flex size-9 items-center justify-center rounded-lg"><Music className="size-5" /></span>
            <span className="hidden text-sm tracking-tight lg:block">Rakuraku Music World</span>
          </Link>
          <MainNav />
          <div className="ms-auto flex min-w-0 items-center gap-1 sm:gap-2">
            <button
              type="button"
              onClick={() => setWorldDialogOpen(true)}
              className="app-interactive border-border-muted bg-background-subtle hover:bg-background-muted/80 flex h-11 min-w-11 items-center justify-center gap-2 rounded-lg border px-2.5 text-sm font-medium transition-colors cursor-pointer sm:justify-start"
              aria-label={`切换世界，当前：${station?.name || (isLocal ? '本地世界' : '电台')}`}
            >
              <Globe className="text-primary size-4 shrink-0" aria-hidden="true" />
              <span
                className={`inline-block size-2 shrink-0 rounded-full ${
                  isWsConnected ? 'bg-success' : 'bg-neutral-muted'
                }`}
              />
              <span className="text-foreground-intense hidden max-w-[120px] truncate sm:block xl:max-w-[180px]">
                {station?.name || (isLocal ? '本地世界' : '电台')}
              </span>
              {auth?.role === 'admin' ? (
                <span className="text-primary hidden text-xs font-semibold xl:inline">房主</span>
              ) : null}
            </button>
            {mounted && <ThemeToggle resolvedTheme={resolvedTheme} />}
          </div>
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-y-auto" id="main-content">
        <Outlet />
      </main>
      <MiniPlayer />
      <StreamPlayer />
      <Toasts />
      <WorldSelectorDialog open={worldDialogOpen} onOpenChange={setWorldDialogOpen} />
    </div>
  )
}
