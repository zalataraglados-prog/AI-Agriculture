#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-8.134.32.223:9000}"
STATE_DIR="${STATE_DIR:-state}"
ACK_TIMEOUT_MS="${ACK_TIMEOUT_MS:-3000}"
BAUD_LIST="${BAUD_LIST:-9600}"
MODBUS_PORT="${MODBUS_PORT:-/dev/ttyUSB0}"
CONFIG_FILE="${CONFIG_FILE:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATEWAY_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${GATEWAY_DIR}"

echo "[run_gateway] target=${TARGET} state_dir=${STATE_DIR} modbus_port=${MODBUS_PORT}"

export GATEWAY_MODBUS_PORT="${MODBUS_PORT}"

ARGS=(run --target "${TARGET}" --state-dir "${STATE_DIR}" --ack-timeout-ms "${ACK_TIMEOUT_MS}")

if [[ -n "${CONFIG_FILE}" ]]; then
  ARGS+=(--config "${CONFIG_FILE}")
fi

ARGS+=(--baud-list "${BAUD_LIST}")

cargo run --release -- "${ARGS[@]}"
