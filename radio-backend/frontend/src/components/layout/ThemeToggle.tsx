import { Button } from '@appica/ui-react/button'
import { useTheme } from '@appica/ui-react/hooks/use-theme'
import { SunHigh, MoonStars, DeviceDesktop } from '@appica/icons-react'

const ORDER = ['light', 'dark', 'system'] as const

export function ThemeToggle({ resolvedTheme }: { resolvedTheme: string | undefined }) {
  const { theme, setTheme } = useTheme()
  const next = ORDER[(ORDER.indexOf((theme ?? 'system') as (typeof ORDER)[number]) + 1) % ORDER.length]

  return (
    <Button
      variant="ghost"
      size="icon-md"
      className="min-h-11 min-w-11"
      aria-label={`切换主题（当前：${theme ?? 'system'}，下一个：${next}）`}
      onClick={() => setTheme(next)}
    >
      {resolvedTheme === 'dark' ? <MoonStars /> : theme === 'light' ? <SunHigh /> : <DeviceDesktop />}
    </Button>
  )
}
