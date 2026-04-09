#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-}"
BAUD="${2:-921600}"
APP_BIN="${3:-${ESP_APP_BIN:-}}"

if [[ -z "${PORT}" ]]; then
  echo "usage: $0 <serial_port> [baud] [app_bin]" >&2
  exit 2
fi

# Priority:
# 1) Explicit app binary argument or ESP_APP_BIN env
# 2) Prebuilt binary at firmware/esp32/rust_sensor_node.bin
# 3) Auto-build binary from firmware/esp32/rust_sensor_node via cargo-espflash
# 4) Build+flash from ESP_IDF_PROJECT_DIR via idf.py
if [[ -z "${APP_BIN}" && -f "firmware/esp32/rust_sensor_node.bin" ]]; then
  APP_BIN="firmware/esp32/rust_sensor_node.bin"
fi

if [[ -z "${APP_BIN}" ]]; then
  BUILD_SCRIPT="scripts/build_rust_sensor_node.sh"
  RUST_PROJECT_DIR="firmware/esp32/rust_sensor_node"
  if [[ -f "${BUILD_SCRIPT}" && -d "${RUST_PROJECT_DIR}" ]]; then
    echo "[flash] no app bin found, trying auto build"
    if ! bash "${BUILD_SCRIPT}"; then
      echo "[flash] auto build failed. Install ESP Rust toolchain (espup) or set ESP_APP_BIN." >&2
      echo "[flash] quick fallback: export ESP_APP_BIN=/path/to/rust_sensor_node.bin" >&2
      exit 7
    fi
    if [[ -f "firmware/esp32/rust_sensor_node.bin" ]]; then
      APP_BIN="firmware/esp32/rust_sensor_node.bin"
    fi
  fi
fi

if [[ -n "${APP_BIN}" ]]; then
  if [[ ! -f "${APP_BIN}" ]]; then
    echo "[flash] APP bin not found: ${APP_BIN}" >&2
    exit 3
  fi

  if command -v cargo >/dev/null 2>&1; then
    if cargo +esp espflash --version >/dev/null 2>&1; then
      echo "[flash] Flash Rust app binary via cargo espflash write-bin"
      echo "[flash] port=${PORT} baud=${BAUD} app=${APP_BIN}"
      cargo +esp espflash write-bin --chip esp32 --port "${PORT}" --baud "${BAUD}" --non-interactive 0x10000 "${APP_BIN}"
      echo "[flash] done"
      exit 0
    fi
  fi

  ESPTOOL_CMD=()
  if command -v esptool.py >/dev/null 2>&1; then
    ESPTOOL_CMD=(esptool.py)
  elif command -v python3 >/dev/null 2>&1 && python3 -m esptool version >/dev/null 2>&1; then
    ESPTOOL_CMD=(python3 -m esptool)
  else
    echo "[flash] esptool is not available (need esptool.py or python3 -m esptool)" >&2
    exit 4
  fi

  echo "[flash] Flash Rust app binary via esptool.py"
  echo "[flash] port=${PORT} baud=${BAUD} app=${APP_BIN}"
  # 0x10000 is ESP32 default app partition offset in common partition tables.
  ${ESPTOOL_CMD[@]} --port "${PORT}" --baud "${BAUD}" write_flash 0x10000 "${APP_BIN}"
  echo "[flash] done"
  exit 0
fi

ESP_IDF_PROJECT_DIR="${ESP_IDF_PROJECT_DIR:-firmware/esp32/rust_sensor_node}"
if [[ ! -d "${ESP_IDF_PROJECT_DIR}" ]]; then
  echo "[flash] No app bin provided and ESP_IDF_PROJECT_DIR not found: ${ESP_IDF_PROJECT_DIR}" >&2
  echo "[flash] set ESP_APP_BIN or create project dir before auto flash" >&2
  exit 5
fi

if ! command -v idf.py >/dev/null 2>&1; then
  echo "[flash] idf.py not found in PATH" >&2
  exit 6
fi

echo "[flash] Build+flash from ESP-IDF project: ${ESP_IDF_PROJECT_DIR}"
(
  cd "${ESP_IDF_PROJECT_DIR}"
  idf.py -p "${PORT}" -b "${BAUD}" flash
)

echo "[flash] done"
