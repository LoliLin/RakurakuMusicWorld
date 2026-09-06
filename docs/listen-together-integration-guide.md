# Listen Together UI - 前端集成指南

> 本文档说明如何将 `listen-together-ui.html` 设计稿转化为项目可用的 Vue 组件，以及各区域对应的后端 API 接口。

## 1. 整体架构概述

### 页面结构（三栏布局）

```
┌──────────┬─────────────────────────┬───────────┐
│ 左导航栏  │     中间内容区           │ 歌词面板   │
│ 220px    │     flex: 1              │ 300px     │
│          │                         │           │
│ 品牌标题  │ ┌─ Group List ────────┐│ 实时歌词   │
│          │ │ 在线听众列表         ││ 滚动显示   │
│ 导航菜单  │ └─────────────────────┘│           │
│          │ ┌─ Now Playing ───────┐│           │
│          │ │ 专辑封面 + 歌曲信息  ││           │
│          │ │ 进度条 + 播放控件    ││           │
│          │ └─────────────────────┘│           │
│          │ ┌─ Play Next ─────────┐│           │
│ 用户头像  │ │ 待播队列            ││           │
│          │ └─────────────────────┘│           │
└──────────┴─────────────────────────┴───────────┘
```

### 推荐的 Vue 组件拆分方案

新建以下文件：

```
radio-backend/frontend/src/
├── views/
│   └── NowPlayingView.vue        ← 修改：桌面端渲染为三栏布局
├── components/
│   ├── LtSidebar.vue             ← 新增：左侧导航 + 在线听众
│   ├── LtPlayerCard.vue          ← 新增：Now Playing 播放卡片
│   ├── LtQueuePanel.vue          ← 新增：待播队列面板
│   ├── LtLyricsPanel.vue         ← 新增：右侧歌词面板（替代现有 LyricsView overlay）
│   ├── LtHeaderBar.vue           ← 可选：替代 HeaderBar
│   └── MiniPlayer.vue            ← 不变
├── composables/
│   └── useListenTogetherLayout.ts ← 新增：桌面端布局切换逻辑
└── style.css                      ← 新增暖色主题 CSS 变量
```

## 2. 主题样式迁移

### CSS 变量映射

在 `style.css` 中新增暖色主题变量集（现有变量不动，仅新增）：

```css
/* Listen Together 暖色主题 */
:root[data-layout="listen-together"] {
  /* 主色调 */
  --lt-bg: #F5F0E6;
  --lt-sidebar-bg: #EDE8DB;
  --lt-card-bg: #FFFFFF;
  --lt-selected-bg: #D4C9A8;
  --lt-green: #8BC34A;
  --lt-divider: #DDD6C6;

  /* 文字 */
  --lt-text-primary: #1C1932;
  --lt-text-secondary: #6B6880;
  --lt-text-muted: rgba(107, 104, 128, 0.5);

  /* 进度条 */
  --lt-progress-track: #E0D8C8;
  --lt-progress-fill: #5C5470;

  /* 控件 */
  --lt-btn-bg: #E8E2D4;
  --lt-play-btn-bg: #1C1932;
  --lt-play-btn-icon: #FFFFFF;

  /* 字体 */
  --lt-font-serif: 'Georgia', 'Noto Serif JP', serif;
  --lt-font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;

  /* 圆角 */
  --lt-radius-sm: 8px;
  --lt-radius-md: 12px;
  --lt-radius-lg: 16px;
  --lt-radius-pill: 9999px;

  /* 阴影 */
  --lt-shadow-card: 0 2px 8px rgba(0,0,0,0.06), 0 8px 24px rgba(0,0,0,0.04);
  --lt-shadow-cover: 0 4px 16px rgba(0,0,0,0.12);
  --lt-shadow-subtle: 0 1px 3px rgba(0,0,0,0.06);
}
```

切换逻辑：桌面端在 `NowPlayingView` 中默认应用此主题，移动端保持原有主题。

### 暗色模式变量集

在上述 `:root[data-layout="listen-together"]` 之后追加暗色覆盖：

