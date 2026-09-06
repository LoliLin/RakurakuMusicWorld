#!/bin/bash
# =============================================================================
# RakurakuMusicWorld - Release Build Script
# =============================================================================
#
# Usage:
#   ./build_release.sh              # 完整构建：前端 + Rust 后端
#   ./build_release.sh --skip-frontend  # 跳过前端构建（仅编译后端）
#
# 产物输出到 dist/，保留已有的 dist/media 与 dist/data 数据。
# =============================================================================

set -euo pipefail

# ─── 颜色与日志 ───────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status()  { echo -e "${BLUE}[*]${NC} $1"; }
print_success() { echo -e "${GREEN}[✓]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[!]${NC} $1"; }
print_error()   { echo -e "${RED}[✗]${NC} $1"; }

# ─── 解析参数 ─────────────────────────────────────────────────
SKIP_FRONTEND=0
for arg in "$@"; do
    case "$arg" in
        --skip-frontend) SKIP_FRONTEND=1 ;;
        *) print_warning "未知参数: $arg" ;;
    esac
done

# ─── 路径常量 ─────────────────────────────────────────────────
ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKEND_DIR="$ROOT_DIR/radio-backend"
FRONTEND_DIR="$BACKEND_DIR/frontend"
DIST_DIR="${RADIO_DIST_DIR:-$ROOT_DIR/dist}"
# Allow CI, desktop sandboxes, and read-only historical target directories to
# choose a writable Cargo target location without changing the deploy layout.
TARGET_DIR="${CARGO_TARGET_DIR:-$BACKEND_DIR/target}"
VERSION="3.0"

echo -e "${BLUE}
══════════════════════════════════════════════
    RakurakuMusicWorld v${VERSION} 构建工具
══════════════════════════════════════════════${NC}
"

# ─── 环境检查 ─────────────────────────────────────────────────
print_status "检测系统环境..."

# Rust 工具链（必需）
if command -v cargo &>/dev/null; then
    print_success "Rust 工具链: $(rustc --version)"
else
    print_error "未找到 Rust 工具链，请先安装:"
    print_error "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 运行时依赖 ffmpeg / ffprobe（仅检查，不强制安装）
for dep in ffmpeg ffprobe; do
    if command -v "$dep" &>/dev/null; then
        print_success "找到 $dep"
    else
        print_warning "$dep 未找到，运行时需要它来播放/扫描音频"
    fi
done

# ─── 前端构建 ─────────────────────────────────────────────────
if [ "$SKIP_FRONTEND" -eq 0 ]; then
    print_status "构建前端 (React 19 + Vite)..."
    cd "$FRONTEND_DIR"

    if ! command -v npm &>/dev/null; then
        print_error "未找到 npm，无法构建前端；请先安装 Node.js。"
        exit 1
    fi

    print_status "安装 npm 依赖..."
    npm ci

    npm run build
    print_success "前端构建完成，产物已写入 radio-backend/static/"
    cd "$ROOT_DIR"
else
    print_warning "已跳过前端构建 (--skip-frontend)"
fi

# ─── Rust 后端构建 ────────────────────────────────────────────
print_status "构建 Rust 后端（含音频引擎）..."
cd "$BACKEND_DIR"
cargo build --release --target-dir "$TARGET_DIR"
print_success "Rust release 构建完成"
cd "$ROOT_DIR"

# ─── 准备 dist 目录 ───────────────────────────────────────────
print_status "准备部署文件..."
mkdir -p "$DIST_DIR/data" "$DIST_DIR/media"

# 复制二进制文件（先写临时文件再替换，避免覆盖运行中的二进制触发 ETXTBSY）
cp "$TARGET_DIR/release/radio-backend" "$DIST_DIR/radio-backend.new"
mv -f "$DIST_DIR/radio-backend.new" "$DIST_DIR/radio-backend"
chmod +x "$DIST_DIR/radio-backend"
print_success "radio-backend 二进制文件已复制"

# 复制前端静态文件
if [ -d "$BACKEND_DIR/static" ]; then
    rm -rf "$DIST_DIR/static"
    cp -r "$BACKEND_DIR/static" "$DIST_DIR/"
    print_success "前端静态文件已复制"
else
    print_warning "未找到 radio-backend/static/，请先构建前端"
fi

# 复制默认配置（仅在不存在时，避免覆盖用户已修改的配置）
if [ ! -f "$DIST_DIR/config.toml" ]; then
    cp "$BACKEND_DIR/config.toml.example" "$DIST_DIR/config.toml"
    print_success "默认 config.toml 已复制"
else
    print_success "保留已有 config.toml"
fi

# media/ 和 data/ 目录会被保留
print_success "dist/ 目录准备完毕（media/ 与 data/ 数据已保留）"

# ─── 生成启动/停止脚本 ────────────────────────────────────────
print_status "生成启动/停止脚本..."

cat > "$DIST_DIR/start.sh" << 'STARTEMBED'
#!/bin/bash
cd "$(dirname "$0")"

echo "启动 RakurakuMusicWorld..."

if [ -f .server.pid ] && kill -0 $(cat .server.pid) 2>/dev/null; then
    echo "服务器已在运行中 (PID $(cat .server.pid))"
    exit 1
fi

nohup ./radio-backend > server.log 2>&1 &
PID=$!
echo $PID > .server.pid
echo "服务器已启动 (PID $PID)"
echo "日志文件: server.log"
echo "访问 http://localhost:2241"
STARTEMBED
chmod +x "$DIST_DIR/start.sh"

cat > "$DIST_DIR/stop.sh" << 'STOPEMBED'
#!/bin/bash
cd "$(dirname "$0")"

echo "停止 RakurakuMusicWorld..."

if [ -f .server.pid ]; then
    PID=$(cat .server.pid)
    if kill -0 $PID 2>/dev/null; then
        kill $PID
        echo "服务器已停止 (PID $PID)"
        rm -f .server.pid
    else
        echo "PID $PID 对应的进程已不存在"
        rm -f .server.pid
    fi
else
    echo "未找到 PID 文件"
fi

# 清理残留进程
pkill -f "radio-backend" 2>/dev/null && echo "清理了残留的 radio-backend 进程" || true
STOPEMBED
chmod +x "$DIST_DIR/stop.sh"

print_success "启动/停止脚本已生成"

# ─── 完成 ─────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}══════════════════════════════════════════════${NC}"
echo -e "${GREEN}  构建完成！${NC}"
echo ""
echo "  使用方法:"
echo "    cd dist"
echo "    ./start.sh   # 启动服务"
echo "    ./stop.sh    # 停止服务"
echo ""
echo "  首次使用:"
echo "    1. 编辑 dist/config.toml 设置 admin_setup_token"
echo "    2. 将音频文件放入 dist/media/"
echo "    3. ./start.sh"
echo "    4. 访问 http://localhost:2241"
echo "    5. 点击导航栏 🔑 按钮，输入 admin_setup_token 获取管理员权限"
echo "    6. 在管理后台配置网易云 Cookie (MUSIC_U) 以启用批量下载"
echo -e "${GREEN}══════════════════════════════════════════════${NC}"
