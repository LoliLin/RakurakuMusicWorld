import { useState, useEffect, type FormEvent } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogClose,
} from '@appica/ui-react/dialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@appica/ui-react/tabs'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
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
  CrownFilled,
  Globe,
  Home,
  Pencil,
  Refresh,
  Server,
  Trash,
  UserFilled,
  Wifi,
  X,
} from '@appica/icons-react'
import {
  getCustomServerUrl,
  setCustomServerUrl,
  isLocalWorld,
  getRecentWorlds,
  removeRecentWorld,
} from '@/api/client'
import {
  fetchDiscoveredWorlds,
  scanLanWorlds,
  fetchMe,
  setDisplayName,
} from '@/api'
import type { DiscoveredWorld } from '@/types'
import { useStore } from '@/store'

interface WorldSelectorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface TestResult {
  success: boolean
  message: string
  stationName?: string
  worldId?: string
}

export function WorldSelectorDialog({ open, onOpenChange }: WorldSelectorDialogProps) {
  const currentStation = useStore((s) => s.station)
  const isWsConnected = useStore((s) => s.wsConnected)
  const auth = useStore((s) => s.auth)
  const addToast = useStore((s) => s.addToast)

  // 玩家昵称快速修改
  const [editingName, setEditingName] = useState(false)
  const [playerName, setPlayerName] = useState(auth?.display_name ?? '')
  const [savingName, setSavingName] = useState(false)

  // 局域网世界扫描
  const [lanWorlds, setLanWorlds] = useState<DiscoveredWorld[]>([])
  const [scanning, setScanning] = useState(false)

  // 远程直连
  const [customUrl, setCustomUrl] = useState('')
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestResult | null>(null)
  const [recentList, setRecentList] = useState<string[]>([])

  const isCurrentLocal = isLocalWorld()
  const isAdmin = auth?.role === 'admin'

  const refreshLanWorlds = async () => {
    try {
      const worlds = await fetchDiscoveredWorlds()
      setLanWorlds(worlds)
    } catch {
      // offline or unreachable
    }
  }

  const handleScanLan = async () => {
    setScanning(true)
    try {
      await scanLanWorlds()
      await new Promise((r) => setTimeout(r, 800))
      await refreshLanWorlds()
      addToast('局域网扫描已更新', 'info')
    } catch {
      addToast('扫描失败，请检查网络', 'error')
    } finally {
      setScanning(false)
    }
  }

  useEffect(() => {
    if (open) {
      setPlayerName(auth?.display_name ?? '')
      setCustomUrl(getCustomServerUrl())
      setRecentList(getRecentWorlds())
      void refreshLanWorlds()
    }
  }, [open, auth?.display_name])

  const handleSaveName = async (e: FormEvent) => {
    e.preventDefault()
    const trimmed = playerName.trim()
    if (!trimmed || savingName) return
    setSavingName(true)
    try {
      await setDisplayName(trimmed)
      const me = await fetchMe()
      useStore.getState().setAuth(me)
      setEditingName(false)
      addToast(`昵称已更新为「${trimmed}」`, 'success')
    } catch (err) {
      addToast(err instanceof Error ? err.message : '修改昵称失败', 'error')
    } finally {
      setSavingName(false)
    }
  }

  const handleConnectTo = (url: string | null) => {
    setCustomServerUrl(url)
    window.location.reload()
  }

  const handleTestRemote = async () => {
    const target = customUrl.trim().replace(/\/+$/, '')
    if (!target) {
      setTestResult({ success: false, message: '请输入服务器地址' })
      return
    }

    setTesting(true)
    setTestResult(null)

    try {
      const res = await fetch(`${target}/api/station`, {
        method: 'GET',
        headers: { Accept: 'application/json' },
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      setTestResult({
        success: true,
        message: '连接成功！目标世界运行正常',
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

  const handleRemoveRecent = (url: string, e: React.MouseEvent) => {
    e.stopPropagation()
    removeRecentWorld(url)
    setRecentList(getRecentWorlds())
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <div className="flex items-center justify-between gap-2">
            <DialogTitle className="flex items-center gap-2 text-lg">
              <Server className="text-primary size-5" />
              世界选择与玩家设置
            </DialogTitle>
            <div className="flex items-center gap-1.5">
              <span
                className={`inline-block size-2 rounded-full ${
                  isWsConnected ? 'bg-success animate-pulse' : 'bg-neutral-muted'
                }`}
              />
              <span className="text-foreground-muted text-xs">
                {isWsConnected ? '已联机同步' : '未连接 / 离线'}
              </span>
            </div>
          </div>
          <DialogDescription>
            选择本地单人世界，或无缝切换至局域网 / 远程多人电台世界。
          </DialogDescription>
        </DialogHeader>

        <DialogBody className="space-y-4">
          {/* 玩家身份区 (Minecraft GamerTag 风格) */}
          <div className="border-border-muted bg-background-subtle flex flex-wrap items-center justify-between gap-3 rounded-xl border p-3 sm:px-4">
            <div className="flex min-w-0 flex-1 items-center gap-3">
              <div
                className={`flex size-10 shrink-0 items-center justify-center rounded-full ${
                  isAdmin ? 'bg-primary/15 text-primary' : 'bg-background-muted text-foreground-muted'
                }`}
              >
                {isAdmin ? <CrownFilled className="size-5" /> : <UserFilled className="size-5" />}
              </div>

              {editingName ? (
                <form onSubmit={handleSaveName} className="flex min-w-0 flex-1 items-center gap-1.5">
                  <Input
                    value={playerName}
                    onChange={(e) => setPlayerName(e.target.value)}
                    maxLength={32}
                    autoFocus
                    placeholder="玩家昵称"
                    className="h-8 text-sm"
                  />
                  <Button
                    type="submit"
                    variant="soft"
                    size="icon-sm"
                    disabled={!playerName.trim() || savingName}
                    aria-label="保存昵称"
                  >
                    {savingName ? <Spinner currentColor /> : <Check />}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={() => {
                      setPlayerName(auth?.display_name ?? '')
                      setEditingName(false)
                    }}
                    aria-label="取消"
                  >
                    <X />
                  </Button>
                </form>
              ) : (
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-foreground-intense truncate text-sm font-semibold">
                      {auth?.display_name || '玩家'}
                    </span>
                    <button
                      type="button"
                      onClick={() => setEditingName(true)}
                      className="text-foreground-muted hover:text-foreground-intense transition-colors"
                      title="快速改名"
                    >
                      <Pencil className="size-3.5" />
                    </button>
                  </div>
                  <div className="mt-0.5 flex items-center gap-1.5">
                    {isAdmin ? (
                      <Badge color="primary" size="xs">
                        👑 房主 (本地最高管理员)
                      </Badge>
                    ) : (
                      <Badge color="neutral" size="xs">
                        🎧 玩家 (听众)
                      </Badge>
                    )}
                    {isCurrentLocal && (
                      <span className="text-foreground-muted text-[11px]">· 本机特权</span>
                    )}
                  </div>
                </div>
              )}
            </div>

            <div className="flex items-center gap-2 text-xs">
              <span className="text-foreground-muted">当前世界:</span>
              <span className="text-foreground-intense font-medium">
                {currentStation?.name || '未知世界'}
              </span>
            </div>
          </div>

          {/* 世界分类 Tabs */}
          <Tabs defaultValue="local" variant="pill" size="sm">
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="local" className="flex items-center gap-1.5">
                <Home className="size-4" />
                <span>单人世界</span>
              </TabsTrigger>
              <TabsTrigger value="lan" className="flex items-center gap-1.5">
                <Wifi className="size-4" />
                <span>局域网世界</span>
                {lanWorlds.length > 0 && (
                  <Badge color="primary" size="xs" className="ms-1">
                    {lanWorlds.length}
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value="remote" className="flex items-center gap-1.5">
                <Globe className="size-4" />
                <span>直接连接</span>
              </TabsTrigger>
            </TabsList>

            {/* Tab 1: 本机单人世界 */}
            <TabsContent value="local" className="mt-3 space-y-3">
              <div className="border-border-muted bg-background flex flex-col justify-between gap-3 rounded-xl border p-4 sm:flex-row sm:items-center">
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="text-foreground-intense text-sm font-semibold">
                      🏠 本机单人世界 (Local Host)
                    </span>
                    {isCurrentLocal ? (
                      <Badge color="success">当前世界</Badge>
                    ) : (
                      <Badge color="neutral">已挂起</Badge>
                    )}
                  </div>
                  <p className="text-foreground-muted text-xs">
                    运行于本机内部进程的专属音乐世界，享有最高管理员权限 (OP)。
                  </p>
                  <p className="text-foreground-muted font-mono text-[11px]">
                    地址: 127.0.0.1:2241
                  </p>
                </div>

                <div className="shrink-0">
                  {isCurrentLocal ? (
                    <Button variant="outline" disabled size="sm">
                      已在世界中
                    </Button>
                  ) : (
                    <Button
                      variant="primary"
                      size="sm"
                      onClick={() => handleConnectTo(null)}
                    >
                      返回本机世界
                    </Button>
                  )}
                </div>
              </div>

              <div className="border-border-muted/50 bg-background-subtle rounded-lg border p-3 text-xs text-foreground-muted">
                💡 <strong className="text-foreground-intense">提示：</strong>
                在单人世界中，您可以随意添加、上传与删除歌曲，自由切歌与管理电台配置；所有改动仅保留在本地。
              </div>
            </TabsContent>

            {/* Tab 2: 局域网世界 */}
            <TabsContent value="lan" className="mt-3 space-y-3">
              <div className="flex items-center justify-between gap-2">
                <span className="text-foreground-muted text-xs">
                  自动搜索同局域网中的其它电台实例 (UDP 54321 广播)
                </span>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleScanLan}
                  disabled={scanning}
                >
                  {scanning ? <Spinner className="size-3.5" currentColor /> : <Refresh className="size-3.5" />}
                  {scanning ? '扫描中…' : '刷新扫描'}
                </Button>
              </div>

              {lanWorlds.length === 0 ? (
                <div className="border-border-muted bg-background-subtle rounded-xl border border-dashed py-8 text-center">
                  <Wifi className="text-foreground-muted/50 mx-auto size-8 mb-2" />
                  <p className="text-foreground-intense text-sm font-medium">未发现局域网世界</p>
                  <p className="text-foreground-muted text-xs mt-1">
                    确保同 Wi-Fi / 局域网内的其它设备已启动 RakurakuMusicWorld。
                  </p>
                </div>
              ) : (
                <div className="grid gap-2.5 sm:grid-cols-2">
                  {lanWorlds.map((world) => {
                    const isTargetCurrent = world.world_id === currentStation?.world_id
                    return (
                      <div
                        key={world.world_id}
                        className="border-border-muted bg-background flex flex-col justify-between rounded-xl border p-3.5 text-xs shadow-xs"
                      >
                        <div className="space-y-1.5">
                          <div className="flex items-center justify-between gap-1">
                            <span className="text-foreground-intense truncate font-semibold">
                              {world.station_name}
                            </span>
                            <div className="flex items-center gap-1 shrink-0">
                              {world.is_headless && <Badge color="neutral" size="xs">Headless</Badge>}
                              <Badge color="primary" size="xs">v{world.version}</Badge>
                            </div>
                          </div>
                          <div className="text-foreground-muted font-mono text-[11px]">
                            {world.host}:{world.port}
                          </div>
                        </div>

                        <div className="mt-3 flex items-center justify-end border-t border-border-muted/50 pt-2">
                          {isTargetCurrent ? (
                            <Badge color="success">当前已连接</Badge>
                          ) : (
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              onClick={() => handleConnectTo(world.url)}
                            >
                              加入世界
                            </Button>
                          )}
                        </div>
                      </div>
                    )
                  })}
                </div>
              )}
            </TabsContent>

            {/* Tab 3: 直接连接 */}
            <TabsContent value="remote" className="mt-3 space-y-3">
              <div className="border-border-muted bg-background rounded-xl border p-3.5 sm:p-4 space-y-3">
                <div className="space-y-1">
                  <label htmlFor="remote-world-input" className="text-foreground-intense text-xs font-medium">
                    服务器地址 (Server IP / Domain)
                  </label>
                  <div className="flex flex-col gap-2 sm:flex-row">
                    <Input
                      id="remote-world-input"
                      type="url"
                      value={customUrl}
                      onChange={(e) => setCustomUrl(e.target.value)}
                      placeholder="http://192.168.1.100:2241 或 http://my-radio.example.com"
                      className="text-xs font-mono"
                    />
                    <div className="flex shrink-0 items-center gap-1.5">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={handleTestRemote}
                        disabled={testing || !customUrl.trim()}
                      >
                        {testing ? <Spinner currentColor className="size-3.5" /> : '测试'}
                      </Button>
                      <Button
                        type="button"
                        variant="primary"
                        size="sm"
                        disabled={!customUrl.trim()}
                        onClick={() => handleConnectTo(customUrl.trim())}
                      >
                        加入服务器
                      </Button>
                    </div>
                  </div>
                </div>

                {testResult && (
                  <Alert variant={testResult.success ? 'success' : 'error'} className="py-2 text-xs">
                    <AlertIcon>{testResult.success ? <Check /> : <X />}</AlertIcon>
                    <div>
                      <AlertTitle>{testResult.message}</AlertTitle>
                      {testResult.stationName && (
                        <AlertDescription>
                          电台名称: <strong>{testResult.stationName}</strong>
                        </AlertDescription>
                      )}
                    </div>
                  </Alert>
                )}
              </div>

              {/* 最近连接历史 */}
              {recentList.length > 0 && (
                <div className="space-y-1.5">
                  <span className="text-foreground-muted text-xs font-medium">最近连接历史</span>
                  <div className="divide-y divide-border-muted/50 border-border-muted bg-background rounded-lg border">
                    {recentList.map((url) => (
                      <div
                        key={url}
                        className="flex items-center justify-between p-2.5 text-xs hover:bg-background-subtle transition-colors cursor-pointer"
                        onClick={() => handleConnectTo(url)}
                      >
                        <span className="font-mono text-foreground-intense truncate">{url}</span>
                        <div className="flex items-center gap-2 shrink-0">
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon-sm"
                            onClick={(e) => handleRemoveRecent(url, e)}
                            title="删除此记录"
                          >
                            <Trash className="size-3.5 text-foreground-muted hover:text-destructive" />
                          </Button>
                          <Button type="button" variant="soft" size="sm">
                            连接
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </TabsContent>
          </Tabs>
        </DialogBody>

        <DialogFooter>
          <DialogClose render={<Button variant="outline">关闭</Button>} />
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
