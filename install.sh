#!/usr/bin/env bash

set -euo pipefail

REPO_URL="${RAKURAKU_REPO:-https://github.com/Risaly-Noroki-Dev-Club/RakurakuMusicWorld.git}"
REF="${RAKURAKU_REF:-main}"
SERVICE_NAME="rakuraku-music-world"
INSTALL_DIR="${RAKURAKU_INSTALL_DIR:-/opt/rakuraku-music-world}"
DATA_DIR="${RAKURAKU_DATA_DIR:-/var/lib/rakuraku}"
CONFIG_DIR="${RAKURAKU_CONFIG_DIR:-/etc/rakuraku}"
USER_NAME="${RAKURAKU_USER:-radio}"
PORT="${RAKURAKU_PORT:-2241}"

need_root() {
  if [ "$(id -u)" -ne 0 ]; then
    echo "Please run as root, for example: curl -fsSL <url> | sudo bash"
    exit 1
  fi
}

install_packages() {
  if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    apt-get install -y git curl ca-certificates build-essential pkg-config libssl-dev ffmpeg nodejs npm
  elif command -v pacman >/dev/null 2>&1; then
    pacman -Sy --needed --noconfirm git curl base-devel pkgconf openssl ffmpeg nodejs npm
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y git curl gcc gcc-c++ make pkgconf-pkg-config openssl-devel ffmpeg nodejs npm
  else
    echo "Unsupported distribution. Install git, curl, Rust, Node.js/npm, ffmpeg, pkg-config and OpenSSL dev headers first."
    exit 1
  fi
}

install_rust() {
  if command -v cargo >/dev/null 2>&1; then
    return
  fi

  export CARGO_HOME="/root/.cargo"
  export RUSTUP_HOME="/root/.rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  export PATH="/root/.cargo/bin:$PATH"
}

ensure_user() {
  if ! id -u "$USER_NAME" >/dev/null 2>&1; then
    useradd --system --home-dir "$DATA_DIR" --shell /usr/sbin/nologin "$USER_NAME"
  fi
}

build_project() {
  local workdir
  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT

  git clone --depth 1 --branch "$REF" "$REPO_URL" "$workdir/src"

  (cd "$workdir/src/radio-backend/frontend" && npm ci && npm run build)
  (cd "$workdir/src/radio-backend" && cargo build --release)

  install -d "$INSTALL_DIR" "$INSTALL_DIR/static"
  install -m 0755 "$workdir/src/radio-backend/target/release/radio-backend" "$INSTALL_DIR/radio-backend"
  rm -rf "$INSTALL_DIR/static"
  cp -r "$workdir/src/radio-backend/static" "$INSTALL_DIR/static"

  install -d "$CONFIG_DIR" "$DATA_DIR/data" "$DATA_DIR/media"
  if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    install -m 0640 "$workdir/src/radio-backend/config.toml.example" "$CONFIG_DIR/config.toml"
    sed -i "s/port = 2241/port = $PORT/" "$CONFIG_DIR/config.toml"
  fi
}

install_service() {
  cat > "/etc/systemd/system/${SERVICE_NAME}.service" <<SERVICE
[Unit]
Description=RakurakuMusicWorld
After=network.target

[Service]
Type=simple
User=${USER_NAME}
Group=${USER_NAME}
WorkingDirectory=${DATA_DIR}
Environment=RADIO_CONFIG=${CONFIG_DIR}/config.toml
Environment=RADIO_LOG_LEVEL=info
ExecStart=${INSTALL_DIR}/radio-backend
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=${SERVICE_NAME}

NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=${DATA_DIR} ${CONFIG_DIR}
LimitNOFILE=4096

[Install]
WantedBy=multi-user.target
SERVICE

  chown -R "$USER_NAME:$USER_NAME" "$DATA_DIR"
  chown -R root:"$USER_NAME" "$CONFIG_DIR"
  chmod 0750 "$CONFIG_DIR"
  chmod 0660 "$CONFIG_DIR/config.toml"

  systemctl daemon-reload
  systemctl enable --now "$SERVICE_NAME"
}

need_root
install_packages
install_rust
export PATH="/root/.cargo/bin:$PATH"
ensure_user
build_project
install_service

echo "RakurakuMusicWorld is installed and running."
echo "Open: http://localhost:${PORT}"
echo "Config: ${CONFIG_DIR}/config.toml"
echo "Media: ${DATA_DIR}/media"
echo "Logs: journalctl -u ${SERVICE_NAME} -f"