```css
/* Listen Together 暗色模式 */
:root[data-layout="listen-together"][data-theme="dark"],
:root[data-layout="listen-together"].dark {
  --lt-bg: #1A1820;
  --lt-sidebar-bg: #141218;
  --lt-card-bg: #22202A;
  --lt-selected-bg: #2E2B38;
  --lt-divider: #2E2B3A;

  --lt-text-primary: rgba(240, 236, 255, 0.94);
  --lt-text-secondary: rgba(240, 236, 255, 0.55);
  --lt-text-muted: rgba(240, 236, 255, 0.30);

  --lt-progress-track: #2E2B3A;
  --lt-progress-fill: #A29BFE;

  --lt-btn-bg: #2A2832;
  --lt-play-btn-bg: #F0ECFF;
  --lt-play-btn-icon: #1C1932;

  --lt-shadow-card: 0 2px 8px rgba(0,0,0,0.36), 0 8px 24px rgba(0,0,0,0.24);
  --lt-shadow-cover: 0 4px 16px rgba(0,0,0,0.48);
  --lt-shadow-subtle: 0 1px 3px rgba(0,0,0,0.32);
}
```

暗色模式选择器逻辑与现有项目一致，使用 `data-theme="dark"` 属性（由 `useThemeSync.ts` + `store.ts` 的 `cycleTheme()` 控制）。三种模式切换规则：

| 模式 | `data-theme` 属性 | 行为 |
|------|-------------------|------|
| 浅色 | `data-theme="light"` 或移除属性 | 使用 `:root` / `:root[data-layout="listen-together"]` 变量 |
| 深色 | `data-theme="dark"` | 使用 `[data-theme="dark"]` 覆盖变量 |
| 跟随系统 | 无 `data-theme` 属性 + `matchMedia` 监听 | `useThemeSync.ts` 自动根据 `prefers-color-scheme` 切换 |

**实现方式**: 复用现有的 `store.themeIdx` / `cycleTheme()` / `useThemeSync()`，无需新增状态管理。只需确保 CSS 变量的 `[data-theme="dark"]` 选择器同时覆盖 `--lt-*` 变量即可。

### 可配置品牌色

电台名称、图标、副标题等均可通过管理后台 (`POST /api/admin/settings`) 修改。Listen Together 布局的主题色也应支持配置。

新增品牌色变量：

```css
:root[data-layout="listen-together"] {
  /* 品牌色 — 可通过管理后台配置或从 station 信息中读取 */
  --lt-accent: #8BC34A;        /* 默认柔和绿色 */
  --lt-accent-soft: rgba(139, 195, 74, 0.15);
  --lt-accent-text: #5a8a2a;
}
```

使用 `--lt-accent` 的组件：
- 导航选中项前的圆点指示器
- 在线听众 "NEXT" 徽章背景
- 主题切换按钮 active 态边框

**品牌色配置来源**（后端扩展建议）：

目前 `POST /api/admin/settings` 支持 `station_name`, `short_name`, `subtitle`, `description`, `icon_url`。建议新增一个 `theme_color` 字段（HEX 格式），供前端在 `loadStationInfo()` 时读取并应用：

```
// station API 返回示例
{
  "name": "RakurakuMusicWorld",
  "short_name": "RR",
  "theme_color": "#8BC34A",
  "stream_url": "/stream",
  ...
}
```

前端应用逻辑（在 `loadStationInfo()` 中追加）：
```typescript
if (info.theme_color) {
  document.documentElement.style.setProperty('--lt-accent', info.theme_color)
  // 自动生成 soft/text 变体
  document.documentElement.style.setProperty('--lt-accent-soft', info.theme_color + '26') // 15% opacity hex
  document.documentElement.style.setProperty('--lt-accent-text', info.theme_color)
}
```

如果后端暂不扩展 `theme_color` 字段，前端也可以在 `store.ts` 中增加 `themeColor` 字段并持久化到 `localStorage`，由用户在设置页面自行选择。

## 3. 各区域 API 对接详情

### 3.1 在线听众列表 (Group List)

**数据源**: WebSocket `listeners_update` 消息

**WebSocket 消息格式**:
```typescript
// types.ts 中已定义
interface WsListenersUpdate {
  type: 'listeners_update'
  count: number
  names: string[]
}
```

**Store 字段**:
```typescript
store.onlineListenerCount  // number - 在线人数
store.onlineListenerNames  // string[] - 用户名列表
```

**对接方式**:
1. WebSocket 连接自动接收 `listeners_update` 消息（已实现于 `api/websocket/messages.ts`）
2. 组件中直接读取 `store.onlineListenerNames` 渲染列表
3. "NEXT" 标签标识当前正在收听的活跃用户（可通过 store 中记录的当前用户名匹配，或标记第一个名字）
4. 头像暂无 API 支持，使用姓名首字母生成彩色圆形头像

