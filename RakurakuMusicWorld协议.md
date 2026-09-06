# RakurakuMusicWorld 协议

> RakurakuMusicWorld 前后端通信协议（v3）。前端实现见
> `radio-backend/frontend/`（React 19 + Appica UI），后端见 `radio-backend/`（axum + SQLite + 内嵌 radio-engine）。
> 本文件是修改后端时的**兼容性红线**：改字段/结构前先对照此文档，避免破坏前端。

---

## 0. 总览

- 单端口 `2241`，单二进制。所有路径可嵌套在 `server.base_path` 下（默认 `/`）。
- REST 响应一律 `{ "success": bool, "data": T | null, "error": string | null }`，
  **例外**：`GET /api/station`、`GET /api/now-playing`、`GET /api/listeners`、`GET /manifest.json` 为裸 JSON。
- 时间戳序列化为 RFC3339 字符串（SQLite `NaiveDateTime`）。
- 设备身份：`device_token` httpOnly Cookie（`Path={base_path}`、`SameSite=Lax`、`Max-Age=365d`，HTTPS 反代下加 `Secure`），由全局中间件自动下发。
- 角色：`admin` / `user`。

---

## 1. REST API

### 1.1 公开信息（无需认证）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/station` | 电台信息（裸 JSON）：`{name, short_name, subtitle, description, icon_url, manifest_url, stream_url, ws_url, needs_setup}`。`ws_url` 按 `X-Forwarded-Host`/`Host` 推断（https→wss），无头时返回相对路径 `/ws`。`needs_setup` = 尚无管理员 |
| GET | `/api/now-playing` | 当前曲目快照（裸 JSON）：`{song: SongSummary\|null, position_ms, duration_ms, lyrics_line: number\|null, lyrics_text: string\|null, started_at: string\|null, stream_url, file_url: string\|null, cover_url: string\|null}` |
| GET | `/api/listeners` | `{success, count, listeners: [{display_name, connected_at}]}`（仅已持久化、未封禁设备） |
| GET | `/api/songs?q=&limit=&offset=` | 歌曲搜索/分页：`{success, data: {data: SongSummary[], total, limit, offset}}` |
| GET | `/api/songs/:id` | 歌曲详情：`Song`（含 `genre, year, file_path, lyrics_path, cover_path, filesize, created_at`） |
| GET | `/api/songs/:id/cover` | 封面图（JPEG/PNG/SVG）。无封面/未配置时**快速**返回占位 SVG（200） |
| GET | `/api/songs/:id/download` | 下载歌曲文件 |
| GET | `/manifest.json` | PWA manifest（裸 JSON，图标来自站点图标或默认） |
| GET | `/site-icon` | 站点图标；未上传时返回默认电台 SVG（200，不再 404） |
| GET | `/stream` | 直播音频流（见 §3） |

### 1.2 队列

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/queue` | 无 | 队列列表：`[{id, song: SongSummary\|null, requested_by, status, position, added_at}]`。`song.id` 为**真实 id**；`has_lyrics`/`has_cover` 真实。非管理员点歌的 `requested_by` 为字面量 `"匿名"` |
| POST | `/api/queue` | Device | 点歌：body `{song_id}`。限流：每设备 3 次/300s、冷却 60s |
| DELETE | `/api/queue/:id` | Admin | 移除队列项 |
| POST | `/api/queue/:id/move` | Admin | 移动：body `{new_position}` |
| POST | `/api/queue/skip` | Admin | 跳过当前曲目（触发 /stream resync） |
| GET | `/api/queue/history` | Admin | 播放历史（20 条）：`[{id, song_id, device_user_id: number\|null, played_at}]` |

### 1.3 设备与认证

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/auth/me` | `{id, display_name, role}`（设备未持久化时 401） |
| POST | `/api/auth/name` | body `{display_name}`（1–32 字符）→ 设置/改名 |
| POST | `/api/auth/claim-admin` | body `{admin_setup_token}` → 提权为 admin |
| POST | `/api/admin/logout` | 清除 device_token Cookie |

