import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Alert,
  AlertDescription,
  AlertIcon,
  AlertTitle,
} from '@appica/ui-react/alert'
import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@appica/ui-react/alert-dialog'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { Field, FieldDescription, FieldLabel } from '@appica/ui-react/field'
import { Input } from '@appica/ui-react/input'
import { Spinner } from '@appica/ui-react/spinner'
import {
  Check,
  CrownFilled,
  DeviceMobile,
  InfoCircleFilled,
  LayoutDashboard,
  Logout,
  Pencil,
  UserFilled,
  X,
} from '@appica/icons-react'
import { adminLogout, fetchMe, setDisplayName } from '@/api'
import { useStore } from '@/store'

function toast(message: string, level: 'info' | 'success' | 'warning' | 'error' = 'info') {
  useStore.getState().addToast(message, level)
}

function errMsg(err: unknown): string {
  return err instanceof Error && err.message ? err.message : '操作失败，请重试'
}

/** 设备命名/改名表单（setDisplayName 会创建设备并持久化）。 */
function DeviceNameForm({ initialValue = '' }: { initialValue?: string }) {
  const [name, setName] = useState(initialValue)
  const [saving, setSaving] = useState(false)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    const trimmed = name.trim()
    if (!trimmed || saving) return
    setSaving(true)
    try {
      await setDisplayName(trimmed)
      const me = await fetchMe()
      useStore.getState().setAuth(me)
      toast('设备名称已保存', 'success')
    } catch (err) {
      toast(errMsg(err), 'error')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <Alert variant="info">
        <AlertIcon>
          <InfoCircleFilled />
        </AlertIcon>
        <AlertTitle>还没有设备身份</AlertTitle>
        <AlertDescription>为这台设备设置一个名称，之后即可参与点歌。</AlertDescription>
      </Alert>
      <form onSubmit={submit} className="flex flex-col gap-3">
        <Field name="display_name">
          <FieldLabel>设备名称</FieldLabel>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={32}
            placeholder="例如：客厅音箱"
            autoComplete="off"
          />
          <FieldDescription>最多 32 个字符，可随时修改。</FieldDescription>
        </Field>
        <div>
          <Button type="submit" disabled={!name.trim() || saving}>
            {saving ? <Spinner currentColor /> : <Check data-icon="start" />}
            {saving ? '保存中…' : '保存'}
          </Button>
        </div>
      </form>
    </div>
  )
}

/** 已登录设备：身份展示 + 行内改名（铅笔）+ 提权（盾牌）+ 退出（需确认）。 */
function DeviceIdentity() {
  const auth = useStore((s) => s.auth)
  const navigate = useNavigate()
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [loggingOut, setLoggingOut] = useState(false)
  const isAdmin = auth?.role === 'admin'

  const [editing, setEditing] = useState(false)
  const [name, setName] = useState(auth?.display_name ?? '')
  const [savingName, setSavingName] = useState(false)

  const startEdit = () => {
    setName(auth?.display_name ?? '')
    setEditing(true)
  }

  const cancelEdit = () => {
    setName(auth?.display_name ?? '')
    setEditing(false)
  }

  const saveName = async (e: FormEvent) => {
    e.preventDefault()
    const trimmed = name.trim()
    if (!trimmed || savingName) return
    setSavingName(true)
    try {
      await setDisplayName(trimmed)
      const me = await fetchMe()
      useStore.getState().setAuth(me)
      setEditing(false)
      toast('设备名称已保存', 'success')
    } catch (err) {
      toast(errMsg(err), 'error')
    } finally {
      setSavingName(false)
    }
  }

  const handleLogout = async () => {
    if (loggingOut) return
    setLoggingOut(true)
    try {
      await adminLogout()
      useStore.getState().setAuth(null)
      setConfirmOpen(false)
      toast('已退出登录', 'success')
      navigate('/')
    } catch (err) {
      toast(errMsg(err), 'error')
    } finally {
      setLoggingOut(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3">
        <span className="bg-background-muted flex size-10 shrink-0 items-center justify-center rounded-full">
          {isAdmin ? <CrownFilled className="text-primary size-5" /> : <UserFilled className="text-foreground-muted size-5" />}
        </span>
        {editing ? (
          <form onSubmit={saveName} className="flex min-w-0 flex-1 items-center gap-1.5">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={32}
              autoFocus
              inputSize="sm"
              aria-label="设备名称"
              className="min-w-0 flex-1"
            />
            <Button type="submit" variant="soft" size="icon-sm" aria-label="保存名称" disabled={!name.trim() || savingName}>
              {savingName ? <Spinner currentColor /> : <Check />}
            </Button>
            <Button type="button" variant="ghost" size="icon-sm" aria-label="取消" onClick={cancelEdit}>
              <X />
            </Button>
          </form>
        ) : (
          <div className="min-w-0">
            <p className="text-foreground-intense truncate text-sm font-semibold">{auth?.display_name ?? '玩家'}</p>
            <Badge variant={isAdmin ? 'primary' : 'soft'} size="xs" className="mt-0.5">
              {isAdmin ? <CrownFilled /> : <UserFilled />}
              {isAdmin ? '👑 房主 (本地管理员)' : '普通玩家 (听众)'}
            </Badge>
          </div>
        )}
        <div className="ms-auto flex shrink-0 items-center gap-1">
          <Button variant="ghost" size="icon-sm" aria-label="改名" onClick={startEdit}>
            <Pencil />
          </Button>
        </div>
      </div>

      {isAdmin && (
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" render={<a href="#admin-panel-title" />}>
            <LayoutDashboard data-icon="start" />
            电台管理
          </Button>
          <Button variant="destructive" onClick={() => setConfirmOpen(true)}>
            <Logout data-icon="start" />
            退出登录
          </Button>
        </div>
      )}

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>退出登录？</AlertDialogTitle>
            <AlertDialogDescription>
              将清除本设备的登录状态，设备名称需要重新设置后才能继续使用。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogClose render={<Button variant="outline">取消</Button>} />
            <Button variant="destructive" onClick={handleLogout} disabled={loggingOut}>
              {loggingOut && <Spinner currentColor />}
              {loggingOut ? '退出中…' : '确认退出'}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

export function DeviceSection() {
  const auth = useStore((s) => s.auth)

  return (
    <section aria-labelledby="settings-device-heading" className="rounded-2xl border border-border-muted bg-background-subtle p-4 sm:p-5">
      <h2 id="settings-device-heading" className="mb-4 flex items-center gap-2 text-base font-semibold">
        <DeviceMobile data-icon="start" />
        设备
      </h2>
      <div className="flex flex-col gap-4">
        {auth ? <DeviceIdentity /> : <DeviceNameForm />}
      </div>
    </section>
  )
}
