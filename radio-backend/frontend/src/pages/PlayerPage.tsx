import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { Carousel, CarouselContent, CarouselSlide, CarouselPagination } from '@appica/ui-react/carousel'
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from '@appica/ui-react/dropdown-menu'
import { ScrollArea } from '@appica/ui-react/scroll-area'
import { Avatar, AvatarFallback } from '@appica/ui-react/avatar'
import { ChevronDown, Headphones, Playlist } from '@appica/icons-react'
import { SongArtwork } from '@/components/SongArtwork'
import { useStore, type Playback } from '@/store'
import { usePlaybackClock } from '@/hooks/usePlaybackClock'
import { LyricsPanel } from '@/components/player/LyricsPanel'
import { QueueList } from '@/components/queue/QueueList'

/**
 * 播放器主栏：封面 banner + 歌名，下方直接是歌词。
 * 播放中封面呈现为旋转的圆形唱片；暂停/停止时静止；无歌时方形图标。
 */
function PlayerPane({ playback, position }: { playback: Playback | null; position: number }) {
  const station = useStore((s) => s.station)
  const onAir = playback !== null && playback.title.length > 0
  const artwork = onAir ? (playback?.coverUrl ?? undefined) : (station?.icon_url ?? undefined)

  return (
    <div className="flex min-w-0 flex-col items-center">
      <div className="relative">
        <SongArtwork
          hasCover={artwork !== undefined}
          coverSrc={artwork}
          alt={onAir ? `${playback.title} 封面` : station?.name ? `${station.name} 图标` : ''}
          size="2xl"
          shape="rounded"
          className="size-52 sm:size-60 lg:size-72"
        />
      </div>

      <div className="mt-5 w-full min-w-0 text-center">
        <h1
          className="text-foreground-intense truncate text-3xl font-bold sm:text-4xl"
          title={onAir ? playback.title : station?.name}
        >
          {onAir ? playback.title : station?.name ?? 'RakurakuMusicWorld'}
        </h1>
        <p className="text-foreground-muted mt-1 truncate text-base">
          {onAir ? playback.artist || '\u00a0' : station?.subtitle || '\u00a0'}
        </p>
      </div>

      {/* 歌词占据原控制栏的位置 */}
      <div className="mt-8 w-full max-w-xl">
        <ScrollArea className="max-h-[45vh] min-h-0 w-full" scrollShadow scrollbarVisibility="auto">
          <LyricsPanel playback={playback} positionMs={position} className="max-h-none overflow-visible" />
        </ScrollArea>
      </div>
    </div>
  )
}

/**
 * 右栏：正在收听（紧凑指示 + Dropdown 展开名单）+ 点歌队列。
 */
function SidebarPane({ count, names, displayName }: { count: number; names: string[]; displayName: string }) {
  const others = names.filter((name) => name !== displayName)
  const queueCount = useStore((s) => s.queue.length)

  return (
    <aside aria-label="侧栏" className="min-w-0 xl:border-border-muted xl:border-l xl:ps-8">
      {/* 正在收听：一行轻量指示，Dropdown 展开在线名单 */}
      <div className="mb-5 flex items-center gap-2">
        <Headphones className="text-foreground-muted size-4 shrink-0" aria-hidden="true" />
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button variant="ghost" size="sm" className="-mx-1 px-1">
                {count > 0 ? `${count} 人在听` : '暂无人在听'}
                <ChevronDown data-icon="end" className="size-3.5" />
              </Button>
            }
          />
          <DropdownMenuContent align="start" className="w-56">
            {others.length === 0 && (
              <DropdownMenuItem disabled>还没有其他听众</DropdownMenuItem>
            )}
            {others.map((name, i) => (
              <DropdownMenuItem key={`${name}-${i}`} disabled className="gap-2.5">
                <Avatar size="sm">
                  <AvatarFallback>{name.trim().charAt(0).toUpperCase() || '?'}</AvatarFallback>
                </Avatar>
                <span className="min-w-0 flex-1 truncate">{name}</span>
                <span className="text-success flex shrink-0 items-center gap-1 text-xs">
                  <span aria-hidden="true" className="size-1.5 rounded-full bg-current" />
                  在线
                </span>
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem disabled className="gap-2.5">
              <Avatar size="sm">
                <AvatarFallback className="bg-primary text-primary-foreground">
                  {displayName.trim().charAt(0).toUpperCase() || '我'}
                </AvatarFallback>
              </Avatar>
              <span className="min-w-0 flex-1 truncate">{displayName || '我'}</span>
              <span className="text-foreground-muted shrink-0 text-xs">当前设备</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        {displayName && <span className="text-foreground-subtle ms-auto truncate text-xs">{displayName}</span>}
      </div>

      {/* 点歌队列 */}
      <div className="mb-3 flex items-center gap-2">
        <Playlist className="text-foreground-muted size-4" aria-hidden="true" />
        <h2 className="text-foreground-intense text-sm font-semibold">点歌队列</h2>
        <Badge variant="soft" size="xs" aria-label={`队列中共 ${queueCount} 首`}>
          {queueCount}
        </Badge>
      </div>
      <QueueList />
    </aside>
  )
}

/**
 * 首页：封面唱片 + 歌词为主角；「正在收听」与「点歌队列」收进右栏。
 */
export default function PlayerPage() {
  const playback = useStore((s) => s.playback)
  const names = useStore((s) => s.listeners.names)
  const count = useStore((s) => s.listeners.count)
  const displayName = useStore((s) => s.auth?.display_name ?? '')
  const position = usePlaybackClock(playback)

  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-4 sm:px-6">
      {/* 移动端（<lg）：播放器（含歌词）/ 侧栏 两卡滑动 */}
      <div className="lg:hidden">
        <Carousel align="start" slidesToScroll={1}>
          <CarouselContent>
            <CarouselSlide className="basis-full">
              <PlayerPane playback={playback} position={position} />
            </CarouselSlide>
            <CarouselSlide className="basis-full">
              <SidebarPane count={count} names={names} displayName={displayName} />
            </CarouselSlide>
          </CarouselContent>
          <CarouselPagination className="mt-4 justify-center" />
        </Carousel>
      </div>

      {/* 桌面（lg+）：主栏 + xl 右栏（正在收听 / 点歌队列） */}
      <div className="hidden gap-10 lg:block xl:grid xl:grid-cols-[minmax(0,1fr)_320px] xl:gap-12">
        <PlayerPane playback={playback} position={position} />
        <SidebarPane count={count} names={names} displayName={displayName} />
      </div>
    </div>
  )
}
