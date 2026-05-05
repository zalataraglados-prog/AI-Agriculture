#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="${INSTALL_ROOT:-/opt/ai-agriculture/cloud}"
SERVICE_NAME="${SERVICE_NAME:-openclaw-chat-adapter}"
SCRIPT_PATH="${SCRIPT_PATH:-$INSTALL_ROOT/scripts/openclaw_chat_adapter.py}"
CLOUD_TOOL_BASE_URL="${CLOUD_TOOL_BASE_URL:-http://127.0.0.1:8088/api/v1/openclaw/tools}"
CLOUD_TOOL_TIMEOUT_SEC="${CLOUD_TOOL_TIMEOUT_SEC:-5}"
CLOUD_TOOL_CONTEXT_MAX_CHARS="${CLOUD_TOOL_CONTEXT_MAX_CHARS:-12000}"
OPENCLAW_DEFAULT_PLANTATION_ID="${OPENCLAW_DEFAULT_PLANTATION_ID:-}"

if [[ "$EUID" -ne 0 ]]; then
  SUDO="sudo"
else
  SUDO=""
fi

$SUDO mkdir -p "$INSTALL_ROOT/scripts" "$INSTALL_ROOT/log"
$SUDO cp "scripts/openclaw_chat_adapter.py" "$SCRIPT_PATH"
$SUDO chmod +x "$SCRIPT_PATH"

$SUDO tee "/etc/systemd/system/${SERVICE_NAME}.service" >/dev/null <<EOF
[Unit]
Description=OpenClaw HTTP Chat Adapter
After=network-online.target openclaw-gateway.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=${INSTALL_ROOT}
Environment=CLOUD_TOOL_BASE_URL=${CLOUD_TOOL_BASE_URL}
Environment=CLOUD_TOOL_TIMEOUT_SEC=${CLOUD_TOOL_TIMEOUT_SEC}
Environment=CLOUD_TOOL_CONTEXT_MAX_CHARS=${CLOUD_TOOL_CONTEXT_MAX_CHARS}
Environment=OPENCLAW_DEFAULT_PLANTATION_ID=${OPENCLAW_DEFAULT_PLANTATION_ID}
ExecStart=/usr/bin/python3 ${SCRIPT_PATH} --host 127.0.0.1 --port 3000
Restart=always
RestartSec=2
StandardOutput=append:${INSTALL_ROOT}/log/openclaw_chat_adapter.log
StandardError=append:${INSTALL_ROOT}/log/openclaw_chat_adapter.err.log

[Install]
WantedBy=multi-user.target
EOF

$SUDO systemctl daemon-reload
$SUDO systemctl enable --now "$SERVICE_NAME"
$SUDO systemctl --no-pager --full status "$SERVICE_NAME" | sed -n '1,14p'

echo "[install] done"
