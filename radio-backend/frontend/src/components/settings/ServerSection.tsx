import { useState, useEffect, type FormEvent } from 'react'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { Field, FieldDescription, FieldLabel } from '@appica/ui-react/field'
import { Input } from '@appica/ui-react/input'
import { Spinner } from '@appica/ui-react/spinner'
import {
  Alert,
  AlertDescription,
  AlertIcon,
  AlertTitle,
} from '@appica/ui-react/alert'
import {
  Check,
  DeviceMobile,
  X,
} from '@appica/icons-react'
import { appRoot, getCustomServerUrl, setCustomServerUrl } from '@/api/client'
import { fetchDiscoveredWorlds, scanLanWorlds } from '@/api'
import type { DiscoveredWorld } from '@/types'
import { useStore } from '@/store'
import { WorldSelectorDialog } from '@/components/layout/WorldSelectorDialog'

interface TestResult {
  success: boolean
  message: string
  stationName?: string
  worldId?: string
}

export function ServerSection() {
  const currentStation = useStore((s) => s.station)
  const isWsConnected = useStore((s) => s.wsConnected)
  const [worldSelectorOpen, setWorldSelectorOpen] = useState(false)

  const [customUrl, setCustomUrl] = useState('')
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestResult | null>(null)

  const [lanWorlds, setLanWorlds] = useState<DiscoveredWorld[]>([])
  const [scanning, setScanning] = useState(false)

  const loadLanWorlds = async () => {
    try {
      const worlds = await fetchDiscoveredWorlds()
      setLanWorlds(worlds)
    } catch {
      // ignore discovery load errors when offline
    }
  }

  const handleScan = async () => {
    setScanning(true)
    try {
      await scanLanWorlds()
      await new Promise((resolve) => setTimeout(resolve, 800))
      await loadLanWorlds()
    } catch {
      // ignore
    } finally {
      setScanning(false)
    }
  }

  useEffect(() => {
    setCustomUrl(getCustomServerUrl())
    void loadLanWorlds()
  }, [])

  const currentRoot = appRoot()
  const isCustom = Boolean(getCustomServerUrl())

  const handleTest = async () => {
    const target = customUrl.trim().replace(/\/+$/, '')
    if (!target) {
      setTestResult({
        success: false,
        message: '请输入服务器地址',
      })
      return
    }

    setTesting(true)
    setTestResult(null)

    try {
      const res = await fetch(`${target}/api/station`, {
        method: 'GET',
        headers: { Accept: 'application/json' },
      })
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`)
      }
      const data = await res.json()
      setTestResult({
        success: true,
        message: '连接成功！',
        stationName: data.name || data.station_name,
        worldId: data.world_id,
      })
    } catch (err) {
      setTestResult({
        success: false,
        message: `无法连接到目标服务器: ${err instanceof Error ? err.message : '网络错误'}`,
      })
    } finally {
      setTesting(false)
    }
  }

  const handleSave = (e: FormEvent) => {
    e.preventDefault()
    const target = customUrl.trim().replace(/\/+$/, '')
    if (target) {
      setCustomServerUrl(target)
    } else {
      setCustomServerUrl(null)
    }
    window.location.reload()
  }

  const handleConnectTo = (url: string) => {
    setCustomServerUrl(url)
    window.location.reload()
  }

  const handleReset = () => {
    setCustomServerUrl(null)
    window.location.reload()
  }

  return (
    <section
      aria-labelledby="server-settings-title"
      className="app-panel p-4 sm:p-6"
    >
      <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
        <h2
          id="server-settings-title"
          className="app-section-title flex items-center gap-2"
        >
          <DeviceMobile className="text-primary size-5" aria-hidden="true" />
          服务器与世界连接
        </h2>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="soft"
            size="sm"
            onClick={() => setWorldSelectorOpen(true)}
          >
            打开世界选择器
          </Button>
          <Badge color={isCustom ? 'warning' : 'primary'}>
            {isCustom ? '远程 World' : '本地 / 默认'}
          </Badge>
          <Badge color={isWsConnected ? 'success' : 'neutral'}>
            {isWsConnected ? '在线同步中' : '离线 / 轮询'}
          </Badge>
        </div>
      </div>

      <div className="mb-5 space-y-2 text-sm">
        <div className="flex flex-wrap items-center gap-x-2">
          <span className="text-foreground-muted">当前连接根地址:</span>
          <code className="bg-background-muted text-foreground-intense rounded px-1.5 py-0.5 font-mono text-xs">
            {currentRoot}
          </code>
        </div>
        {currentStation?.world_id && (
          <div className="flex flex-wrap items-center gap-x-2">
            <span className="text-foreground-muted">当前 World ID:</span>
            <code className="bg-background-muted text-foreground-intense rounded px-1.5 py-0.5 font-mono text-xs">
              {currentStation.world_id}
            </code>
          </div>
        )}
      </div>

      {/* World Browser / LAN Discovery */}
      <div className="border-border-muted bg-background-muted/40 mb-6 rounded-lg border p-3.5 sm:p-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-foreground-intense text-sm font-semibold">
              局域网世界浏览器 (World Browser)
            </h3>
            <p className="text-foreground-muted text-xs">
              基于 UDP 广播自动发现同局域网中的其它电台实例
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            onClick={handleScan}
            disabled={scanning}
          >
            {scanning ? <Spinner className="size-4" currentColor /> : '扫描局域网'}
          </Button>
        </div>

        {lanWorlds.length === 0 ? (
          <div className="text-foreground-muted py-3 text-center text-xs">
            暂未发现局域网其它电台。同网络内开启的 RakurakuMusicWorld 节点将自动列在此处。
          </div>
        ) : (
          <div className="grid gap-2.5 sm:grid-cols-2">
            {lanWorlds.map((world) => {
              const isCurrent = world.world_id === currentStation?.world_id
              return (
                <div
                  key={world.world_id}
                  className="border-border-muted bg-background flex flex-col justify-between rounded-lg border p-3 text-xs"
                >
                  <div className="space-y-1">
                    <div className="flex items-center justify-between gap-1">
                      <span className="text-foreground-intense font-medium">
                        {world.station_name}
                      </span>
                      <div className="flex items-center gap-1">
                        {world.is_headless && (
                          <Badge color="neutral">Headless</Badge>
                        )}
                        <Badge color="primary">v{world.version}</Badge>
                      </div>
                    </div>
                    <div className="text-foreground-muted font-mono">
                      {world.host}:{world.port}
                    </div>
                    <div className="text-foreground-muted truncate text-[11px]">
                      ID: {world.world_id}
                    </div>
                  </div>
                  <div className="mt-2.5 pt-2 border-t border-border-muted/50 flex items-center justify-end">
                    {isCurrent ? (
                      <Badge color="success">当前已连接</Badge>
                    ) : (
                      <Button
                        type="button"
                        variant="outline"
                        onClick={() => handleConnectTo(world.url)}
                      >
                        连接到此世界
                      </Button>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      <form onSubmit={handleSave} className="space-y-4">
        <Field>
          <FieldLabel htmlFor="server-url-input">手动配置服务器地址</FieldLabel>
          <div className="mt-1 flex flex-col gap-2 sm:flex-row">
            <Input
              id="server-url-input"
              type="url"
              value={customUrl}
              onChange={(e) => {
                setCustomUrl(e.target.value)
                setTestResult(null)
              }}
              placeholder="留空为默认同源，或输入 http://192.168.1.100:2241"
              className="flex-1"
            />
            <Button
              type="button"
              variant="outline"
              onClick={handleTest}
              disabled={testing || !customUrl.trim()}
            >
              {testing ? <Spinner className="size-4" currentColor /> : '测试连接'}
            </Button>
          </div>
          <FieldDescription>
            支持连接至远程 Dedicated Server 或通过公网/反代访问的 RakurakuMusicWorld 实例。
          </FieldDescription>
        </Field>

        {testResult && (
          <Alert color={testResult.success ? 'success' : 'danger'}>
            <AlertIcon>
              {testResult.success ? <Check className="size-4" /> : <X className="size-4" />}
            </AlertIcon>
            <div>
              <AlertTitle>{testResult.message}</AlertTitle>
              {testResult.success && (
                <AlertDescription className="mt-1 space-y-0.5 text-xs">
                  {testResult.stationName && <div>电台名称: {testResult.stationName}</div>}
                  {testResult.worldId && <div>World ID: {testResult.worldId}</div>}
                </AlertDescription>
              )}
            </div>
          </Alert>
        )}

        <div className="flex flex-wrap gap-2 pt-2">
          <Button type="submit">
            保存并连接
          </Button>
          {isCustom && (
            <Button type="button" variant="ghost" onClick={handleReset}>
              恢复本地默认
            </Button>
          )}
        </div>
      </form>

      <WorldSelectorDialog open={worldSelectorOpen} onOpenChange={setWorldSelectorOpen} />
    </section>
  )
}
