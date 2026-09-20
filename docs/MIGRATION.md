# MIGRATION — 从现状到 Music World

> 原则（蓝图 §20/§21）：渐进式；**旧功能未被新实现证明可替代前，不删除旧实现**；
> 每步以 Build → Run → Existing Backend → Regression 通过为闸门。
> 本文只列路径与验收，不写实现细节。

---

## Current → Target 总览

```text
Current:  Electron Shell + React SPA → axum Backend(=Logical+Physical 混杂) → engine/SQLite
Target:   Client(UI+Client API) → Protocol → Logical Side(World State) → Physical Side(integrated/dedicated/remote)
```

现状基线（本文档全部依据）：
- Electron 壳 [Existing]，业务零侵入
- 播放状态权威在引擎 `player.rs:publish_state`；队列双份（DB + 引擎内存）
- WS 单向广播 + `pong`；命令走 REST
- 单机 SQLite；无 worldId；无 LAN/Dedicated

---

## Stage 0 — 冻结契约（前置）

**做什么**：把根文档《RakurakuMusicWorld协议.md》§5 红线固化为回归测试清单（REST 包装、WS 字段名、歌词三态、/stream 行为、新连接补发、SongSummary 完整性、占位 200）。

**验收**：手工冒烟脚本化——`npm run build`、`cargo build`、起后端、浏览器 + VLC 验证播放/切歌/歌词/重连。当前无前端测试框架（AGENTS.md 测试约定），回归以脚本 + ring_buffer 内联测试为基线。

## Stage 1 — 定义 World State（纯新增，不改行为）

**做什么**：
1. 新增 `core/world.rs`（或 `radio-backend/src/world/`）：`WorldState { identity, playback: PlaybackState, playlist: PlaylistState, players }`——字段先用现有 DTO 翻译，**不发明新字段**。
2. `PlaybackState`（`radio-engine/src/types.rs`）与 `queue_items` 行之间的转换函数集中到一处。
3. `AudioCommandType` 映射为 Command 枚举（Skip/Next/Prev/Play/Stop/ReloadQueue 之外**不新增**）。

**明确不做**：不改路由、不改 WS 消息、不动 engine。

**验收**：现有冒烟全绿；新代码仅被现有服务调用。

## Stage 2 — 事件细化（向后兼容的 WS 扩展）

**做什么**：
1. `WsMessage` 新增变体（如 `TrackChanged`）——**保留全部现有 5 变体与字段**；前端 switch 新类型时 default 不变。
2. `queue_update` 的"通知→回查 REST"模式保留；新增事件只是多余信息源。
3. Snapshot：新 WS 连接补发从"歌词全量帧"扩展为"完整 playback_state + queue 元数据"（仍在现有 JSON 形态内）。

**验收**：旧前端连新后端零回归（字段只增不改）；新前端可渐进消费。

## Stage 3 — Logical Side 抽取（后端内部分层）

**已落地的进展**：
1. `radio-backend/src/world.rs:WorldRuntime` 成为 Logical Side 的 Command/Query 入口；routes 不再直接调用队列 service 或 `PlayerHandle`。
2. 播放控制、队列增删移动/跳过、启动 rehydrate、metadata/download 后的队列重载，都经 `WorldCommand` 或 World playlist query 进入。
3. `WorldRuntime::snapshot()` 提供统一 World snapshot query；`WorldState` 仍只翻译现有 engine/SQLite/listener 权威，不复制状态。
4. `services/queue` 完成分层重构：拆为 `services/queue/rules.rs`（Logical playlist 规则与编排）与 `services/queue/persistence.rs`（Physical persistence SQLite 适配器），保留原有 `services/queue::*` 契约不变。
5. **双队列归一**：engine 请求队列彻底降级为执行缓存与细节，`queue_sync` 互斥锁与 rehydrate 语义完全收敛进 Logical 层（`WorldRuntime::purge_song` 收回最后残留的路由级锁与执行器队列直接操作）。
6. **PlaybackState 权威上移**：engine 降级为上报物理进度（`EngineProgress`），Logical 聚合层（`AppState.current_playback` + `WorldRuntime::now_playing`）拥有唯一的播放状态权威，统一驱动 500ms 广播与 REST 查询。

**Stage 3 已全面完成**。接下来可进入 Stage 4（Physical Side 抽象，Integrated 第一）。

**风险点**：`player.rs:run()` 主循环与 500ms 发布节奏是稳定性核心；完整 Stage 3 仍需要 `ring_buffer` 内联测试 + 长跑冒烟（多客户端、反复切歌、重启续播）。

**当前验收闸门**：本次边界抽取必须保持协议、REST、`/stream` 行为不变；后续物理侧抽取前再做完整端点 diff。

---

## Stage 4 — Physical Side 抽象（Integrated 第一）

