# ARCHITECTURE — RakurakuMusicWorld

> 本文记录**当前真实架构**，全部结论来自实际代码（file:symbol 标注）。
> 目标架构（Logical Side / Physical Side 分离）：`radio-backend/src/world.rs:WorldRuntime` 是 Logical Side 入口；`radio-backend/src/physical/mod.rs` 定义了四大 Physical Side trait（`PhysicalStorage` / `PhysicalTransport` / `PhysicalPlayerRegistry` / `PhysicalLifecycle`），`IntegratedPhysicalSide` 实现全部 trait 并委托给 `AppState`。`world_id`（UUIDv4）持久化于 `world_meta` 表。
>
> 状态标记：**[Existing]** 已存在｜**[Partial]** 部分存在，有缺口｜**[Missing]** 不存在｜**[Needs Refactor]** 存在但结构阻碍目标架构

---

## 1. 当前总体形态

```mermaid
flowchart LR
    subgraph Desktop["Electron Shell (electron/)"]
        main["main.mjs — BrowserWindow"]
        preload["preload.mjs — 空 API"]
    end
    subgraph Client["Web Client (radio-backend/frontend)"]
        ui["React 19 SPA (zustand store)"]
        audio["<audio> → /stream"]
    end
    subgraph Server["radio-backend 单进程 (axum :2241)"]
        routes["HTTP/WS 路由层"]
        services["services/ 业务逻辑"]
        engine["radio-engine (内嵌库)"]
        sqlite[("SQLite")]
        ring[("RingBuffer 512KB")]
    end
    main -->|"loadURL / loadFile"| ui
    ui -->|"REST + WS"| routes
    audio -->|"/stream"| ring
    routes --> services
    services --> engine
    services --> sqlite
    engine --> ring
```

- 单进程单端口：`radio-backend` 同时服务 REST `/api`、WebSocket `/ws`、音频 `/stream`、静态前端（`routes/mod.rs:build_router`）。亦可通过 `rakuraku-music-world-server` 或 `--headless` 独立以 Dedicated Server 模式运行（无静态文件依赖，适合 Linux 服务器 / systemd 部署）。
- Electron 壳与 Web Client：既可作为本地单机界面，亦可在设置页配置 Remote Server 连接远程 Dedicated World。
- **Logical/Physical Side 分离已就绪**：`WorldRuntime` 是 Logical Side 入口；`physical/mod.rs`（`PhysicalStorage` / `PhysicalTransport` / `PhysicalPlayerRegistry` / `PhysicalLifecycle`）已将物理持久化、流传输、听众注册与无头生命周期抽象为独立 trait。

## 2. 分层现状对照

### 2.1 Client（前端 + 桌面壳）

| 组件 | 位置 | 状态 | 说明 |
| --- | --- | --- | --- |
| UI | `frontend/src/pages/`、`components/` | [Existing] | 播放器 / 曲库 / 设置 三页（`router.tsx`），管理面板内嵌在 Settings；设置页包含 `ServerSection` 供连接远程 World 与 World Browser 局域网浏览 |
| Client State | `frontend/src/store.ts` (zustand) | [Needs Refactor] | 单 store 混合了服务器状态镜像（playback/queue/listeners）、本地 UI 状态（volume/accent/toast）、本地持久化（favorites 走 localStorage，`store.ts:toggleFavorite`） |
| Network | `api/client.ts`、`api/index.ts`、`api/ws.ts` | [Existing] | REST 包装解包 + WS 分发；`appRoot`/`wsUrl` 动态支持配置远程 World；`credentials: 'include'` 与 `x-device-token` 保证跨域身份识别；提供 `discovery` REST API 查询局域网世界 |
| Audio 输出 | `audio/streamAudio.ts` | [Existing] | 单例 `<audio>` → `/stream`；动态基于 `appRoot()` 支持远程流；切歌重连（`?r=` nonce）、指数退避、停滞看门狗 |
| 位置平滑 | `hooks/usePlaybackClock.ts` | [Existing] | 用 `position_ms + (Date.now() - timestamp_ms)` 客户端外推；**这是 World Clock 思路的雏形，但时间基准是墙钟而非服务器时钟** |
| 桌面壳 | `electron/main.mjs` | [Existing] | 窗口 1440×900（双栏 xl 断点需要 ≥1280 CSS px）、外部链接走系统浏览器 |
| Preload API | `electron/preload.mjs` | [Existing]（空） | 无任何 contextBridge 暴露 |

### 2.2 服务端（radio-backend）

| 组件 | 位置 | 状态 | 说明 |
| --- | --- | --- | --- |
| 路由/HTTP 适配 | `routes/` | [Existing] | 纯适配层：解析请求 → 调 service → JSON 响应。无业务逻辑内嵌；支持 headless 模式（免除 `static/` 依赖，根路径暴露服务信息）；暴露 `/api/discovery/*` 端点 |
| Logical Side 入口 | `world.rs:WorldRuntime` | [Partial] | routes 只通过 World command/query 访问播放与队列；运行时暂复用旧 queue service 和 engine |
| 播放状态权威 | `world.rs` + `services/playback_broadcast.rs` | [Existing] | PlaybackState 权威提升至 Logical 层（`AppState.current_playback` + `WorldRuntime::now_playing`），engine 仅上报物理进度 |
| 队列权威 | `services/queue/` + `queue_items` 表 | [Partial] | DB 是 pending 队列唯一权威；engine request_queue 已降级为执行细节；`queue_sync` 互斥锁与同步语义完全收进 Logical 层 |
| 听众注册 | `app/state.rs:listeners` (DashMap) | [Existing] | 内存态，WS 连接注册 / 断开移除，不持久化 |
| 局域网发现 | `services/discovery.rs` + `tokio::net::UdpSocket` | [Existing] | 纯异步 UDP 广播发现层，Beacon 心跳广播/接收 + Probe 主动探测 + 30s TTL 自动清理 |
| WS 广播 | `services/playback_broadcast.rs` + `websocket.rs` | [Existing] | 500ms 轮询 engine → enrich → broadcast；心跳 ping/pong（30s/60s 超时，`websocket.rs:handle_socket`） |
| 歌词快照缓存 | `services/playback_snapshot.rs` | [Existing] | 切歌时解析 .lrc（GBK/UTF-16 容错），全量帧缓存于 `AppState.ws_full_snapshot` 供新连接补发 |
| 持久化 | `db.rs` + `migrations/001..010` | [Existing] | users/songs/playlists/queue_items/play_history/admin_log/favorites/user_requests/ncm_import_tasks/metadata_jobs/world_meta |
| 配置 | `config.rs` + `config.toml` | [Existing] | `POST /api/admin/settings` 原子写回 config.toml（`routes/admin/settings.rs:write_config_atomically`），**不热加载**；支持 `[discovery]` 节 |

