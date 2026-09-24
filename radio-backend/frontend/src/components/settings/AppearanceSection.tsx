import { useEffect, useState } from 'react'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@appica/ui-react/collapsible'
import { useTheme } from '@appica/ui-react/hooks/use-theme'
import { Input } from '@appica/ui-react/input'
import { Separator } from '@appica/ui-react/separator'
import { Toggle } from '@appica/ui-react/toggle'
import { ToggleGroup } from '@appica/ui-react/toggle-group'
import { Bell, Check, ChevronDown, DeviceDesktop, MoonStars, Palette, SunHigh } from '@appica/icons-react'
import { useStore } from '@/store'
import { ACCENT_SEEDS, dynamicAccent } from '@/lib/accents'
import { cn } from '@/lib/cn'

const THEME_OPTIONS = [
  { value: 'light', label: '浅色', icon: SunHigh },
  { value: 'dark', label: '深色', icon: MoonStars },
  { value: 'system', label: '跟随系统', icon: DeviceDesktop },
] as const

type NotificationPermissionState = 'granted' | 'denied' | 'default' | 'unsupported' | 'unknown'

const PERMISSION_TEXT: Record<NotificationPermissionState, string> = {
  granted: '已开启',
  denied: '已拒绝',
  default: '未开启',
  unsupported: '浏览器不支持',
  unknown: '检查中…',
}

/** 桌面通知：切歌时标签页隐藏会自动发送，这里只负责申请权限。 */
function NotificationBlock() {
  const [permission, setPermission] = useState<NotificationPermissionState>('unknown')

  useEffect(() => {
    if (!('Notification' in window)) {
      setPermission('unsupported')
      return
    }
    setPermission(Notification.permission)
  }, [])

  const request = async () => {
    if (!('Notification' in window)) {
      useStore.getState().addToast('当前浏览器不支持桌面通知', 'warning')
      return
    }
    try {
      const result = await Notification.requestPermission()
      setPermission(result)
      if (result === 'granted') useStore.getState().addToast('桌面通知已开启', 'success')
      else if (result === 'denied') useStore.getState().addToast('通知权限被拒绝，请在浏览器设置中允许', 'warning')
      else useStore.getState().addToast('未授予通知权限', 'info')
    } catch {
      useStore.getState().addToast('请求通知权限失败', 'error')
    }
  }

  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <p className="text-foreground-intense flex items-center gap-1.5 text-sm font-medium">
          <Bell className="size-4 shrink-0" />
          <span className="min-w-0 truncate">桌面通知</span>
          <Badge variant={permission === 'granted' ? 'success' : 'soft'} size="xs" className="shrink-0">
            {PERMISSION_TEXT[permission]}
          </Badge>
        </p>
        <p className="text-foreground-muted mt-1 text-xs">
          切换歌曲时，若标签页处于隐藏状态，会自动发送桌面通知。
        </p>
      </div>
      <Button
        variant="outline"
        size="sm"
        onClick={request}
        disabled={permission === 'granted' || permission === 'unsupported' || permission === 'unknown'}
        className="shrink-0"
      >
        开启桌面通知
      </Button>
    </div>
  )
}

