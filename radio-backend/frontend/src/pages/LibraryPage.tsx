import { Tabs, TabsList, TabsTrigger, TabsContent } from '@appica/ui-react/tabs'
import { SongSearch } from '@/components/library/SongSearch'
import { FavoritesTab } from '@/components/library/FavoritesTab'

/** 曲库页：歌曲搜索 / 本地收藏。 */
export default function LibraryPage() {
  return (
    <div className="app-page flex flex-col gap-5">
      <div>
        <p className="app-eyebrow mb-1">YOUR MUSIC LIBRARY</p>
        <h1 className="app-page-title">曲库</h1>
        <p className="text-foreground-muted mt-2 text-sm">发现音乐，加入大家的点歌队列。</p>
      </div>
      <Tabs defaultValue="songs" variant="line">
        <TabsList className="w-full sm:w-auto">
          <TabsTrigger value="songs" className="flex-1 sm:min-w-28">
            全部歌曲
          </TabsTrigger>
          <TabsTrigger value="favorites" className="flex-1 sm:min-w-28">
            我的收藏
          </TabsTrigger>
        </TabsList>
        <TabsContent value="songs" keepMounted className="pt-4">
          <SongSearch />
        </TabsContent>
        <TabsContent value="favorites" keepMounted className="pt-4">
          <FavoritesTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
