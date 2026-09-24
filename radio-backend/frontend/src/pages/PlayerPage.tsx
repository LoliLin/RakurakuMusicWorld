import { useState } from 'react'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from '@appica/ui-react/dropdown-menu'
import { Avatar, AvatarFallback } from '@appica/ui-react/avatar'
import { ChevronDown, Headphones, Playlist } from '@appica/icons-react'
import { SongArtwork } from '@/components/SongArtwork'
import { useStore, type Playback } from '@/store'
import { usePlaybackClock } from '@/hooks/usePlaybackClock'
import { LyricsPanel } from '@/components/player/LyricsPanel'
import { QueueList } from '@/components/queue/QueueList'

type MobilePane = 'player' | 'queue'

function ListeningStatus({ playback }: { playback: Playback | null }) {
  const audioStatus = useStore((s) => s.audioStatus)
  if (!playback?.title) return <span className="text-foreground-muted text-sm">等待电台开始播放</span>
  const label = {
    idle: '等待音频', connecting: '正在连接', playing: '直播中',
    paused: '已暂停收听', reconnecting: '正在重连', error: '连接不稳定',
  }[audioStatus]
  return <span className="border-border-muted bg-primary-subtle text-primary inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold" role="status"><span className="bg-primary size-1.5 rounded-full" aria-hidden="true" />{label}</span>
}

function PlayerPane({ playback, position }: { playback: Playback | null; position: number }) {
  const station = useStore((s) => s.station)
  const onAir = playback !== null && playback.title.length > 0
  const artwork = onAir ? (playback?.coverUrl ?? undefined) : (station?.icon_url ?? undefined)
  return (
    <section aria-label="正在播放" className="grid min-w-0 items-start gap-8 md:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] xl:gap-10">
      <div className="flex min-w-0 flex-col items-center text-center">
        <SongArtwork hasCover={artwork !== undefined} coverSrc={artwork} alt={onAir ? `${playback.title} 封面` : station?.name ? `${station.name} 图标` : ''} size="2xl" shape="rounded" className="size-56 overflow-hidden rounded-xl sm:size-64 xl:size-72" />
        <div className="mt-6 w-full min-w-0">
          <h2 className="app-page-title break-words" title={onAir ? playback.title : station?.name}>{onAir ? playback.title : station?.name ?? 'RakurakuMusicWorld'}</h2>
          <p className="text-foreground-muted mt-2 truncate text-sm sm:text-base">{onAir ? playback.artist || '未知艺术家' : station?.subtitle || '一起听音乐'}</p>
        </div>
        <div className="mt-5"><ListeningStatus playback={playback} /></div>
      </div>
      <div className="app-panel flex min-h-[280px] min-w-0 flex-col bg-background p-4 sm:p-6">
        <div className="mb-3 flex items-center justify-between gap-3"><h2 className="app-section-title">同步歌词</h2><span className="text-foreground-muted text-xs">随音乐滚动</span></div>
        <LyricsPanel playback={playback} positionMs={position} className="max-h-[48vh] min-h-[220px]" />
      </div>
    </section>
  )
}

function SidebarPane({ count, names, displayName }: { count: number; names: string[]; displayName: string }) {
  const others = names.filter((name) => name !== displayName)
  const queueCount = useStore((s) => s.queue.length)
  return (
    <aside aria-label="点歌队列与听众" className="min-w-0 space-y-4">
      <div className="app-panel flex items-center gap-2 px-4 py-3">
        <Headphones className="text-primary size-4 shrink-0" aria-hidden="true" />
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="ghost" size="sm" className="-mx-2 px-2" />}>
            {count > 0 ? `正在收听 ${count} 人` : '暂无人在听'}<ChevronDown data-icon="end" className="size-3.5" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-56">
            {others.length === 0 && <DropdownMenuItem disabled>还没有其他听众</DropdownMenuItem>}
            {others.map((name, i) => <DropdownMenuItem key={`${name}-${i}`} disabled className="gap-2.5"><Avatar size="sm"><AvatarFallback>{name.trim().charAt(0).toUpperCase() || '?'}</AvatarFallback></Avatar><span className="min-w-0 flex-1 truncate">{name}</span></DropdownMenuItem>)}
            {displayName && <DropdownMenuItem disabled>当前设备：{displayName}</DropdownMenuItem>}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="app-panel bg-background p-4">
        <div className="mb-4 flex items-center gap-2"><Playlist className="text-primary size-4" aria-hidden="true" /><h2 className="app-section-title">点歌队列</h2><Badge variant="soft" size="xs" aria-label={`队列中共 ${queueCount} 首`}>{queueCount}</Badge></div>
        <QueueList />
      </div>
    </aside>
  )
}

export default function PlayerPage() {
  const playback = useStore((s) => s.playback)
  const names = useStore((s) => s.listeners.names)
  const count = useStore((s) => s.listeners.count)
  const queueCount = useStore((s) => s.queue.length)
  const displayName = useStore((s) => s.auth?.display_name ?? '')
  const position = usePlaybackClock(playback)
  const [mobilePane, setMobilePane] = useState<MobilePane>('player')
  return (
    <div className="app-page">
      <div className="mb-5 flex items-end justify-between gap-3"><div><p className="app-eyebrow mb-1">RAKURAKU RADIO</p><h1 className="app-page-title">一起听</h1></div><p className="text-foreground-muted hidden text-sm sm:block">同一首歌，同一个音乐世界</p></div>
      <div className="border-border-muted mb-5 grid grid-cols-2 rounded-lg border p-1 lg:hidden" aria-label="播放器内容切换">
        <button type="button" aria-pressed={mobilePane === 'player'} aria-controls="player-pane" onClick={() => setMobilePane('player')} className={`app-interactive min-h-11 rounded-md px-3 text-sm font-semibold ${mobilePane === 'player' ? 'bg-primary text-primary-foreground' : 'text-foreground-muted'}`}>正在播放</button>
        <button type="button" aria-pressed={mobilePane === 'queue'} aria-controls="queue-pane" onClick={() => setMobilePane('queue')} className={`app-interactive min-h-11 rounded-md px-3 text-sm font-semibold ${mobilePane === 'queue' ? 'bg-primary text-primary-foreground' : 'text-foreground-muted'}`}>点歌队列 {queueCount}</button>
      </div>
      <div className="grid min-w-0 gap-5 lg:grid-cols-[minmax(0,1fr)_300px] xl:grid-cols-[minmax(0,1fr)_320px] xl:gap-8">
        <div id="player-pane" className={mobilePane === 'player' ? 'min-w-0' : 'hidden min-w-0 lg:block'}><PlayerPane playback={playback} position={position} /></div>
        <div id="queue-pane" className={mobilePane === 'queue' ? 'min-w-0' : 'hidden min-w-0 lg:block'}><SidebarPane count={count} names={names} displayName={displayName} /></div>
      </div>
    </div>
  )
}
