import { Tabs, TabsList, TabsTrigger, TabsContent } from '@appica/ui-react/tabs'
import { LayoutDashboard } from '@appica/icons-react'
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

/**
 * 设置页。设备 / 外观 / 网易云账号 / 服务器连接 + （仅管理员）「电台管理」分区，
 * 取代独立的 /admin 页面。
 */
export default function SettingsPage() {
  const isAdmin = useStore((s) => s.auth?.role === 'admin')

  return (
    <div className="mx-auto w-full max-w-5xl px-4 py-6 sm:px-6">
      <div className="flex flex-col gap-4">
        <ServerSection />
        <DeviceSection />
        <AppearanceSection />
        <NcmSection />

        {isAdmin && (
          <section aria-labelledby="admin-panel-title" className="border-border-muted bg-background-subtle rounded-xl border p-4 sm:p-5">
            <h2 id="admin-panel-title" className="text-foreground-intense mb-4 flex items-center gap-2 text-lg font-semibold">
              <LayoutDashboard className="text-primary size-5" aria-hidden="true" />
              电台管理
            </h2>
            <Tabs variant="line" defaultValue="overview">
              <TabsList className="w-full overflow-x-auto">
                <TabsTrigger value="overview">概览</TabsTrigger>
                <TabsTrigger value="songs">歌曲</TabsTrigger>
                <TabsTrigger value="users">用户</TabsTrigger>
                <TabsTrigger value="downloads">下载</TabsTrigger>
                <TabsTrigger value="ncm">网易云</TabsTrigger>
                <TabsTrigger value="station">电台设置</TabsTrigger>
              </TabsList>
              <TabsContent value="overview" className="pt-4">
                <AdminOverview />
              </TabsContent>
              <TabsContent value="songs" className="pt-4">
                <AdminSongs />
              </TabsContent>
              <TabsContent value="users" className="pt-4">
                <AdminUsers />
              </TabsContent>
              <TabsContent value="downloads" className="pt-4">
                <AdminDownloads />
              </TabsContent>
              <TabsContent value="ncm" className="pt-4">
                <AdminNcm />
              </TabsContent>
              <TabsContent value="station" className="pt-4">
                <AdminStationSettings />
              </TabsContent>
            </Tabs>
          </section>
        )}
      </div>
    </div>
  )
}
