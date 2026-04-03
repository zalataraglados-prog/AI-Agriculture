#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-8.134.32.223:9000}"
SERIAL_PORT="${SERIAL_PORT:-/dev/ttyUSB0}"
SERIAL_BAUD="${SERIAL_BAUD:-115200}"
EXPECTED_ACK="${EXPECTED_ACK:-ack:mq7}"
ACK_TIMEOUT_MS="${ACK_TIMEOUT_MS:-3000}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATEWAY_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${GATEWAY_DIR}"

echo "[run_mq7_gateway] target=${TARGET} serial=${SERIAL_PORT}@${SERIAL_BAUD} expected_ack=${EXPECTED_ACK}"

cargo run --release -- \
  --target "${TARGET}" \
  --serial-port "${SERIAL_PORT}" \
  --serial-baud "${SERIAL_BAUD}" \
  --ack-timeout-ms "${ACK_TIMEOUT_MS}" \
  --expected-ack "${EXPECTED_ACK}"
