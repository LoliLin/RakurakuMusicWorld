import { useEffect, useState } from 'react'
import { Outlet } from 'react-router-dom'
import { useTheme } from '@appica/ui-react/hooks/use-theme'
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
      <header className="border-border-muted bg-background/85 sticky top-0 z-40 border-b backdrop-blur">
        <div className="mx-auto flex h-14 w-full max-w-6xl items-center gap-2 px-4">
          <MainNav />
          <div className="ms-auto flex shrink-0 items-center gap-1.5 sm:gap-2">
            <button
              type="button"
              onClick={() => setWorldDialogOpen(true)}
              className="border-border-muted bg-background-subtle hover:bg-background-muted/80 flex h-8 items-center gap-1.5 rounded-full border px-2.5 text-xs transition-colors cursor-pointer"
              title="切换世界 / 玩家设置"
            >
              <span
                className={`inline-block size-2 shrink-0 rounded-full ${
                  isWsConnected ? 'bg-success animate-pulse' : 'bg-neutral-muted'
                }`}
              />
              <span className="text-foreground-intense font-medium max-w-[100px] truncate sm:max-w-[180px]">
                {station?.name || (isLocal ? '本地世界' : '电台')}
              </span>
              {auth?.role === 'admin' ? (
                <span className="text-primary text-[11px] font-bold" title="本机管理员 (OP)">
                  👑
                </span>
              ) : null}
            </button>
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
      <WorldSelectorDialog open={worldDialogOpen} onOpenChange={setWorldDialogOpen} />
    </div>
  )
}
