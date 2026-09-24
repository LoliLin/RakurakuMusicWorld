import { Heart } from '@appica/icons-react'
import { SongRow } from '@/components/library/SongRow'
import { useStore } from '@/store'

/** 收藏 tab：localStorage 快照列表（store.favoriteSongs），最新添加在前。 */
export function FavoritesTab() {
  const favoriteSongs = useStore((s) => s.favoriteSongs)

  if (favoriteSongs.length === 0) {
    return (
      <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border-muted bg-background-subtle/40 px-6 py-14 text-center">
        <Heart className="text-foreground-subtle size-10" aria-hidden="true" />
        <p className="text-foreground-muted text-sm">还没有收藏。在曲库歌曲右侧点击爱心即可收藏。</p>
      </div>
    )
  }

  const sorted = [...favoriteSongs].sort((a, b) => b.addedAt - a.addedAt)

  return (
    <ul aria-label="收藏的歌曲" className="divide-border-muted divide-y overflow-hidden rounded-xl border border-border-muted bg-background">
      {sorted.map(({ song }) => (
        <li key={song.id} className="flex">
          <SongRow song={song} />
        </li>
      ))}
    </ul>
  )
}
