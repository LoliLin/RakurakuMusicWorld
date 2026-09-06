import { useEffect, useRef, useState } from 'react'
import { fetchStationSettings, saveStationSettings, uploadStationIcon } from '@/api'
import type { StationSettings } from '@/types'
import { useStore } from '@/store'
import { Button } from '@appica/ui-react/button'
import { Field, FieldLabel } from '@appica/ui-react/field'
import { Input } from '@appica/ui-react/input'
import { Skeleton } from '@appica/ui-react/skeleton'
import { Spinner } from '@appica/ui-react/spinner'
import { Textarea } from '@appica/ui-react/textarea'
import { Thumbnail } from '@appica/ui-react/thumbnail'
import { Check, Radio, Upload } from '@appica/icons-react'

const MAX_ICON_BYTES = 2 * 1024 * 1024

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : '操作失败'
}

export function AdminStationSettings() {
  const addToast = useStore((s) => s.addToast)
  const [settings, setSettings] = useState<StationSettings | null>(null)
  const [loading, setLoading] = useState(true)
  const [stationName, setStationName] = useState('')
  const [shortName, setShortName] = useState('')
  const [subtitle, setSubtitle] = useState('')
  const [description, setDescription] = useState('')
  const [saving, setSaving] = useState(false)
  const [iconFile, setIconFile] = useState<File | null>(null)
  const [uploadingIcon, setUploadingIcon] = useState(false)
  const [iconFailed, setIconFailed] = useState(false)
  const [iconVersion, setIconVersion] = useState(0)
  const fileRef = useRef<HTMLInputElement>(null)

  async function load() {
    try {
      const s = await fetchStationSettings()
      setSettings(s)
      setStationName(s.station_name ?? '')
      setShortName(s.short_name ?? '')
      setSubtitle(s.subtitle ?? '')
      setDescription(s.description ?? '')
    } catch (e) {
      addToast(errMsg(e), 'error')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  // resolved_icon_url may be relative (or absent) — render as-is; fall back when it fails to load.
  const iconSrc = settings?.resolved_icon_url || settings?.icon_url || ''
  // 本地图标（icon_path 非空）是固定 URL（/site-icon），上传后会被浏览器缓存；
  // 用版本号做 cache-busting，保证上传后立即显示新图标。
  const iconDisplaySrc = settings?.icon_path && iconSrc ? `${iconSrc}?v=${iconVersion}` : iconSrc
  // 已有图标时在输入框里显示文件名（如 site-icon.svg）
  const iconFileName = settings?.icon_path ? settings.icon_path.split(/[\\/]/).pop() ?? '' : ''

  useEffect(() => {
    setIconFailed(false)
  }, [iconSrc])

  async function handleSave() {
    setSaving(true)
    try {
      await saveStationSettings({
        station_name: stationName.trim(),
        short_name: shortName.trim(),
        subtitle: subtitle.trim(),
        description: description.trim(),
      })
      addToast('已保存（部分设置需重启服务端生效）', 'success')
    } catch (e) {
      addToast(errMsg(e), 'error')
    } finally {
      setSaving(false)
    }
  }

  async function handleUploadIcon(file: File) {
    if (file.size > MAX_ICON_BYTES) {
      addToast('图标文件不能超过 2MB', 'warning')
      return
    }
    setUploadingIcon(true)
    try {
      await uploadStationIcon(file)
      addToast('图标已上传', 'success')
      setIconFile(null)
      setIconVersion((v) => v + 1)
      await load()
    } catch (e) {
      addToast(errMsg(e), 'error')
    } finally {
      setUploadingIcon(false)
    }
  }

  async function pickIconFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0] ?? null
    e.target.value = ''
    if (!file) return
    setIconFile(file)
    await handleUploadIcon(file)
  }

  return (
    <div className="mx-auto w-full max-w-3xl space-y-8">
      {loading ? (
        <div className="space-y-4">
          <Skeleton className="h-10 w-full rounded-lg" />
          <Skeleton className="h-10 w-full rounded-lg" />
          <Skeleton className="h-10 w-full rounded-lg" />
          <Skeleton className="h-28 w-full rounded-lg" />
        </div>
      ) : (
        <>
          <section aria-label="电台信息" className="space-y-4">
            <h2 className="text-sm font-semibold">电台信息</h2>
            <Field>
              <FieldLabel>电台名称</FieldLabel>
              <Input
                inputSize="sm"
                placeholder="RakurakuMusicWorld"
                value={stationName}
                onChange={(e) => setStationName(e.target.value)}
              />
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel>短名称</FieldLabel>
                <Input
                  inputSize="sm"
                  placeholder="Rakuraku"
                  value={shortName}
                  onChange={(e) => setShortName(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel>副标题</FieldLabel>
                <Input
                  inputSize="sm"
                  placeholder="轻松听，慢慢点"
                  value={subtitle}
                  onChange={(e) => setSubtitle(e.target.value)}
                />
              </Field>
            </div>
            <Field>
              <FieldLabel>电台简介</FieldLabel>
              <Textarea
                inputSize="sm"
                rows={4}
                placeholder="向听众介绍你的电台……"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </Field>
            <Button variant="primary" size="sm" onClick={handleSave} disabled={saving}>
              {saving ? <Spinner currentColor className="size-4" aria-label="保存中" /> : <Check data-icon="start" />}
              保存设置
            </Button>
          </section>

          <section aria-label="电台图标" className="space-y-4">
            <h2 className="text-sm font-semibold">电台图标</h2>
            <div className="flex flex-wrap items-center gap-4">
              <Thumbnail
                size="xl"
                shape="rounded"
                variant="image"
                src={iconDisplaySrc || undefined}
                alt="电台图标"
                onLoadingStatusChange={(s) => {
                  if (s === 'error') setIconFailed(true)
                }}
                className="shrink-0"
                style={{ borderRadius: 12 }}
              >
                {(!iconSrc || iconFailed) && <Radio className="size-5" aria-hidden />}
              </Thumbnail>
              <div className="min-w-0 flex-1 space-y-2">
                <Input
                  readOnly
                  value={iconFile?.name || iconFileName || '未选择任何文件'}
                  inputSize="sm"
                  aria-label="电台图标文件"
                  startSlot={<Radio className="size-4" aria-hidden />}
                  endSlot={
                    <Button
                      className="-me-1.5"
                      variant="ghost"
                      size="icon-sm"
                      aria-label="选择并上传图标"
                      disabled={uploadingIcon}
                      onClick={() => fileRef.current?.click()}
                    >
                      {uploadingIcon ? <Spinner currentColor className="size-4" aria-label="上传中" /> : <Upload />}
                    </Button>
                  }
                />
                <input
                  ref={fileRef}
                  type="file"
                  accept="image/*"
                  className="hidden"
                  aria-hidden
                  onChange={pickIconFile}
                />
                <p className="text-foreground-muted mt-1.5 text-xs">支持 PNG / JPG / SVG，不超过 2MB。</p>
              </div>
            </div>
          </section>
        </>
      )}
    </div>
  )
}