**Vue 模板示例**:
```vue
<template>
  <div class="lt-group-list">
    <div class="lt-group-header">
      <span class="lt-back-icon">←</span>
      <span>在线听众</span>
    </div>
    <div v-for="(name, idx) in store.onlineListenerNames" :key="idx"
         class="lt-user-row">
      <div class="lt-avatar" :style="{ background: avatarColor(name) }">
        {{ name.charAt(0) }}
      </div>
      <span class="lt-username">{{ name }}</span>
      <span v-if="idx === 0" class="lt-next-badge">NEXT</span>
    </div>
    <div v-if="store.onlineListenerNames.length === 0" class="lt-empty">
      暂无其他听众
    </div>
  </div>
</template>
```

---

### 3.2 Now Playing 播放卡片

**数据源**: WebSocket `playback_state` 消息 + REST API

**WebSocket 消息格式**:
```typescript
interface WsPlaybackState {
  type: 'playback_state'
  song_id: number
  title: string
  artist: string
  position_ms: number
  duration_ms: number
  lyrics_line: number | null
  lyrics_lines?: LyricsLine[]
  status: string              // 'playing' | 'stopped' | 'paused'
  cover_url: string           // 直接可用的封面 URL
  stream_url?: string
  timestamp_ms?: number
}
```

**Store 字段**:
```typescript
store.playbackState.song_id
store.playbackState.title
store.playbackState.artist
store.playbackState.position_ms
store.playbackState.duration_ms
store.playbackState.status       // 'playing' | 'stopped' | 'paused'
store.playbackState.cover_url    // 优先使用此 URL
store.displayPositionMs         // 插值后的平滑进度（由 interpolation.ts 维护）
```

**专辑封面获取**:
```
优先级：
1. store.playbackState.cover_url  （WebSocket 推送的完整 URL）
2. GET /api/songs/{song_id}/cover （REST API 兜底）
```

**封面 URL 计算逻辑**（已在 NowPlayingView.vue 中实现）:
```typescript
const coverSrc = computed(() => {
  if (store.playbackState.cover_url) return store.playbackState.cover_url
  if (store.playbackState.song_id > 0)
    return apiUrl('/api/songs/' + store.playbackState.song_id + '/cover')
  return ''
})
```

**进度条**:
- 当前位置: `store.displayPositionMs`（毫秒，由 `interpolation.ts` 以 60fps 插值更新）
- 总时长: `store.playbackState.duration_ms`
- 进度百分比: `(displayPositionMs / duration_ms) * 100`
- 时间格式化: 已有 `formatTime(ms)` 工具函数（store.ts）
- 剩余时间: `formatTime(duration_ms - displayPositionMs)`

**播放控件**:

| 按钮 | 功能 | API |
|------|------|-----|
| ⏮ 上一首 | `adminSkipPrev()` | `POST /api/admin/playlist/prev` |
| ⏸ 播放/暂停 | `audioEl.play()` / `audioEl.pause()` | 本地 audio 元素控制 |
| ⏭ 下一首 | `adminSkipNext()` | `POST /api/admin/playlist/next` |

> 注意：上一首/下一首仅 admin 角色可用。非 admin 用户按钮应 disabled 或隐藏。

**Vue 模板示例**:
```vue
<template>
  <div class="lt-player-card">
    <div class="lt-cover">
      <img :src="coverSrc" :alt="store.playbackState.title" />
    </div>
    <h2 class="lt-title">{{ store.playbackState.title || '等待播放...' }}</h2>
    <p class="lt-artist">{{ store.playbackState.artist }}</p>

    <div class="lt-progress">
      <span>{{ formatTime(store.displayPositionMs) }}</span>
      <div class="lt-progress-track">
        <div class="lt-progress-fill"
             :style="{ width: progressPct + '%' }" />
      </div>
      <span>-{{ formatTime(store.playbackState.duration_ms - store.displayPositionMs) }}</span>
    </div>

    <div class="lt-controls">
      <button @click="onPrev" :disabled="!isAdmin">⏮</button>
      <button class="lt-play-btn" @click="togglePlay">
        {{ isPlaying ? '⏸' : '▶' }}
      </button>
      <button @click="onNext" :disabled="!isAdmin">⏭</button>
    </div>
  </div>
</template>
```

---

### 3.3 右侧歌词面板

**数据源**: WebSocket `playback_state` 消息中的 `lyrics_lines` 字段

