<div align="center">

# RakurakuMusicWorld

**把一台普通设备变成所有人都能参与点歌、同步收听的音乐世界。**

Rust 音频引擎、Web 后端、React 前端、Electron 桌面端与 Android 客户端统一架构：一个服务即可提供网页、实时状态、逐行歌词、连续 MP3 音频流以及局域网世界互联。

[![License: MIT](https://img.shields.io/badge/license-MIT-2f6f5e.svg)](LICENSE)
![Rust](https://img.shields.io/badge/backend-Rust-de6b35.svg)
![React 19](https://img.shields.io/badge/frontend-React_19-149eca.svg)
![SQLite](https://img.shields.io/badge/database-SQLite-0f80cc.svg)
![Electron](https://img.shields.io/badge/desktop-Electron-47848F.svg)
![Android](https://img.shields.io/badge/mobile-Capacitor_Android-3DDC84.svg)
[![CI](https://github.com/LoliLin/RakurakuMusicWorld/actions/workflows/ci.yml/badge.svg)](https://github.com/LoliLin/RakurakuMusicWorld/actions/workflows/ci.yml)

[快速开始](#快速开始) · [世界模型与多端架构](#minecraft-风格的世界模型) · [技术文档](docs/TECHNICAL.md) · [开发与构建](#构建与开发)

</div>

![浅色播放器：封面、同步歌词、点歌队列与常驻播放条](docs/screenshots/player-light.png)

---

## 核心特性

- 📻 **连续电台流**：内置 Rust 音频引擎驱动 `ffmpeg` 解码与高质重采样，写入内存共享环形缓冲，所有听众从 `/stream` 收听同一条低延迟 MP3 流。
- 🎶 **多人优先点歌**：浏览或搜索曲库即可点播歌曲；请求歌曲优先于基础轮播曲目，房主可自由切歌、调序与移出。
- ⚡ **实时状态同步**：WebSocket 每 500 ms 广播播放进度与曲目元数据，前端平滑对其毫秒级时间戳；弱网或断线自动无缝降级为轮询。
- 📝 **逐行歌词与动态封面**：自动扫描同名 `.lrc`、旁路封面或内嵌音频标签；切歌时全量同步，播放中平滑滚动。
- 🔍 **智能元数据补全**：优先提取本地音频标签；缺少信息时可由房主一键匿名匹配网易云候选，为歌曲自动补齐封面与专辑名。
- 👑 **极简玩家身份与安全模型**：
  - **开箱即用**：无需繁琐的账号密码注册，玩家拥有独立的设备身份与昵称，随时随地行内一键快速改名。
  - **本地用户即房主 (Host is Admin)**：基于底层操作系统内核 TCP 回环（Loopback）鉴权，本地主机访问者自动获得最高管理员权限 (OP)。
  - **彻底杜绝远程提权**：彻底移除网络提权接口与令牌验证，远程或局域网访问者只能作为普通听众，杜绝被恶意侵入的风险。
- 🌐 **多端覆盖**：
  - **Web / PWA**：自适应桌面与移动端屏幕，支持深浅色与动态主题色推导。
  - **Desktop (Electron)**：Windows / macOS / Linux 桌面客户端，启动时自动静默探测并拉起后台引擎（无黑框 Sidecar），数据独立存储。
  - **Android (Capacitor)**：一键构建 APK 安装包，支持随身收听与局域网世界加入。

---

## Minecraft 风格的世界模型

RakurakuMusicWorld 采用了类似 Minecraft 的“单人 / 局域网 / 多人服务器”交互设计：

```
                    ┌─────────────────────────┐
                    │      世界选择与玩家设置      │
                    └────────────┬────────────┘
         ┌───────────────────────┼───────────────────────┐
         ▼                       ▼                       ▼
  🏠 本机单人世界           📡 局域网广播世界          🌐 多人直接连接
 (Local Host World)        (LAN UDP Discovery)      (Direct Connect)
 • 本机内置 Sidecar 运行    • 自动扫描局域网房间      • 输入 IP / 域名连接
 • 本地用户即房主 (OP)       • 一键加入好友电台        • 自动保存最近历史
```

- **🏠 单人世界 (Singleplayer)**：运行在本地的独立音乐世界。桌面客户端双击即可自启动，本地用户自动拥有全部曲库管理、歌曲上传与电台配置权限。
- **📡 局域网世界 (LAN Worlds)**：开启局域网广播后，同一 Wi-Fi 或局域网内的其它设备打开客户端，即可自动发现正在运行的电台房间，一键加入收听与点歌。
- **🌐 直接连接 (Direct Connect)**：可输入任意远程公网服务器地址（如 `http://192.168.1.100:2241` 或自定义域名），快速加入多人电台。

---

## 界面一览

播放器以当前曲目为中心：桌面端同时显示歌词与队列，移动端用滑动分页在播放器和队列之间切换。底部迷你播放器在所有页面保持可用。

深色主题保留相同的信息密度与操作路径，可在设置中手动选择，也可以跟随系统外观自动切换。

![深色播放器：当前曲目、逐行歌词与点歌队列](docs/screenshots/player-dark.png)

设置页集中管理设备身份、个性化外观和管理员功能。主题支持浅色、深色和跟随系统，种子色会生成相应的整套界面配色。

![设置页：设备资料、主题模式与颜色选择](docs/screenshots/settings-theme.png)

---

## 快速开始

### 方式 A：一键打包全部产物 (推荐)

如果你在 Windows 环境下，只需运行仓库根目录的 `build_all.ps1`，即可一键自动打包 Web、服务端、桌面客户端以及 Android APK：

```powershell
powershell -ExecutionPolicy Bypass -File .\build_all.ps1
```

构建完成后产物分布如下：
- `dist/`：独立运行的单可执行程序发布包（包含静态网页与默认配置）。
- `dist-desktop/`：免安装解压即用的桌面应用（`RakurakuMusicWorld.exe`）。
- `dist-android/`：Android 手机安装包（`RakurakuMusicWorld-debug.apk`）。

---

### 方式 B：Linux 一行脚本安装

适用于带 `systemd` 的 Debian/Ubuntu、Arch Linux 和 Fedora：

```bash
curl -fsSL https://raw.githubusercontent.com/LoliLin/RakurakuMusicWorld/main/install.sh | sudo bash
```

安装完成后：

```bash
# 放入音乐文件（支持 mp3, flac, wav, ogg, m4a, aac 等）
sudo cp /path/to/music/* /var/lib/rakuraku/media/
sudo systemctl restart rakuraku-music-world

# 查看实时运行日志
journalctl -u rakuraku-music-world -f
```

默认访问地址为 `http://服务器地址:2241`。本地访问自动拥有管理员权限。

---

### 方式 C：从源码构建服务端

依赖工具：Rust (2021 edition)、Node.js (>= 20)、`ffmpeg` 与 `ffprobe`。

```bash
git clone https://github.com/LoliLin/RakurakuMusicWorld.git
cd RakurakuMusicWorld

# Linux / macOS 下构建独立发布目录
./build_release.sh

# 放入音乐并启动
cp /path/to/music/* dist/media/
cd dist
./start.sh
```

停止服务：
```bash
cd dist && ./stop.sh
```

---

## 构建与开发

### Web & API 本地开发

```bash
# 终端 1：启动 Rust 后端服务
cd radio-backend && cargo run

# 终端 2：启动 Vite 前端热重载开发服务器 (代理 /api, /ws, /stream 至 :2241)
cd radio-backend/frontend && npm run dev
```

### 桌面端开发与运行

```bash
# 自动启动内置 Sidecar 并打开 Electron 窗口
node electron/run-desktop.mjs
```

### Android 移动端构建

```powershell
# 需要提前安装 Android SDK (Platform 35, Build-Tools 35.0.0, Java 21)
powershell -ExecutionPolicy Bypass -File .\build_android.ps1
```

---

## 页面与能力

| 页面 | 访问路径 | 核心能力 |
| --- | --- | --- |
| **播放器** | `/`、`/player` | 实时流播放、大唱片封面、逐行歌词高亮、在线听众列表、点歌队列与切歌 |
| **曲库** | `/library` | 检索全部歌曲、快捷加入播放队列、本地收藏标记、歌曲元数据查看 |
| **设置** | `/settings` | 玩家昵称快速修改、主题模式切换、种子色自定义、个人网易云账号绑定 |
| **世界选择** | 顶部徽章点击呼出 | 本地单机世界回切、局域网世界 UDP 实时扫描与一键加入、远程服务器连接 |

本地房主（管理员）在设置中可进入**电台管理面板**：
- **概览与统计**：曲目总数、听众趋势、播放统计与系统运行日志。
- **歌曲管理**：批量上传（支持 100MB 拖拽）、在线试听、删除与全库重新扫描。
- **元数据匹配**：全自动或单曲交互式网易云元数据比对、专辑封面及歌词补齐。
- **批量下载**：网易云单曲/歌单异步抓取下载并自动入库。
- **电台品牌配置**：站点名称、副标题、介绍与自定义 Favicon 图标上传。

---

## 技术架构

```
RakurakuMusicWorld/
├── radio-engine/          # 纯 Rust 音频引擎（无全局状态、单写多读环形缓冲、ffmpeg 解码管道）
├── radio-backend/         # Axum 0.7 + SQLite 单二进制后端（REST API、WebSocket 状态广播、静态托管）
│   └── frontend/          # React 19 + TypeScript + Tailwind v4 + Appica UI 单页应用
├── electron/              # Electron 桌面端外壳（Sidecar 子进程自动调度、静默托盘生命周期）
├── .github/workflows/     # GitHub Actions 跨平台 CI 工作流
└── docs/                  # 技术设计细节、网络通信协议与世界模型文档
```

详细的技术选型与协议规范可参阅 [技术文档](docs/TECHNICAL.md) 与 [WORLD_MODEL.md](docs/WORLD_MODEL.md)。

---

## License 与致谢

本项目以 [MIT License](LICENSE) 发布。

网易云相关实现参考 [Music163bot-Go](https://github.com/XiaoMengXinX/Music163bot-Go) 的 API 思路并在 Rust 中重构。感谢 [FFmpeg](https://ffmpeg.org/)、[Axum](https://github.com/tokio-rs/axum)、[React](https://react.dev/)、[Appica UI](https://appica.dev/)、[Vite](https://vite.dev/)、[Capacitor](https://capacitorjs.com/)、[Electron](https://www.electronjs.org/) 与 [SQLx](https://github.com/launchbadge/sqlx)。

灵感来源：《孤独摇滚！》中的伊地知虹夏。

### 人生致谢

Chinese Football 在《Win&Lose》的封底写过：

> 每个人都想成为赢家，想让自己付出的时间得到胜利的喜悦作为回报。
>
> 日复一日，我开始接受自己是一个失败者，也开始接受有些梦想注定会失败这个事实。我学会安慰自己：你拥有的是过程，至少你尝试过，收获在别处，你已经赢下了与自己的战斗。
>
> 那么就祝贺自己还算清醒吧。我没有在与他人竞争之后迷失于虚荣，也没有在与自己竞争之后沉溺于情绪。
>
> 只是我有时仍然会做梦，在其中一个梦里，我还没有抵达最终的结局。在某一个结局里，我最终成为了一个强大的人，而 Chinese Football 成为了中国摇滚的传奇。

对我来说，这个项目大概也是这样的心情。快要十八岁了，我还不是一个厉害的大人，也不敢说自己真的多么会写代码。这个项目里有许多求助、试错、重写、妥协和大模型留下的痕迹。

也许从传统意义上说，它并不是一个人独自完成的胜利。但收获在别处。至少我认真地想过自己想做什么，至少我把一个想法从混乱带到了可以运行、可以使用、可以告一段落的地方。至少我在怀疑自己的时候还是继续往前推了一点。至少在这个版本结束的时候，我可以承认：我没有真正成为某种意义上的赢家，但我也没有输给自己。

于是把这段话留在这里，当作这个项目的封底，也当作一份人生致谢。

我想特别感谢我的家人。他们一直给我前行的勇气，也是我成长的底气。我想感谢知夏、噗噗砰砰砰、lunatic、violet、鹤汣以及成玉河，感谢他们在千里之外的陪伴，排名不分先后。

我还想感谢雕佬，谢谢你请我吃了这么多餐饭，你简直是个天才。

我还想感谢 Chinese Football 乐队，下一个十年，我们一起冲出亚洲，走向世界！

最后，我想感谢 Cynun。谢谢 ta 让我重拾这份很久之前的计划。我们一直都在。

献给那些最终没有完全实现、但仍然照亮过我的梦。
