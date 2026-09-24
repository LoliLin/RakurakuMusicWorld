import { useState } from 'react'
import { DeviceSection } from '@/components/settings/DeviceSection'
import { AppearanceSection } from '@/components/settings/AppearanceSection'
import { NcmSection } from '@/components/settings/NcmSection'
import { ServerSection } from '@/components/settings/ServerSection'
import { AdminOverview } from '@/components/admin/AdminOverview'
import { AdminSongs } from '@/components/admin/AdminSongs'
import { AdminUsers } from '@/components/admin/AdminUsers'
import { AdminDownloads } from '@/components/admin/AdminDownloads'
import { AdminNcm } from '@/components/admin/AdminNcm'
import { AdminStationSettings } from '@/components/admin/AdminStationSettings'
import { useStore } from '@/store'

type SettingsSection = 'device' | 'appearance' | 'world' | 'ncm' | 'admin'
type AdminSection = 'overview' | 'songs' | 'users' | 'downloads' | 'ncm' | 'station'

const adminSections: { id: AdminSection; label: string }[] = [
  { id: 'overview', label: '概览' },
  { id: 'songs', label: '歌曲' },
  { id: 'users', label: '用户' },
  { id: 'downloads', label: '下载' },
  { id: 'ncm', label: '网易云' },
  { id: 'station', label: '电台设置' },
]

export default function SettingsPage() {
  const isAdmin = useStore((s) => s.auth?.role === 'admin')
  const [section, setSection] = useState<SettingsSection>('device')
  const [adminSection, setAdminSection] = useState<AdminSection>('overview')
  const sections: { id: SettingsSection; label: string }[] = [
    { id: 'device', label: '个人资料' },
    { id: 'appearance', label: '外观与通知' },
    { id: 'world', label: '世界连接' },
    { id: 'ncm', label: '网易云账号' },
    ...(isAdmin ? [{ id: 'admin' as const, label: '电台管理' }] : []),
  ]

  return (
    <div className="app-page">
      <div className="mb-7">
        <p className="app-eyebrow mb-1">PERSONAL SPACE</p>
        <h1 className="app-page-title">我的设置</h1>
        <p className="text-foreground-muted mt-2 text-sm">管理设备身份、外观和当前音乐世界。</p>
      </div>
      <div className="grid min-w-0 gap-6 lg:grid-cols-[220px_minmax(0,1fr)] lg:gap-8">
        <nav aria-label="设置分类" className="min-w-0">
          <div className="border-border-muted grid grid-cols-2 gap-2 border-b pb-3 sm:grid-cols-4 lg:sticky lg:top-4 lg:flex lg:flex-col lg:border-b-0 lg:pb-0">
            {sections.map(({ id, label }) => (
              <button
                key={id}
                type="button"
                aria-current={section === id ? 'page' : undefined}
                onClick={() => setSection(id)}
                className={`app-interactive min-h-11 rounded-lg px-4 text-center text-sm font-medium transition-colors lg:w-full lg:text-left ${section === id ? 'bg-primary-subtle text-primary' : 'text-foreground-muted hover:bg-background-subtle hover:text-foreground-intense'}`}
              >
                {label}
              </button>
            ))}
          </div>
        </nav>
        <div className="min-w-0">
          {section === 'device' && <DeviceSection onOpenAdmin={() => setSection('admin')} />}
          {section === 'appearance' && <AppearanceSection />}
          {section === 'world' && <ServerSection />}
          {section === 'ncm' && <NcmSection />}
          {section === 'admin' && isAdmin && (
            <section aria-labelledby="admin-panel-title" className="app-panel min-w-0 p-4 sm:p-6">
              <h2 id="admin-panel-title" className="app-section-title mb-1">电台管理</h2>
              <p className="text-foreground-muted mb-5 text-sm">曲库、听众与电台设置集中在这里。</p>
              <nav aria-label="电台管理分类" className="grid grid-cols-3 gap-2 sm:flex sm:flex-wrap">
                {adminSections.map(({ id, label }) => (
                  <button
                    key={id}
                    type="button"
                    aria-current={adminSection === id ? 'page' : undefined}
                    onClick={() => setAdminSection(id)}
                    className={`app-interactive min-h-11 min-w-0 rounded-lg px-3 text-sm font-medium transition-colors ${adminSection === id ? 'bg-primary-subtle text-primary' : 'text-foreground-muted hover:bg-background-subtle hover:text-foreground-intense'}`}
                  >
                    {label}
                  </button>
                ))}
              </nav>
              <div className="min-w-0 pt-5">
                {adminSection === 'overview' && <AdminOverview />}
                {adminSection === 'songs' && <AdminSongs />}
                {adminSection === 'users' && <AdminUsers />}
                {adminSection === 'downloads' && <AdminDownloads />}
                {adminSection === 'ncm' && <AdminNcm />}
                {adminSection === 'station' && <AdminStationSettings />}
              </div>
            </section>
          )}
        </div>
      </div>
    </div>
  )
}