### 1.4 曲库（播放列表 / 收藏）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET/POST | `/api/playlists` | 列表 `[{id, device_user_id, name, is_public, song_count, created_at}]` / 创建 `{name, is_public?}` |
| GET/DELETE | `/api/playlists/:id` | 详情（playlist + songs）/ 删除 |
| POST/DELETE | `/api/playlists/:id/songs` `/…/songs/:song_id` | 加歌 / 移除歌曲 |
| GET | `/api/favorites` | 收藏列表 `[{id, device_user_id, song_id\|null, playlist_id\|null, created_at}]` |
| POST | `/api/favorites` | body `{song_id}` 或 `{playlist_id}` |
| DELETE | `/api/favorites/:id` | 取消收藏 |

> 前端当前仅使用曲库搜索与本地收藏（localStorage）；播放列表/收藏 API 保留供扩展。

### 1.5 网易云（设备级）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/ncm` | `{configured, method, phone_hint}` |
| POST | `/api/ncm` | body `{cookie}` |
| POST | `/api/ncm/test` | 测试登录 |

### 1.6 管理（全部 Admin）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/admin/users` | 设备用户列表 |
| POST | `/api/admin/users/:id/ban` `/unban` | 封禁 / 解封 |
| PUT | `/api/admin/users/:id/role` | body `{role: "admin"\|"user"}` |
| GET | `/api/admin/stats` | 统计（数字字段） |
| GET | `/api/admin/logs` | 管理日志 `[{id, admin_id, action, details, created_at}]` |
| POST | `/api/admin/rescan-songs` | 重新扫描媒体目录（触发 ReloadQueue；**入库即预热封面缓存**） |
| GET/DELETE | `/api/admin/songs` `/api/admin/songs/:id` | 全部歌曲 / 删除 |
| POST | `/api/admin/upload` | multipart `file`（≤100MB）；**入库即预热封面缓存** |
| GET/POST | `/api/admin/settings` | 电台设置（见 §4） |
| POST | `/api/admin/settings/icon` | multipart `file`（≤2MB）站点图标 |
| POST | `/api/admin/playlist/next` `/prev` | 切歌（触发 /stream resync） |
| POST | `/api/admin/download` | 旧版批量下载 `{playlist, quality?, format?}` |
| GET | `/api/admin/download/stream` `/status` | SSE 进度 / 状态 `{running, log}` |
| POST | `/api/admin/download/batch` | 新版批量下载 `{source, items: [{id?, url?, artist?, title?, save_as?, override_lyrics}], lyrics_save_mode: "none"\|"separate"\|"overwrite", quality?, format?}` → `{task_id, total}` |
| GET | `/api/admin/download/batch/stream` `/status` | SSE 事件 `{log, done, task_id?}` / 状态 `{task_id, running, source, total, success, failed, items: [{id?, title?, artist?, success, error?, file_path?}]}` |
| GET/POST | `/api/admin/ncm` `/test` `/playlist` `/import` | 网易云管理（`/playlist` body `{link}` → `{total, batch_id, message}`） |

---

## 2. WebSocket（`/ws`）

- 明文文本 JSON；`type` 字段为判别符（internally tagged，字段平铺）。
- **保活**：服务端每 30s 发 `{"type":"ping","timestamp":…}`，客户端必须在 60s 内回**字面量 `pong`**，否则被断开。服务端不接收其他客户端消息。
- 服务端消息类型：`playback_state` / `queue_update` / `notice` / `ping` / `listeners_update`。

### 2.1 playback_state（每 500ms）

```json
{
  "type": "playback_state",
  "song_id": 64,
  "title": "…",
  "artist": "…",
  "position_ms": 123456,
  "duration_ms": 270582,
  "lyrics_line": 12,
  "lyrics_lines": null,
  "status": "playing",
  "stream_url": "/stream",
  "file_url": "/api/songs/64/file",
  "cover_url": "/api/songs/64/cover",
  "timestamp_ms": 1786000000000
}
```