### 2.3 音频引擎（radio-engine）

| 组件 | 位置 | 状态 | 说明 |
| --- | --- | --- | --- |
| 播放主循环 | `player.rs:run()` | [Existing] | request 优先、folder cycle 兜底；命令经 `AudioCommandType`（Skip/Next/Prev/Play/Stop/ReloadQueue） |
| 状态发布 | `player.rs:publish_state` | [Existing] | 500ms 节流，`PlaybackState{playlist_index, file_path, position_ms, duration_ms, status, track_start_timestamp_ms, song_id, ...}`（`types.rs:70`） |
| 音频管道 | `player.rs:stream_track` | [Existing] | ffmpeg 子进程 decode→libmp3lame 128k→stdout→RingBuffer |
| 环形缓冲 | `ring_buffer.rs` | [Existing] | 单写多读，`clear_and_resync_readers()` 实现切歌时直播边缘重同步 |

## 3. 控制流：一次"切歌"的实际路径

```
Admin UI → POST /api/admin/playlist/next (routes/admin/playback.rs)
        → WorldRuntime::dispatch(Skip)       [Logical command]
        → PlayerHandle::send_command(Skip)   [Integrated Physical Side]
        → player.rs:run() 消费命令, stream_track 结束
        → ring_buffer.clear_and_resync_readers()  [/stream 客户端连接被关闭]
        → 客户端 <audio> ended → streamAudio.reconnect() 回直播边缘
        → player.rs:publish_state → playback_broadcast 500ms 轮询
        → playback_snapshot.build_message (歌词全量帧)
        → ws_tx.broadcast → websocket.rs:handle_socket → 浏览器 store.applyPlaybackState
```

队列 REST 也先进入 `WorldRuntime`，再调用现有 `services/queue.rs` 的持久化/执行适配；
这保留行为不变，同时给后续替换 Logical playlist 实现留下单一入口。

结论：Command 与 State/Event 已有稳定的迁移接缝；Physical Side trait 边界（`physical/mod.rs`）已定义并由 `IntegratedPhysicalSide` 实现。

## 4. 映射到目标架构：抽取候选

| 现有模块 | 目标角色 | 判定 |
| --- | --- | --- |
| `world.rs:WorldRuntime` | Logical Side 的 Command/Query 入口 | [Partial] | 负责 World command 翻译、队列操作入口与快照查询；底层暂复用现有实现 |
| `services/queue/` (`rules` + `persistence`) | Logical playlist 规则 + Physical persistence adapter | [Existing] 规则/存储分层与双队列归一已落地，锁与执行器同步完全收进 Logical 层 |
| `services/playback_broadcast.rs` + `playback_snapshot.rs` | Logical 播放聚合 + Event 派发 + Snapshot 生成 | [Existing] | Logical 层聚合权威状态，驱动 500ms 广播并维系全量歌词快照 |
| `auth.rs`（device identity）+ `state.rs:listeners` | World 的 Player（身份/在线） | [Existing] 概念已存在，缺 worldId 维度 |
| `models/ws.rs:WsMessage` | Protocol 层 | [Needs Refactor] 内嵌 URL/歌词等富数据，UI 直接消费服务器格式 |
| `http/stream.rs` + `ring_buffer.rs` | Physical Side 的音频传输细节 | [Existing] 应整体留在 Physical Side |
| `physical/mod.rs` | Physical Side trait 边界 + Integrated 实现 | [Existing] 四大 trait（Storage/Transport/PlayerRegistry/Lifecycle）+ `IntegratedPhysicalSide` 委托给 AppState |
| `services/discovery.rs` + `routes/discovery.rs` | LAN Discovery 与 World Browser 支撑 | [Existing] 纯异步 UDP 广播发现层，与 World Protocol 解耦，为 World Browser 提供局域网世界列表与即时扫描 |
| `config.rs` + `config.toml` + `world_meta` 表 | World metadata/persistence | [Existing] `world_id` (UUIDv4) 持久化于 `world_meta` 表，首次启动生成，跨重启稳定；station.name 保留为显示名 |
| `websocket.rs:handle_socket` ping/pong + `usePlaybackClock` | World Clock | [Partial] 有 timestamp 同步雏形，无 offset/延迟估计 |

## 5. 明确的边界（不要动的部分）

- `RingBuffer` 容量必须 2 的幂（524288）；`/stream` 背压依赖有界 channel。
- `/stream` 切歌关连接、前端 `ended` 重连——这是直播流的正确语义，迁移时保持。
- 歌词三态（`[]`/`null`/数组）是前端强依赖契约。
- ffmpeg 管道与 keepalive bind（`bootstrap.rs:bind_with_keepalive`）是运维稳定性关键。
