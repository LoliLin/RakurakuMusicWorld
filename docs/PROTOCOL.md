# PROTOCOL — 当前真实通信协议 v3

> 只记录**代码中实际实现**的协议。权威版本：仓库根 `RakurakuMusicWorld协议.md`（前后端契约红线）。
> 本文是其架构视角摘要 + 实现位置索引，供 Logical/Physical Side 拆分时界定"协议层应隔离什么"。

---

## 1. 传输与封装

| 层 | 实现 | 位置 |
| --- | --- | --- |
| REST | axum handlers；响应包装 `{success, data?, error?}`；4 个裸 JSON 例外 | `error.rs`、`routes/station.rs`、`frontend/src/api/client.ts:apiFetch` |
| WebSocket | 文本 JSON，`type` 字段判别（internally tagged） | `models/ws.rs:WsMessage` |
| 音频 | HTTP chunked `audio/mpeg` 直播流 | `http/stream.rs` + `radio-engine/src/stream.rs` |
| 身份 | httpOnly `device_token` Cookie（365d，SameSite=Lax） | `http/middleware.rs:device_cookie_middleware` |

## 2. WebSocket 消息（服务端 → 客户端，单向）

### 2.1 `playback_state` — 每 500ms（`services/playback_broadcast.rs`）

```json
{
  "type": "playback_state",
  "song_id": 64,            // DB 主键；folder-cycle 曲目为 -1
  "title": "…", "artist": "…",
  "position_ms": 123456, "duration_ms": 270582,
  "lyrics_line": 12,        // 当前行索引（可 null）
  "lyrics_lines": null,     // 见歌词契约
  "status": "playing",      // playing | stopped | crossfading
  "stream_url": "/stream", "file_url": "…", "cover_url": "…",
  "timestamp_ms": 1786000000000   // 服务器墙钟；客户端外推基准
}
```

字段定义：`models/ws.rs:17-31`。enrichment（song_id 解析、URL 组装、歌词附加）在 `services/playback_snapshot.rs:build_message`。

**歌词三态契约**（前端 `store.ts:applyPlaybackState` 强依赖）：
- 切歌首帧：`lyrics_lines = [{time_ms, text}, …]`（有歌词）或 `[]`（无歌词）
- 后续帧：`lyrics_lines = null`（不重发），只带 `lyrics_line`
- 新连接补发：`websocket.rs:112-127` 从 `AppState.ws_full_snapshot` 重放最近全量帧

### 2.2 其他消息（`models/ws.rs:32-47`）

| type | 字段 | 触发点 |
| --- | --- | --- |
| `queue_update` | `{action, song_title?, requested_by?, queue_size}` | `services/queue.rs` 各操作后 `websocket.rs:broadcast`；**通知而非状态**——前端据此回查 `GET /api/queue`（`store.ts:applyQueueUpdate`） |
| `notice` | `{message, level: info\|warning\|error}` | 连接欢迎语、系统提示 |
| `ping` | `{timestamp}` | 每 30s 心跳（`websocket.rs:162-176`） |
| `listeners_update` | `{count, names[]}` | WS 连接建立/断开（`websocket.rs:broadcast_listeners_update`） |

### 2.3 客户端 → 服务端

**唯一合法消息**：文本 `pong`（心跳应答，60s 超时断开，`websocket.rs:149-160`）。服务端不处理其他客户端消息。

## 3. REST 端点清单（按域）

实现索引：`routes/mod.rs:build_router`。完整字段级契约见根协议文档 §1。

| 域 | 端点 | 权限 | 备注 |
| --- | --- | --- | --- |
| 公开 | `GET /api/station` `GET /api/now-playing` `GET /api/listeners` `GET /manifest.json` `GET /site-icon` | 无 | 前三者为裸 JSON；`now-playing` 是 WS 断线时的 REST 兜底 |
| 歌曲 | `GET /api/songs` `GET /api/songs/:id` `…/cover` `…/download` | 无 | cover 无资源时 200 占位 SVG（红线 7） |
| 队列 | `GET/POST /api/queue`、`DELETE /api/queue/:id`、`POST /api/queue/:id/move`、`POST /api/queue/skip`、`GET /api/queue/history` | 读无 / 写 Device / 管理 Admin | POST 限流 3/300s + 冷却 60s（`services/queue.rs:check_rate_limit/check_cooldown`） |
| 认证 | `GET /api/auth/me`、`POST /api/auth/name`、`POST /api/auth/claim-admin`、`POST /api/admin/logout` | Device | 提权需 `admin_setup_token` |
| 收藏/歌单 | `GET/POST /api/favorites…`、`/api/playlists…` | Device | 前端当前仅用 localStorage 收藏（`store.ts:FAVORITES_KEY`）；API 保留 |
| NCM | `GET/POST /api/ncm`、`POST /api/ncm/test` | Device | 设备级 Cookie |
| 管理 | `/api/admin/*`（users/stats/logs/songs/upload/rescan/settings/download/metadata…） | Admin | 上传/入库后发 `ReloadQueue` |
| 音频 | `GET /stream` | 无 | 见 §4 |

## 4. `/stream` 行为语义（红线）

- `200 audio/mpeg`、`Cache-Control: no-cache`、无 Range、chunked 无 Content-Length（`http/stream.rs:stream_handler`）。
- 连接数上限 429 保护（`STREAM_CONNECTIONS`，`http/stream.rs:14`）；空闲 15s 回收。
- **切歌（skip/next/prev）**：`ring_buffer.clear_and_resync_readers()` 关闭现有连接 → 客户端 `ended`/`error` → 前端 `streamAudio.reconnect()` 带 `?r=nonce` 重连直播边缘（`streamAudio.ts:87-104`）。
- 停止：清缓冲不断连；前端停滞看门狗 8s 无数据强制重连（`streamAudio.ts:STALL_RECONNECT_MS`）。

## 5. 与目标 Protocol 层的差距

| 目标（Handshake/JoinWorld/Command/Event/Snapshot/Ping/Pong/Error） | 现状 |
| --- | --- |
| Handshake | [Missing] WS 连接即加入；身份由 Cookie 隐式携带 |
| JoinWorld / LeaveWorld | [Partial] 由 WS 连接生命周期隐式表达（listeners_update） |
| Command | [Missing] 客户端→服务器仅有 `pong`；所有操作走 REST |
| Event | [Partial] `WsMessage` 5 变体；粒度粗（无 TrackChanged/Seeked） |
| Snapshot | [Partial] 仅歌词全量帧补发；无全量 World Snapshot |
| Ping/Pong | [Existing] 30s/60s 心跳 |
| Error | [Partial] REST `AppError`→HTTP 状态；WS 无错误帧（断连即错误） |

## 6. UI 与协议的耦合点（拆分时需隔离）

1. 前端直接消费服务器字段名（snake_case）与内嵌 URL（`stream_url`/`file_url`/`cover_url`）——`types.ts`、`store.ts`。
2. `applyQueueUpdate` 的"通知→回查"模式 = 事件与状态未分离。
3. 歌词全量/增量帧的前端缓存逻辑（`store.ts:167-201`）属于 UI 优化，不该出现在协议层。
4. 客户端位置外推假设 `Date.now()` 与服务器 `timestamp_ms` 同纪元（`usePlaybackClock.ts`）——World Clock 需要显式 offset 模型。