**歌词数据结构**:
```typescript
interface LyricsLine {
  timeMs: number   // 时间戳（毫秒）
  text: string     // 歌词文本
}
```

**Store 字段**:
```typescript
store.lyricsLines          // LyricsLine[] - 完整歌词行
store.playbackState.lyrics_line  // number | null - 当前高亮行索引
store.displayPositionMs    // 当前播放位置（用于计算高亮行）
```

**歌词更新逻辑**（已在 `api/websocket/messages.ts` 中实现）:
- 歌曲切换时（`song_id` 变化），WebSocket 一次性推送完整 `lyrics_lines` 数组
- 后续每 500ms 更新只推送 `lyrics_line`（当前行索引）
- Store 缓存歌词数组，不需要重复请求

**当前行索引计算**（参考 `LyricsView.vue`）:
```typescript
const lyricActiveIdx = computed(() => {
  if (store.lyricsLines.length === 0) return -1
  const pos = store.displayPositionMs
  let idx = -1
  for (let i = store.lyricsLines.length - 1; i >= 0; i--) {
    if (store.lyricsLines[i].timeMs <= pos) {
      idx = i
      break
    }
  }
  return idx
})
```

**自动滚动**: 当 `lyricActiveIdx` 变化时，调用 `el.scrollIntoView({ behavior: 'smooth', block: 'center' })`

**Vue 模板示例**:
```vue
<template>
  <div class="lt-lyrics-panel">
    <div ref="lyricsBoxRef" class="lt-lyrics-scroll">
      <div v-for="(line, idx) in store.lyricsLines" :key="idx"
           :class="['lt-lyrics-line', {
             'active': lyricActiveIdx === idx,
             'near': Math.abs(lyricActiveIdx - idx) <= 2,
             'far': Math.abs(lyricActiveIdx - idx) > 2
           }]"
           :ref="el => { if (lyricActiveIdx === idx) scrollToLine(el) }">
        {{ line.text }}
      </div>
    </div>
  </div>
</template>
```

---

### 3.4 待播队列 (Play Next)

**数据源**: REST API `GET /api/queue`

**API 端点**:
```
GET /api/queue
Response: {
  success: true,
  data: QueueItem[]
}

QueueItem: {
  id: number
  song_id: number
  song?: Song       // 包含 title, artist, album, cover_url, duration_ms
  requested_by: string
  status: string    // 'playing' | 'pending'
  played_at?: string
  position?: number
}
```

**Store 字段**:
```typescript
store.queue  // QueueItem[]
```

**数据刷新**: 由 `refreshQueue()` 定时拉取（每 5 秒），或 WebSocket `queue_update` 消息触发

**队列项封面获取**:
```
queue[i].song?.cover_url  或  GET /api/songs/{queue[i].song_id}/cover
```

**"+ Add" 添加歌曲按钮**:
1. 点击后弹出搜索面板（复用现有 `UpNextView.vue` 的搜索逻辑）
2. 搜索 API: `GET /api/songs?q=keyword&limit=50&offset=0`
3. 点歌 API: `POST /api/queue { song_id: number }`

**Vue 模板示例**:
```vue
<template>
  <div class="lt-queue-panel">
    <div class="lt-queue-header">
      <h3>Play Next</h3>
      <button class="lt-add-btn" @click="showSearch = true">+ Add</button>
    </div>
    <div v-for="item in store.queue" :key="item.id" class="lt-queue-item">
      <div class="lt-queue-thumb">
        <img v-if="item.song?.cover_url" :src="item.song.cover_url" />
        <div v-else class="lt-queue-thumb-placeholder">♪</div>
      </div>
      <div class="lt-queue-info">
        <div class="lt-queue-title">{{ item.song?.title || '未知歌曲' }}</div>
        <div class="lt-queue-artist">{{ item.song?.artist || '' }}</div>
      </div>
    </div>
    <div v-if="store.queue.length === 0" class="lt-empty">
      队列为空
    </div>
  </div>
</template>
```

---

### 3.5 左侧导航

**数据源**: 路由配置 + Store

| 导航项 | 路由 | 对应视图 |
|--------|------|---------|
| Now Listening | `/` | `NowPlayingView.vue`（三栏布局模式） |
| My Library | `/library` | `LibraryView.vue`（保持现有单栏） |
| Account | `/settings` | `SettingsView.vue`（保持现有单栏） |

**品牌标题**: `store.stationName`（通过 `GET /api/station` 获取）

