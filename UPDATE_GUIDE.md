# 更新指南：切换到 Vue 单页面前端

本文说明本次更新后的构建、部署和访问方式变化。

## 主要变化

- 页面不再由后端 Askama 模板渲染，统一改为 Vue SPA。
- 后端仍是单二进制 `radio-backend`，继续服务 REST API、WebSocket `/ws`、音频流 `/stream` 和静态前端文件。
- `/`、`/library`、`/up-next`、`/settings`、`/admin/*` 等页面路径都会返回 `static/index.html`，由前端路由接管。
- 旧的 `/queue` 入口会兼容跳转到 `/up-next`。
- `/api/*` 路由仍返回 JSON；不存在的 API 不会回退到 HTML 页面。
- 后端现在支持 `server.base_path` 原生子路径挂载，前端可用 `VITE_BASE_PATH` 构建匹配路径。
- PWA 的 `manifest.json`、`sw.js`、service worker scope 和图标路径已支持根路径与子路径部署。
- 网易云下载遇到上游空响应、非 JSON 响应或风控页时，会返回更明确的错误，不再只暴露 `expected value at line 1 column 1`。
- 设置页主题切换现在会同步 Vuetify 主题和自定义 CSS 变量，自动模式会跟随系统深浅色。
- 曲库页不再只显示搜索结果，默认会公开展示全站歌曲总数和分页列表。
- 前端增加非线性动效，并通过 `prefers-reduced-motion` 尊重系统减少动态效果设置。

## 构建顺序

前端改动后，必须先构建前端，再打包发布目录：

```bash
cd radio-backend/frontend
npm run build
cd ../..
./build_release.sh
```

注意：`./build_release.sh` 只复制现有的 `radio-backend/static/` 到 `dist/static/`，不会自动运行 Vite。

## 子路径部署

根路径部署保持默认：

```toml
[server]
base_path = "/"
```

如果要部署到 `https://example.com/radio/`：

```toml
[server]
base_path = "/radio"
```

前端构建必须使用相同前缀，并保留结尾斜杠：

```bash
cd radio-backend/frontend
VITE_BASE_PATH=/radio/ npm run build
cd ../..
./build_release.sh
```

后端会原生服务 `/radio/`、`/radio/api/*`、`/radio/ws`、`/radio/stream`。反向代理应把 `/radio` 前缀原样转发给后端，不要剥离前缀。

## PWA 检查

HTTPS 反代下建议检查：

- 根路径部署：`/manifest.json`、`/sw.js`、`/icon-192.png`、`/icon-512.png`。
- 子路径部署：`/radio/manifest.json`、`/radio/sw.js`、`/radio/icon-192.png`、`/radio/icon-512.png`。
- `sw.js` 必须返回 JavaScript，不能由 SPA fallback 返回 `index.html`。
- 更新前端后，如浏览器仍使用旧 UI，可在 DevTools Application 面板 unregister 旧 service worker 后刷新。

## 后端单独构建

只验证 Rust 后端时可以运行：

```bash
cd radio-backend
cargo build
```

## 开发模式

开发前端时运行 Vite：

```bash
cd radio-backend/frontend
npm run dev
```

Vite 监听 `5173`，并把 `/api`、`/ws`、`/stream` 代理到 `localhost:2241`。

## 部署后检查

发布后从 `dist/` 启动：

```bash
cd dist
./start.sh
```

建议检查：

- 清空浏览器 Cookie 后访问 `/`，应直接进入 Vue 页面，不应显示 `{"error":"Not authenticated","success":false}`。
- 直接访问 `/library`、`/up-next`、`/settings`、`/admin/users`，应由 SPA 正常打开。
- 访问 `/queue` 应跳转到 `/up-next`。
- 访问 `/api/station` 应返回 JSON。
- 访问不存在的 `/api/...` 应返回 JSON 404，而不是前端 HTML。
- 如果设置了 `base_path = "/radio"`，以上路径都应带 `/radio` 前缀，例如 `/radio/api/station`。

## 首次认证变化

后端会在首次无 `device_token` 的请求中创建设备用户，并在同一次请求内让受保护 API 能读取新 token，同时通过 `Set-Cookie` 写入浏览器。

这避免首次进入电台时先看到未认证 JSON，再刷新才正常的问题。
