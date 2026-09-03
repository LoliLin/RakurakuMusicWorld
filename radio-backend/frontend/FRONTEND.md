# Rakuraku Music Station NG — 前端设计规范 / Frontend Design Guide

> 面向接手本项目前端开发的工程师。本文档描写浏览器端 Web UI 的架构、色彩系统、API 交互约定与开发流程。

---

## 文件位置 / File Map

| 文件 | 用途 | 状态 |
|------|------|------|
| `radio-backend/frontend/` | Vite + Vue 3 + TypeScript SFC 项目根目录 | 活跃 |
| `radio-backend/frontend/src/main.ts` | Vue 应用入口 | 活跃 |
| `radio-backend/frontend/src/App.vue` | 根组件（布局 + router-view + 全局 overlay） | 活跃 |
| `radio-backend/frontend/src/router.ts` | vue-router 路由配置（history 模式） | 活跃 |
| `radio-backend/frontend/src/store.ts` | 响应式全局 store + 工具函数 | 活跃 |
| `radio-backend/frontend/src/api.ts` | API 层（HTTP + WebSocket + 轮询） | 活跃 |
| `radio-backend/frontend/src/types.ts` | TypeScript 类型定义 | 活跃 |
| `radio-backend/frontend/src/style.css` | 全局 CSS 样式 + CSS 变量主题 | 活跃 |
| `radio-backend/frontend/src/views/` | 页面级组件 | 活跃 |
| `radio-backend/frontend/src/components/` | 共享组件 + admin 子组件 | 活跃 |
| `radio-backend/frontend/src/components/admin/` | 管理面板 7 个子组件 | 活跃 |
| `radio-backend/frontend/public/` | 直接复制的静态文件（manifest.json, sw.js） | 活跃 |
| `radio-backend/static/` | **构建产物**目录（Vite 输出 + ServeDir 目标） | 构建生成 |
| `radio-backend/static/assets/` | 带哈希的 JS/CSS 产物 | 构建生成 |

---

## 前端框架 / Frontend Framework

项目使用 **Vue 3 SFC + Vite + TypeScript**：

| 依赖 | 版本 | 用途 |
|------|------|------|
| vue | ^3.5 | 前端框架 |
| vue-router | ^4.5 | SPA 路由（history 模式） |
| vite | ^6.2 | 构建工具 + 开发服务器 |
| typescript | ~5.7 | 类型系统 |
| vue-tsc | ^2.2 | Vue SFC 类型检查 |
| @vitejs/plugin-vue | ^5.2 | Vite Vue SFC 插件 |

### 架构核心

- **SFC 组件**：每个功能模块是独立的 `.vue` 单文件组件（template + script + style）
- **响应式全局状态**：所有 UI 状态存储在 `reactive()` 单例 `store` 中（`src/store.ts`）
- **类型安全**：所有状态、API 响应、WebSocket 消息均有 TypeScript 接口定义（`src/types.ts`）
- **vue-router history 模式**：URL 驱动导航（`/`, `/library`, `/up-next`, `/settings`, `/admin/:subtab`），`/queue` 兼容跳转到 `/up-next`
- **Rust 后端 SPA 回退**：`ServeDir::new("static").fallback(ServeFile::new("static/index.html"))`；`/api/*` 单独注册 JSON fallback，避免 API 404 返回 HTML
- **Base path aware**：`VITE_BASE_PATH` 控制 Vite base、Vue Router history、API URL、WebSocket、PWA manifest 和 service worker scope
- **Theme aware**：设置页的自动/浅色/深色会同步 Vuetify `useTheme()` 与 `data-theme` CSS 变量，自动模式跟随系统偏好
- **Motion system**：全局 easing token 使用非线性曲线，列表/迷你播放器/背景过渡有动效，并通过 `prefers-reduced-motion` 降级

### 关键文件结构

**`src/main.ts`** — 应用入口
```
import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import './style.css'
createApp(App).use(router).mount('#app')
```

**`src/App.vue`** — 根组件
- App layout：Vuetify 侧边栏/底部导航 + `<router-view>` + 全局音频元素 + MiniPlayer
- 生命周期：initAudio → loadStationInfo → connectWebSocket → loadDeviceUser → 轮询

