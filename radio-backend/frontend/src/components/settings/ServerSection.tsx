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
import { useStore } from '@/store'

interface TestResult {
  success: boolean
  message: string
  stationName?: string
  worldId?: string
}

export function ServerSection() {
  const currentStation = useStore((s) => s.station)
  const isWsConnected = useStore((s) => s.wsConnected)

  const [customUrl, setCustomUrl] = useState('')
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestResult | null>(null)

  useEffect(() => {
    setCustomUrl(getCustomServerUrl())
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
    // 刷新页面以完全重新初始化 WebSocket、音频流与状态单例
    window.location.reload()
  }

  const handleReset = () => {
    setCustomServerUrl(null)
    window.location.reload()
  }

  return (
    <section
      aria-labelledby="server-settings-title"
      className="border-border-muted bg-background-subtle rounded-xl border p-4 sm:p-5"
    >
      <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
        <h2
          id="server-settings-title"
          className="text-foreground-intense flex items-center gap-2 text-lg font-semibold"
        >
          <DeviceMobile className="text-primary size-5" aria-hidden="true" />
          服务器与世界连接
        </h2>
        <div className="flex items-center gap-2">
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

      <form onSubmit={handleSave} className="space-y-4">
        <Field>
          <FieldLabel htmlFor="server-url-input">目标服务器地址</FieldLabel>
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
            支持连接至远程 Dedicated Server 或局域网中的其它 RakurakuMusicWorld 实例。
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
    </section>
  )
}