**当前用户**: `store.deviceUser?.display_name`

**导航切换逻辑**:
- 在 `Listen Together` 布局模式下，切换到 My Library / Account 时退出三栏布局，回到现有单栏视图
- 切回 Now Listening 时恢复三栏布局

---

## 4. WebSocket 事件汇总

所有实时数据通过 WebSocket 连接 (`/ws`) 推送，消息处理逻辑已在 `api/websocket/messages.ts` 中实现：

| 消息类型 | 触发频率 | 更新的 Store 字段 | 影响的 UI 区域 |
|----------|---------|-------------------|---------------|
| `playback_state` | 歌曲信息变化时 + 每 500ms 位置更新 | `playbackState.*`, `displayPositionMs`, `lyricsLines` | 播放卡片、进度条、歌词面板 |
| `queue_update` | 有人点歌时 | 间接触发 `refreshQueue()` | 待播队列 |
| `listeners_update` | 用户上下线时 | `onlineListenerCount`, `onlineListenerNames` | 在线听众列表 |
| `notice` | 管理员发通知时 | 触发 toast | 全局 |
| `ping` | 每 30 秒 | 无（需回复 pong） | 无 |

## 5. REST API 端点汇总

| 端点 | 方法 | 用途 | 使用场景 |
|------|------|------|---------|
| `/api/station` | GET | 获取电台名称、流地址、图标 | 导航栏品牌标题 |
| `/api/songs/{id}/cover` | GET | 获取歌曲封面图 | 播放卡片封面、队列项缩略图 |
| `/api/queue` | GET | 获取待播队列列表 | Play Next 面板 |
| `/api/queue` | POST | 点歌（添加到队列）| "+ Add" 功能 |
| `/api/queue/{id}` | DELETE | 移除队列项 | 队列管理（admin） |
| `/api/songs?q=&limit=&offset=` | GET | 搜索曲库歌曲 | 搜索点歌面板 |
| `/api/admin/playlist/next` | POST | 切到下一首 | 播放控件 ⏭ |
| `/api/admin/playlist/prev` | POST | 切到上一首 | 播放控件 ⏮ |
| `/api/auth/device` | POST | 设备认证（获取 device_token）| 首次访问 |

## 6. 实现步骤建议

### Phase 1: 基础框架（1-2 天）
1. 在 `style.css` 中添加 `--lt-*` 暖色主题 CSS 变量
2. 新建 `LtSidebar.vue` — 左导航栏（静态路由 + 在线听众）
3. 修改 `NowPlayingView.vue` — 桌面端条件渲染三栏布局容器

### Phase 2: 播放核心（1-2 天）
4. 新建 `LtPlayerCard.vue` — 播放卡片（封面 + 信息 + 进度条 + 控件）
5. 新建 `LtLyricsPanel.vue` — 右侧歌词面板
6. 对接现有 WebSocket 数据流（playback_state, lyrics_lines）

### Phase 3: 队列与听众（1 天）
7. 新建 `LtQueuePanel.vue` — 待播队列面板
8. 在 `LtSidebar.vue` 中对接在线听众列表

### Phase 4: 响应式与交互优化（1 天）
9. 确保移动端不受影响（三栏布局仅在 `store.isDesktop` 时激活）
10. 添加搜索点歌弹窗（复用现有搜索 API）
11. 添加切歌/暂停动效
12. 测试 WebSocket 断连重连后的 UI 状态恢复

## 7. 关键注意事项

1. **不影响移动端**: 三栏布局仅在 `store.isDesktop` (viewport >= 960px) 时启用，移动端保持现有 `NowPlayingView` 全屏居中布局不变
2. **不影响其他页面**: Library / Settings / Admin 等页面完全不受影响，三栏布局只在 Now Playing 路由生效
3. **歌词面板替代全屏 overlay**: 桌面端歌词作为常驻右栏显示，不需要点击按钮打开。移动端仍使用现有 `LyricsView.vue` 的全屏 overlay
4. **封面图兜底**: 优先使用 `playbackState.cover_url`，失败时使用 API 获取，均失败时显示渐变色占位
5. **Admin 权限控制**: 播放控件的上一首/下一首按钮仅 admin 可用，需根据 `store.deviceUser?.role === 'admin'` 判断
6. **进度插值**: 使用现有的 `interpolation.ts` 提供的 60fps 平滑进度更新，不要直接使用 `playbackState.position_ms`（会抖动）