**`src/store.ts`** — 全局状态
```
reactive({
  deviceUser, stationName,
  playbackState, lyricsLines, useFileMode,
  queue, history, searchQuery, searchResults,
  myPlaylists, toasts,
  // ... 管理面板状态
})
```
导出工具函数：`formatTime`, `toast`, `applyTheme`, `cycleTheme`, `applyStationColors`

**`src/api.ts`** — API 聚合导出
- 具体实现拆在 `src/api/{client,station,auth,queue,songs,playlists,admin,ncm,websocket}.ts`
- 所有 HTTP URL 通过 `apiUrl('/api/...')` 拼接，避免硬编码域名根路径
- WebSocket 通过 `appPath('/ws')` 拼接，支持 `/radio/ws` 之类的子路径部署
- HTTP：fetch + httpOnly `device_token` cookie
- WebSocket：自动重连（指数退避，最多 20 次）
- 轮询：队列 5s、播放状态 2s（WebSocket 断开时兜底）
- 播放控制：推流模式为主，文件播放模式作为内部备份
- 曲库：`GET /api/songs?q=&limit=&offset=` 同时服务公开全站列表和搜索结果；前端分页追加并显示总数

**`src/router.ts`** — 路由配置
```
/            → NowPlayingView
/library     → LibraryView
/up-next     → UpNextView
/queue       → redirect /up-next
/settings    → SettingsView
/admin       → redirect /admin/users
/admin/:subtab → AdminView (meta: requiresAdmin)
```
导航守卫：`/admin/*` 需要 `role === 'admin'`，否则重定向到 `/`。

### 组件树 / Component Tree

```
App.vue
├── Vuetify navigation         # 桌面侧边栏 / 移动端底部导航
├── audio                      # 全局音频元素，使用 /stream
├── <router-view>
│   ├── NowPlayingView.vue     # 播放页面（封面、进度条、歌词、管理员切歌）
│   ├── UpNextView.vue         # 待播队列 + 播放历史
│   ├── LibraryView.vue        # 曲库搜索 + 我的歌单 + NCM 用户配置
│   ├── SettingsView.vue       # 显示名称、主题、管理员提权
│   └── AdminView.vue          # 管理页面壳（router-view 分配子标签）
│       ├── AdminUsers.vue     # 👥 用户管理（提权/降权/封禁/日志）
│       ├── AdminSongs.vue     # 🎵 歌曲管理（删除/重新扫描/切歌）
│       ├── AdminUpload.vue    # 📤 上传
│       ├── AdminDownload.vue  # ⬇️ 批量下载
│       ├── AdminNcm.vue       # 🎵 网易云（复用 NcmSettings 组件）
│       ├── AdminSettings.vue  # ⚙️ 系统设置
│       └── AdminStats.vue     # 📊 统计
├── MiniPlayer.vue             # 非播放页底部/侧边迷你播放器
└── NcmSettings.vue            # 网易云配置（管理员 + 用户复用）
```

---

## 色彩系统 / Color System

所有颜色通过 CSS 自定义属性（变量）引用。

```css
:root {
    --primary: #764ba2;
    --secondary: #667eea;
    --bg: #f4f4f9;
    --card: #ffffff;
    --text: #333;
    --text-muted: #6c757d;
    --text-inverse: #ffffff;
    --border: #e9ecef;
    --success: #28a745;
    --danger: #dc3545;
    --warning: #ffc107;
    --info: #17a2b8;
    --radius: 12px;
    --shadow: 0 4px 20px rgba(0,0,0,0.08);
}
```

`--primary`, `--secondary`, `--bg` 在运行时由 `/api/station` 接口动态覆盖。

### 主题与动效

- 主题状态存储在 `localStorage.radio_theme`，取值为 `auto`、`light`、`dark`。
- `auto` 通过 `prefers-color-scheme` 选择 Vuetify 主题，并监听系统变化。
- `style.css` 中的 `[data-theme="dark"]` 覆盖自定义布局变量，避免 Vuetify 深色但自定义容器仍浅色。
- 动效使用 `--am-ease-emphasized`、`--am-ease-spring`、`--am-ease-exit`，避免线性机械感。
- `prefers-reduced-motion: reduce` 会显著降低动画和过渡。

---

## 与后端通信 / API Communication

### 端口约定

