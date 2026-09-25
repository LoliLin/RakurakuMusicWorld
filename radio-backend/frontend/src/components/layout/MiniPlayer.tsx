import { useNavigate } from 'react-router-dom'
import { Button } from '@appica/ui-react/button'
import { Popover, PopoverTrigger, PopoverContent } from '@appica/ui-react/popover'
import { Progress } from '@appica/ui-react/progress'
import { Slider } from '@appica/ui-react/slider'
import { PlayerPause, PlayerPlay, PlayerSkipBack, PlayerSkipForward, Maximize, Volume2, VolumeOff } from '@appica/icons-react'
import { SongArtwork } from '@/components/SongArtwork'
import { useStore } from '@/store'
import { usePlaybackClock } from '@/hooks/usePlaybackClock'
import { formatTime } from '@/lib/format'
import { pauseAudio, resumeAudio, setAudioVolume } from '@/audio/streamAudio'
import { adminSkipNext, adminSkipPrev } from '@/api'

/** Docked bottom player bar: transport (play/pause + admin skip) + volume. */
export function MiniPlayer() {
  const playback = useStore((s) => s.playback)
  const volume = useStore((s) => s.volume)
  const isAdmin = useStore((s) => s.auth?.role === 'admin')
  const audioStatus = useStore((s) => s.audioStatus)
  const audioPaused = useStore((s) => s.audioPaused)
  const needsPlay = useStore((s) => s.needsPlay)
  const navigate = useNavigate()
  const position = usePlaybackClock(playback)
  const listening = !audioPaused && !needsPlay && (audioStatus === 'playing' || audioStatus === 'connecting' || audioStatus === 'reconnecting')

  const skip = async (dir: 'prev' | 'next') => {
    try {
      if (dir === 'prev') await adminSkipPrev()
      else await adminSkipNext()
    } catch (e) {
      useStore.getState().addToast(e instanceof Error ? e.message : '操作失败', 'error')
    }
  }

  if (!playback || !playback.title) return null

  const duration = playback.durationMs || 0
  const pct = duration > 0 ? Math.min(100, (position / duration) * 100) : 0
  const muted = volume <= 0

  return (
    <div className="border-border-muted bg-background/95 sticky bottom-0 z-40 border-t backdrop-blur">
      <div className="mx-auto flex h-16 w-full max-w-7xl items-center gap-2 px-4 sm:gap-3 sm:px-6">
        <SongArtwork
          hasCover={playback.coverUrl !== null}
          coverSrc={playback.coverUrl ?? undefined}
          size="sm"
        />
        <div className="min-w-0 flex-1">
          <p className="text-foreground-intense truncate text-sm font-medium">{playback.title}</p>
          <p className="text-foreground-muted truncate text-xs">{playback.artist || '未知艺术家'} · {audioStatus === 'playing' ? '直播中' : audioStatus === 'reconnecting' ? '正在重连' : audioStatus === 'connecting' ? '正在连接' : audioStatus === 'error' ? '连接异常' : '已暂停'}</p>
        </div>
        <div className="hidden w-44 items-center gap-2 sm:flex">
          <Progress value={pct} className="min-w-0 flex-1" aria-label="播放进度" />
          <span className="text-foreground-muted shrink-0 text-xs tabular-nums">{formatTime(position)}</span>
        </div>

        {/* 音量：Popover 滑条，点击图标静音/恢复 */}
        <Popover>
          <PopoverTrigger
            render={
              <Button
                variant="ghost"
                size="icon-md"
                className="min-h-11 min-w-11"
                aria-label="调整音量"
              />
            }
          >
            {muted ? <VolumeOff /> : <Volume2 />}
          </PopoverTrigger>
          <PopoverContent side="top" align="center" className="w-48">
            <div className="flex items-center gap-3 px-1 py-1">
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={muted ? '取消静音' : '静音'}
                onClick={() => {
                  const next = muted ? 0.6 : 0
                  useStore.getState().setVolume(next)
                  setAudioVolume(next)
                }}
              >
                {muted ? <VolumeOff /> : <Volume2 />}
              </Button>
              <Slider
                value={volume}
                min={0}
                max={1}
                step={0.05}
                onValueChange={(v) => {
                  const next = Array.isArray(v) ? v[0] : v
                  useStore.getState().setVolume(next)
                  setAudioVolume(next)
                }}
                className="min-w-0 flex-1"
                aria-label="音量"
              />
              <span className="text-foreground-muted w-8 shrink-0 text-right text-xs tabular-nums">
                {Math.round(volume * 100)}%
              </span>
            </div>
          </PopoverContent>
        </Popover>

        {isAdmin && (
          <Button
            variant="ghost"
            size="icon-md"
            className="hidden sm:inline-flex"
            aria-label="上一首"
            onClick={() => void skip('prev')}
          >
            <PlayerSkipBack />
          </Button>
        )}
        <Button
          variant="outline"
          size="icon-md"
          className="min-h-11 min-w-11"
          aria-label={listening ? '暂停收听' : '继续收听'}
          onClick={() => (listening ? pauseAudio() : resumeAudio())}
        >
          {listening ? <PlayerPause /> : <PlayerPlay />}
        </Button>
        {isAdmin && (
          <Button
            variant="ghost"
            size="icon-md"
            className="hidden sm:inline-flex"
            aria-label="下一首"
            onClick={() => void skip('next')}
          >
            <PlayerSkipForward />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon-md"
          className="hidden sm:inline-flex"
          aria-label="打开播放器"
          onClick={() => navigate('/player')}
        >
          <Maximize />
        </Button>
      </div>
    </div>
  )
}
