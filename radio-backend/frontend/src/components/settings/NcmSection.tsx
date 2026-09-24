import { useEffect, useState, type FormEvent } from 'react'
import { Badge } from '@appica/ui-react/badge'
import { Button } from '@appica/ui-react/button'
import { Field, FieldDescription, FieldLabel } from '@appica/ui-react/field'
import { Skeleton } from '@appica/ui-react/skeleton'
import { Spinner } from '@appica/ui-react/spinner'
import { Textarea } from '@appica/ui-react/textarea'
import { BrandNeteaseMusic, Check, CircleCheckFilled, Flask2 } from '@appica/icons-react'
import { fetchNcmStatus, saveNcmCookie, testNcmLogin } from '@/api'
import type { NcmStatus } from '@/types'
import { useStore } from '@/store'

function toast(message: string, level: 'info' | 'success' | 'warning' | 'error' = 'info') {
  useStore.getState().addToast(message, level)
}

function errMsg(err: unknown): string {
  return err instanceof Error && err.message ? err.message : '操作失败，请重试'
}

/** 本设备的网易云账号配置（曲库导入在设置页的电台管理中）。 */
export function NcmSection() {
  const [status, setStatus] = useState<NcmStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [cookie, setCookie] = useState('')
  const [saving, setSaving] = useState(false)
  const [testing, setTesting] = useState(false)

  useEffect(() => {
    let cancelled = false
    fetchNcmStatus()
      .then((s) => {
        if (!cancelled) setStatus(s)
      })
      .catch((err) => {
        if (!cancelled) toast(errMsg(err), 'error')
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  const reload = async () => {
    try {
      setStatus(await fetchNcmStatus())
    } catch (err) {
      toast(errMsg(err), 'error')
    }
  }

  const save = async (e: FormEvent) => {
    e.preventDefault()
    const trimmed = cookie.trim()
    if (!trimmed || saving) return
    setSaving(true)
    try {
      await saveNcmCookie(trimmed)
      setCookie('')
      toast('网易云 Cookie 已保存', 'success')
      await reload()
    } catch (err) {
      toast(errMsg(err), 'error')
    } finally {
      setSaving(false)
    }
  }

  const test = async () => {
    if (testing) return
    setTesting(true)
    try {
      const result = await testNcmLogin()
      if (!result.success) throw new Error(result.output || '网易云登录测试失败')
      toast('网易云登录测试通过', 'success')
    } catch (err) {
      toast(errMsg(err), 'error')
    } finally {
      setTesting(false)
    }
  }

  return (
    <section aria-labelledby="settings-ncm-heading" className="app-panel p-4 sm:p-6">
      <h2 id="settings-ncm-heading" className="app-section-title mb-4 flex items-center gap-2">
        <BrandNeteaseMusic data-icon="start" />
        网易云账号
      </h2>

      <div className="flex flex-col gap-4">
        {loading ? (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-5 w-40" />
            <Skeleton className="h-4 w-64 max-w-full" />
          </div>
        ) : status ? (
          <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            <Badge variant={status.configured ? 'success' : 'soft'} size="xs" className="shrink-0">
              {status.configured && <CircleCheckFilled />}
              {status.configured ? '已配置' : '未配置'}
            </Badge>
            {status.method && <span className="text-foreground-muted text-sm">登录方式：{status.method}</span>}
            {status.phone_hint && (
              <span className="text-foreground-muted min-w-0 truncate text-sm">账号：{status.phone_hint}</span>
            )}
          </div>
        ) : null}

        <form onSubmit={save} className="flex flex-col gap-3">
          <Field name="ncm_cookie">
            <FieldLabel>网易云 Cookie</FieldLabel>
            <Textarea
              value={cookie}
              onChange={(e) => setCookie(e.target.value)}
              rows={3}
              placeholder="粘贴网易云 Cookie（如 MUSIC_U=…），用于本设备登录网易云"
            />
            <FieldDescription>保存在服务端与本设备身份关联的账号记录中，用于验证网易云登录状态。</FieldDescription>
          </Field>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="submit" disabled={!cookie.trim() || saving}>
              {saving ? <Spinner currentColor /> : <Check data-icon="start" />}
              {saving ? '保存中…' : '保存'}
            </Button>
            <Button type="button" variant="outline" onClick={test} disabled={testing || !status?.configured}>
              {testing ? <Spinner currentColor /> : <Flask2 data-icon="start" />}
              {testing ? '测试中…' : '测试登录'}
            </Button>
          </div>
        </form>

        <p className="text-foreground-muted text-xs">
          说明：此处配置的是本设备的网易云账号；曲库导入功能请在设置页的「电台管理」中操作。
        </p>
      </div>
    </section>
  )
}