**做什么**：
1. 定义 Physical Side trait 边界：runtime lifecycle / 网络端点 / 持久化 / 客户端连接注册。
2. 现有 axum 进程实现为 **Integrated/Hosted Physical Side**（单机 = 本地 World，"打开 App 即用"）。
3. `config.toml` → World metadata：引入 `world_id`（UUID，首次启动生成，存 SQLite `world_meta` 表），station.name 保留为显示名。

**已落地的进展**：
1. 数据库迁移 `010_world_meta.sql` 新增 `world_meta` 表（KV 存储，persistent world identity）。
2. `db.rs:get_or_create_world_id` — 首次启动生成 UUIDv4 并持久化，后续调用幂等返回同一 ID。
3. `AppState.world_id` 在启动时从 DB 加载；`/api/station` 返回 `world_id` 字段。
4. `world.rs:WorldIdentity` 填入 `Some(state.world_id.clone())`。
5. `physical/mod.rs` 定义四大 trait 边界：`PhysicalStorage`、`PhysicalTransport`、`PhysicalPlayerRegistry`、`PhysicalLifecycle`；`IntegratedPhysicalSide` 实现全部 trait，直接委托给 `AppState`。
6. 单元测试验证 `world_id` 跨次调用幂等性与持久化稳定性、`IntegratedPhysicalSide` 实现全部 Physical 约束。

**Stage 4 已全面完成**。接下来可进入 Stage 5（Dedicated Server，无 GUI）。

**验收**：同一二进制既可被 Web Client 连（现状），也可被标记为"本地 World"启动；`world_id` 稳定跨重启。

## Stage 5 — Dedicated Server（无 GUI）

**做什么**：
1. `radio-backend` 增加 headless 运行模式（无静态前端依赖、日志为主）——复用同一 Logical Side。
2. 客户端可配置 Remote World 地址（Electron 设置页新增连接目标；Cookie 流程适配跨域或反代同域）。

**已落地的进展**：
1. `radio-backend/Cargo.toml` 新增双二进制目标：`radio-backend` 与专用入口 `rakuraku-music-world-server`。
2. `config.rs` 引入 `headless: bool` 配置与 `RADIO_HEADLESS` 环境变量支持，`main.rs` 支持 `--headless` 命令行参数。
3. `routes/mod.rs` 在 headless 模式下免除静态文件依赖，`GET /` 暴露结构化专用服务信息响应（包含 world_id、station_name、版本与各端点）。
4. `PhysicalLifecycle` trait 补充 `is_headless(&self)` 查询能力并在 `IntegratedPhysicalSide` 中落地。
5. 认证与跨域增强：`auth.rs` 提取器扩展支持 `X-Device-Token` 与 `Authorization: Bearer`，响应头暴露 `x-device-token`，`bootstrap.rs` CORS 支持凭据与 Token 头透传。
6. 前端远程 World 支持：`client.ts` 抽象 `appRoot` 与 `wsUrl`，支持根据存储的自定义地址动态连接；`streamAudio.ts` 正确解析远程音频流；设置页新增 `ServerSection` 支持远程连接测试、切换与重置。
7. `build_release.sh` 编译并复制两个二进制，生成专用 `start-server.sh` 启动脚本。

**Stage 5 已全面完成**。接下来可进入 Stage 6（LAN Discovery / Browser）。

**验收**：`rakuraku-music-world-server` 在无桌面 Linux/无静态资源目录下跑通；Web/Electron 客户端连 Dedicated 与连本机行为一致。

## Stage 6 — LAN（最后做）

蓝图 §14 的 World Browser / LAN Discovery 依赖 mDNS/UDP 广播——**协议与发现解耦**，本阶段只做发现层，World Protocol 不变。

---

## 不做什么（每阶段通用）

- P2P / CRDT / QUIC / WebRTC / 分布式一致性 / 账户系统 / 插件市场（蓝图 §22）
- 重写 React UI 或迁移框架；Android 方案推迟到 Core/Protocol 稳定后（蓝图 §18）
- 不机械搬目录匹配目标树（蓝图 §19）——按依赖边界逐步收敛

## 兼容性闸门（每阶段通用）

1. `cd radio-backend/frontend && npm run build`（tsc 严格模式）
2. `cargo build` + `cargo test ring_buffer`
3. 起后端 → Web Client 手工回归：播放/切歌/歌词/队列/收藏/管理
4. Electron 冒烟：`npm run electron:dev` 连真实后端
5. 协议文档与实现零 drift（根协议文档为红线）

## 当前位置

```
[██████████████████████] Stage 5 全部完成：Dedicated Server (无 GUI Headless 模式) + 客户端远程 World 连接与跨域认证
下一步：Stage 6（LAN Discovery 与 World Browser）
已完成前置：Rebrand、Electron 壳、依赖升级、协议文档与 Stage 1/2 兼容扩展、WorldRuntime 接缝、playlist 规则/存储分层、双队列归一、PlaybackState 权威上移、Physical 4 大 trait + Integrated 实现 + world_id、Dedicated Server + Remote World
```
