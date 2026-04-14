#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-8.134.32.223:9000}"
STATE_DIR="${STATE_DIR:-state}"
ACK_TIMEOUT_MS="${ACK_TIMEOUT_MS:-3000}"
BAUD_LIST="${BAUD_LIST:-115200,57600,9600,74880}"
NATIVE_GPIO="${NATIVE_GPIO:-0}"
GPIO_PH7="${GPIO_PH7:-}"
GPIO_PC11="${GPIO_PC11:-}"
GPIO_INTERVAL_MS="${GPIO_INTERVAL_MS:-}"
CONFIG_FILE="${CONFIG_FILE:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATEWAY_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${GATEWAY_DIR}"

echo "[run_gateway] target=${TARGET} state_dir=${STATE_DIR} native_gpio=${NATIVE_GPIO}"

ARGS=(run --target "${TARGET}" --state-dir "${STATE_DIR}" --ack-timeout-ms "${ACK_TIMEOUT_MS}")

if [[ -n "${CONFIG_FILE}" ]]; then
  ARGS+=(--config "${CONFIG_FILE}")
fi

if [[ "${NATIVE_GPIO}" == "1" ]]; then
  ARGS+=(--native-gpio)
  if [[ -n "${GPIO_PH7}" ]]; then
    ARGS+=(--gpio-ph7 "${GPIO_PH7}")
  fi
  if [[ -n "${GPIO_PC11}" ]]; then
    ARGS+=(--gpio-pc11 "${GPIO_PC11}")
  fi
  if [[ -n "${GPIO_INTERVAL_MS}" ]]; then
    ARGS+=(--gpio-interval-ms "${GPIO_INTERVAL_MS}")
  fi
else
  ARGS+=(--baud-list "${BAUD_LIST}")
fi

cargo run --release -- "${ARGS[@]}"