| 服务 | 端口 | 说明 |
|------|------|------|
| Rust 后端（API + WebSocket + 音频流 + 静态文件） | **2241** | 前端从此端口加载 |
| Vite 开发服务器 | **5173** | 仅开发时使用，代理 API 到 2241 |

### 开发代理

`vite.config.ts` 中的 dev server 代理配置：
```ts
proxy: {
  '/api': 'http://localhost:2241',
  '/ws': { target: 'ws://localhost:2241', ws: true },
  '/stream': 'http://localhost:2241',
}
```

### 子路径与 PWA

根路径部署使用默认构建：

```bash
npm run build
```

子路径部署必须显式设置 Vite base，并与后端 `[server].base_path` 一致：

```bash
VITE_BASE_PATH=/radio/ npm run build
```

相关约定：

- `src/router.ts` 使用 `createWebHistory(import.meta.env.BASE_URL)`。
- `src/main.ts` 使用 `navigator.serviceWorker.register(BASE_URL + 'sw.js', { scope: BASE_URL })`。
- `public/manifest.json` 使用相对 `id`、`start_url`、`scope` 和 icon 路径。
- `public/sw.js` 通过 `self.registration.scope` 计算当前部署前缀，API、WebSocket、音频流不缓存。
- `public/icon.svg`、`icon-192.png`、`icon-512.png` 会复制到 `radio-backend/static/`，用于 PWA 安装。

### WebSocket 消息

JSON，用 `type` 字段区分：`playback_state`, `queue_update`, `notice`, `ping`。

### HTTP API

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| `GET` | `/api/station` | — | 电台名、主题色、流地址 |
| `GET` | `/api/now-playing` | — | 当前曲目（HTTP 兜底） |
| `GET` | `/api/songs?q=&limit=50` | — | 曲库搜索 |
| `GET` | `/api/auth/me` | Device | 当前设备信息 |
| `POST` | `/api/auth/name` | Device | 设置显示名称 |
| `POST` | `/api/auth/claim-admin` | — | 通过 admin_setup_token 提权 |
| `GET` `POST` | `/api/queue` | Device/— | 队列 |
| `GET` `POST` `DELETE` | `/api/playlists` | Device | 歌单 |
| `*` | `/api/admin/*` | Admin | 管理操作 |

---

## 开发流程 / Workflow

```bash
cd radio-backend/frontend

# 开发模式（热更新代理到 Rust 后端）
npm run dev
# → http://localhost:5173

# 生产构建（输出到 ../static/）
npm run build
# → 生成 radio-backend/static/index.html + assets/*

# 子路径生产构建（需与后端 server.base_path 一致）
VITE_BASE_PATH=/radio/ npm run build

# Rust 后端构建
cd .. && cargo build
# 或从项目根目录
./build_release.sh
# 注意：build_release.sh 只复制现有 radio-backend/static/，不会自动运行 Vite。
```

### 构建集成

`vite.config.ts` 配置：
- `build.outDir: '../static'` — 输出到 `radio-backend/static/`
- `build.emptyOutDir: true` — 每次构建清空旧产物
- 默认 `publicDir: 'public'` — `public/` 下的文件（sw.js, manifest.json）原样复制到输出
- `base: process.env.VITE_BASE_PATH || '/'` — 控制静态资源、router、PWA 在子路径部署时的前缀

### 开发规范 / Conventions

- **TypeScript 优先** — 所有新增代码使用 `.ts` / `.vue` SFC
- **SFC `<script setup lang="ts">`** — 使用 Composition API + `<script setup>` 语法
- **响应式全局 store** — 新增状态到 `store.ts` 的 `reactive()` 对象
- **API 封装** — 新增接口调用到 `src/api/*.ts`，并通过 `src/api.ts` 聚合导出
- **不硬编码端口或根路径** — HTTP 用 `apiUrl()`，应用内路径用 `appPath()`，避免破坏子路径部署
- **CSS 变量** — 所有颜色通过 CSS 自定义属性
- **组件拆分** — 按功能视图拆分，提取可复用共享组件；管理面板每个子标签独立组件
- **状态提示** — 使用 `<StatusMessage>` 组件，不重复内联样式
- **路由守卫** — 需要认证的管理页面通过 `router.beforeEach` 保护