export function AppearanceSection() {
  const { theme, setTheme } = useTheme()
  const accent = useStore((s) => s.accent)
  const current = (theme ?? 'system') as string

  const isActive = (seed: string) => accent.seed.toLowerCase() === seed.toLowerCase()

  const pickSeed = (seed: string) => useStore.getState().setAccent(dynamicAccent(seed))

  return (
    <section aria-labelledby="settings-appearance-heading" className="app-panel p-4 sm:p-6">
      <h2 id="settings-appearance-heading" className="app-section-title mb-4 flex items-center gap-2">
        <Palette data-icon="start" />
        外观
      </h2>

      <div className="flex flex-col gap-5">
        <div className="flex flex-col gap-2">
          <h3 className="text-foreground-intense text-sm font-medium">主题</h3>
          <ToggleGroup
            aria-label="主题模式"
            value={[current]}
            onValueChange={(values) => {
              if (values.length > 0) setTheme(values[0])
            }}
            className="w-full rounded-lg border border-border-muted bg-background p-1"
          >
            {THEME_OPTIONS.map(({ value, label, icon: Icon }) => (
              <Toggle
                key={value}
                value={value}
                className={cn(
                  'flex h-9 min-w-0 flex-1 items-center justify-center gap-1.5 rounded-md px-2 text-sm font-medium transition',
                  'text-foreground-muted not-data-pressed:hover:bg-background-muted hover:text-foreground-intense',
                  'data-pressed:bg-primary data-pressed:text-primary-foreground',
                  'focus-visible:outline-ring',
                )}
              >
                <Icon data-icon="start" className="shrink-0" />
                <span className="min-w-0 truncate">{label}</span>
              </Toggle>
            ))}
          </ToggleGroup>
        </div>

        <Collapsible className="flex flex-col gap-3">
          <CollapsibleTrigger className="group flex w-full items-center gap-1.5 text-start">
            <h3 className="text-foreground-intense text-sm font-medium">主题色</h3>
            <ChevronDown
              className="text-foreground-muted size-4 transition-transform duration-200 group-data-panel-open:rotate-180 motion-reduce:transition-none"
              aria-hidden="true"
            />
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="flex flex-col gap-3">
              <div className="flex flex-wrap items-center gap-2.5">
            {ACCENT_SEEDS.map(({ seed, name }) => {
              const pair = dynamicAccent(seed)
              const active = isActive(seed)
              return (
                <button
                  key={seed}
                  type="button"
                  aria-label={`主题色 ${name}`}
                  aria-pressed={active}
                  title={`${name}（浅 ${pair.light} / 深 ${pair.dark}）`}
                  onClick={() => pickSeed(seed)}
                  className={cn(
                    'flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-full border border-black/10 transition',
                    'focus-visible:outline-ring',
                    active && 'ring-2 ring-offset-2 ring-primary',
                  )}
                  style={{
                    background: `linear-gradient(135deg, ${pair.light} 50%, ${pair.dark} 50%)`,
                  }}
                >
                  {active && <Check className="size-4 text-white drop-shadow" />}
                </button>
              )
            })}
            <div className="flex min-w-0 items-center gap-2">
              <Input
                type="color"
                value={accent.seed}
                onChange={(e) => pickSeed(e.target.value)}
                aria-label="自定义主题色"
                className="size-8 shrink-0 cursor-pointer border-0 bg-transparent p-1"
              />
              <code className="text-foreground-muted min-w-0 truncate font-mono text-xs">{accent.seed.toUpperCase()}</code>
            </div>
          </div>

          <div className="flex flex-wrap gap-3">
            <div className="rounded-xl bg-primary p-4 text-primary-foreground shadow-sm">
              <p className="text-sm font-semibold">浅色模式</p>
              <p className="mt-0.5 font-mono text-xs opacity-80">{accent.light.toUpperCase()}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <span className="rounded-md bg-primary-foreground/15 px-3 py-1 text-xs">标签</span>
                <span className="rounded-md bg-primary-foreground px-3 py-1 text-xs text-primary">主按钮</span>
              </div>
            </div>
            <div className="rounded-xl bg-primary p-4 text-primary-foreground shadow-sm">
              <p className="text-sm font-semibold">深色模式</p>
              <p className="mt-0.5 font-mono text-xs opacity-80">{accent.dark.toUpperCase()}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <span className="rounded-md bg-primary-foreground/15 px-3 py-1 text-xs">标签</span>
                <span className="rounded-md bg-primary-foreground px-3 py-1 text-xs text-primary">主按钮</span>
              </div>
            </div>
          </div>
          </div>
        </CollapsibleContent>
      </Collapsible>

        <Separator />

        <NotificationBlock />
      </div>
    </section>
  )
}
