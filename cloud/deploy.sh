#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="cloud"
APP_NAME="ai-agri-cloud-receiver"

INSTALL_ROOT="${INSTALL_ROOT:-/opt/ai-agriculture/cloud}"
SERVICE_NAME="${SERVICE_NAME:-ai-agri-cloud-receiver}"
BIND_ADDR="${BIND_ADDR:-0.0.0.0:9000}"
EXPECTED_PAYLOAD="${EXPECTED_PAYLOAD:-success}"

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
$SUDO mkdir -p "${INSTALL_ROOT}/bin" "${INSTALL_ROOT}/log"
$SUDO cp "target/release/${BIN_NAME}" "${INSTALL_ROOT}/bin/${APP_NAME}"
$SUDO chmod +x "${INSTALL_ROOT}/bin/${APP_NAME}"

if command -v systemctl >/dev/null 2>&1; then
  echo "[deploy] Configuring systemd service ${SERVICE_NAME} ..."
  $SUDO tee "/etc/systemd/system/${SERVICE_NAME}.service" >/dev/null <<EOF
[Unit]
Description=AI Agriculture Cloud Receiver
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${INSTALL_ROOT}/bin/${APP_NAME} --bind ${BIND_ADDR} --expected ${EXPECTED_PAYLOAD} --timeout-ms 0
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
  nohup "${INSTALL_ROOT}/bin/${APP_NAME}" --bind "${BIND_ADDR}" --expected "${EXPECTED_PAYLOAD}" --timeout-ms 0 \
    > "${INSTALL_ROOT}/log/receiver.log" 2> "${INSTALL_ROOT}/log/receiver.err.log" &
  echo $! > "${INSTALL_ROOT}/cloud_receiver.pid"
  echo "[deploy] Started with PID $(cat "${INSTALL_ROOT}/cloud_receiver.pid")"
  echo "[deploy] Done."
fi