- `song_id`：DB 主键；曲库外的曲目为 `-1`（此时 `file_url`/`cover_url` 为 `null`）。
- `status`：`"playing" | "stopped" | "crossfading"`。
- **歌词契约**（前端强依赖）：
  - 切歌后的**第一条**消息携带全量 `lyrics_lines`：有歌词为 `[{time_ms, text}, …]`，无歌词为 **`[]`**（与 `null` 语义严格区分）。
  - 后续 500ms 消息 `lyrics_lines: null`，只带 `lyrics_line`（当前行索引）。
  - **新连接建立时服务端补发**最近一条全量帧（含 `lyrics_lines`），重连客户端也能立即拿到歌词。

### 2.2 其他消息

```json
{"type": "queue_update", "action": "added", "song_title": "…", "requested_by": "…", "queue_size": 5}
{"type": "notice", "message": "Connected to …", "level": "info"}
{"type": "ping", "timestamp": 1786000000000}
{"type": "listeners_update", "count": 3, "names": ["A", "B"]}
```

- `listeners_update.names`：已持久化且未封禁的在线设备 display_name（不含匿名设备；可能包含自己）。
- `notice.level`：`"info" | "warning" | "error"`。

---

## 3. `/stream` 直播流

- `200`、`Content-Type: audio/mpeg`、`Cache-Control: no-cache`、CORS `*`、chunked 无 Content-Length。
- 无 Range 支持（始终 200 全流）。
- **切歌（skip/next/prev）**：服务端 resync 读者并**关闭当前连接**——客户端应监听 `ended`/`error` 重连到 live edge（前端实现：`src/audio/streamAudio.ts`，重连带 `?r=` 防缓存参数）。
- 停止（stop）：缓冲清空但不断连，读者空闲至数据恢复或 60s 空闲超时。
- 外部播放器（VLC/mpv/ffplay）可直接播放 `http://host:2241/stream`。

---

## 4. 配置（`config.toml` + 环境变量）

| 段 | 键 | 环境变量 |
|----|----|----------|
| server | host、port（2241）、base_path（/） | `RADIO_SERVER_PORT`、`RADIO_BASE_PATH` |
| database | url（sqlite） | `RADIO_DATABASE_URL` |
| audio_engine | media_path、stream_base（auto/相对/绝对）、crossfade_enabled | `RADIO_MEDIA_PATH`、`RADIO_STREAM_BASE`、`RADIO_CROSSFADE_ENABLED` |
| device | cookie_max_age_days、admin_setup_token | `RADIO_ADMIN_SETUP_TOKEN` |
| station | name、short_name、subtitle、description、icon_path | `RADIO_STATION_NAME` |
| logging | level | `RADIO_LOG_LEVEL` |
| ncm | device_id、download_concurrency | `RADIO_NCM_DEVICE_ID`、`RADIO_NCM_DOWNLOAD_CONCURRENCY` |

- `GET /api/admin/settings` → `{station_name, short_name, subtitle, description, icon_url, icon_path, resolved_icon_url}`；
  `POST` body 可含上述字段（写 `config.toml`，**需重启生效**）。
- `.env.example` 已与上表对齐（`RADIO_REDIS_URL`/`RADIO_JWT_SECRET` 已移除——代码不再读取）。

---

## 5. 兼容性红线（改后端前必读）

1. **REST 包装**：`{success, data?, error?}` 结构、四个裸 JSON 端点、字段命名（camelCase 保持现状）不可改。
2. **WS 判别与字段名**：`type` 值、`playback_state` 全部字段名、`lyrics_lines` 元素 `{time_ms, text}`、`pong` 明文应答不可改。
3. **歌词三态语义**：`[]`（无歌词）/ `null`（未重发）/ 数组（全量）不可混淆。
4. **/stream 行为**：no-cache、无 Range、切歌关连接（前端靠 `ended` 重连）。
5. **新连接补发全量帧**：前端依赖此机制获取重连后的歌词。
6. **`SongSummary` 嵌入响应**（队列等）必须携带真实 `song_id` 与 `has_lyrics`/`has_cover`。
7. `/site-icon`、`/api/songs/:id/cover` 在无资源时必须返回 **200 占位**，不得 404。
