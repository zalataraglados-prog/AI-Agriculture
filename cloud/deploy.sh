#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="cloud"
APP_NAME="ai-agri-cloud-receiver"

INSTALL_ROOT="${INSTALL_ROOT:-/opt/ai-agriculture/cloud}"
SERVICE_NAME="${SERVICE_NAME:-ai-agri-cloud-receiver}"
BIND_ADDR="${BIND_ADDR:-0.0.0.0:9000}"
CONFIG_PATH="${CONFIG_PATH:-${INSTALL_ROOT}/config/sensors.toml}"
OVERWRITE_CONFIG="${OVERWRITE_CONFIG:-0}"
STATIC_SOURCE_FRONTEND="${STATIC_SOURCE_FRONTEND:-${SCRIPT_DIR}/../frontend}"
STATIC_SOURCE_DASHBOARD="${STATIC_SOURCE_DASHBOARD:-${SCRIPT_DIR}/dashboard}"
STATIC_TARGET_FRONTEND="${STATIC_TARGET_FRONTEND:-${INSTALL_ROOT}/frontend}"
STATIC_TARGET_DASHBOARD="${STATIC_TARGET_DASHBOARD:-${INSTALL_ROOT}/dashboard}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[deploy] ERROR: cargo not found. Please install Rust first."
  exit 1
fi

if [[ "$EUID" -ne 0 ]]; then
  SUDO="sudo"
else
  SUDO=""
fi

echo "[deploy] Building release binary..."
cd "$SCRIPT_DIR"
cargo build --release

echo "[deploy] Installing binary to ${INSTALL_ROOT} ..."
$SUDO mkdir -p \
  "${INSTALL_ROOT}/bin" \
  "${INSTALL_ROOT}/log" \
  "$(dirname -- "${CONFIG_PATH}")" \
  "${INSTALL_ROOT}/sql/migrations" \
  "${STATIC_TARGET_FRONTEND}" \
  "${STATIC_TARGET_DASHBOARD}"
$SUDO cp "target/release/${BIN_NAME}" "${INSTALL_ROOT}/bin/${APP_NAME}"
$SUDO chmod +x "${INSTALL_ROOT}/bin/${APP_NAME}"
echo "[deploy] Syncing sql migrations to ${INSTALL_ROOT}/sql/migrations"
$SUDO cp -r "sql/migrations/"* "${INSTALL_ROOT}/sql/migrations/"

if [[ ! -f "${CONFIG_PATH}" ]]; then
  echo "[deploy] Config not found, installing default config to ${CONFIG_PATH}"
  $SUDO cp "config/sensors.toml" "${CONFIG_PATH}"
elif [[ "${OVERWRITE_CONFIG}" == "1" ]]; then
  echo "[deploy] OVERWRITE_CONFIG=1, replacing ${CONFIG_PATH}"
  $SUDO cp "config/sensors.toml" "${CONFIG_PATH}"
else
  echo "[deploy] Keeping existing config at ${CONFIG_PATH} (set OVERWRITE_CONFIG=1 to replace)"
fi

if [[ -d "${STATIC_SOURCE_FRONTEND}" ]]; then
  echo "[deploy] Syncing frontend to ${STATIC_TARGET_FRONTEND}"
  if command -v rsync >/dev/null 2>&1; then
    $SUDO rsync -a --delete "${STATIC_SOURCE_FRONTEND}/" "${STATIC_TARGET_FRONTEND}/"
  else
    $SUDO rm -rf "${STATIC_TARGET_FRONTEND:?}/"*
    $SUDO cp -a "${STATIC_SOURCE_FRONTEND}/." "${STATIC_TARGET_FRONTEND}/"
  fi
else
  echo "[deploy] WARNING: frontend/rice not found at ${STATIC_SOURCE_FRONTEND}"
fi

if [[ -d "${STATIC_SOURCE_DASHBOARD}" ]]; then
  echo "[deploy] Syncing dashboard fallback to ${STATIC_TARGET_DASHBOARD}"
  if command -v rsync >/dev/null 2>&1; then
    $SUDO rsync -a --delete "${STATIC_SOURCE_DASHBOARD}/" "${STATIC_TARGET_DASHBOARD}/"
  else
    $SUDO rm -rf "${STATIC_TARGET_DASHBOARD:?}/"*
    $SUDO cp -a "${STATIC_SOURCE_DASHBOARD}/." "${STATIC_TARGET_DASHBOARD}/"
  fi
fi

if command -v systemctl >/dev/null 2>&1; then
  echo "[deploy] Configuring systemd service ${SERVICE_NAME} ..."
  $SUDO tee "/etc/systemd/system/${SERVICE_NAME}.service" >/dev/null <<EOF
[Unit]
Description=AI Agriculture Cloud Receiver
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${INSTALL_ROOT}/bin/${APP_NAME} --config ${CONFIG_PATH} --bind ${BIND_ADDR} --timeout-ms 0
Restart=always
RestartSec=2
WorkingDirectory=${INSTALL_ROOT}
StandardOutput=append:${INSTALL_ROOT}/log/receiver.log
StandardError=append:${INSTALL_ROOT}/log/receiver.err.log

[Install]
WantedBy=multi-user.target
EOF

  $SUDO systemctl daemon-reload
  $SUDO systemctl enable --now "${SERVICE_NAME}"
  echo "[deploy] Service started:"
  $SUDO systemctl --no-pager --full status "${SERVICE_NAME}" | sed -n '1,15p'
  echo "[deploy] Done."
else
  echo "[deploy] systemctl not found. Starting process with nohup..."
  nohup "${INSTALL_ROOT}/bin/${APP_NAME}" --config "${CONFIG_PATH}" --bind "${BIND_ADDR}" --timeout-ms 0 \
    > "${INSTALL_ROOT}/log/receiver.log" 2> "${INSTALL_ROOT}/log/receiver.err.log" &
  echo $! > "${INSTALL_ROOT}/cloud_receiver.pid"
  echo "[deploy] Started with PID $(cat "${INSTALL_ROOT}/cloud_receiver.pid")"
  echo "[deploy] Done."
fi
