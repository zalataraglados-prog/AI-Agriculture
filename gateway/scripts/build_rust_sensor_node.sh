#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_DIR="${ROOT_DIR}/firmware/esp32/rust_sensor_node"
OUT_BIN="${ROOT_DIR}/firmware/esp32/rust_sensor_node.bin"
ESP_RUST_TARGET="${ESP_RUST_TARGET:-xtensa-esp32-none-elf}"

if [[ -f "${HOME}/export-esp.sh" ]]; then
  # Load espup toolchain env automatically when available.
  # shellcheck disable=SC1090
  . "${HOME}/export-esp.sh"
fi

if [[ ! -d "${PROJECT_DIR}" ]]; then
  echo "[build] rust project not found: ${PROJECT_DIR}" >&2
  exit 20
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "[build] cargo not found" >&2
  exit 21
fi

if ! cargo +esp espflash --help >/dev/null 2>&1; then
  echo "[build] cargo-espflash is required" >&2
  echo "[build] install: cargo install cargo-espflash" >&2
  exit 22
fi

echo "[build] building rust_sensor_node -> ${OUT_BIN}"
(
  cd "${PROJECT_DIR}"
  cargo +esp espflash save-image \
    --chip esp32 \
    --ignore-app-descriptor \
    --target "${ESP_RUST_TARGET}" \
    --release \
    --package rust_sensor_node \
    --bin rust_sensor_node \
    "${OUT_BIN}"
)

if [[ ! -f "${OUT_BIN}" ]]; then
  echo "[build] build finished but output bin missing: ${OUT_BIN}" >&2
  exit 23
fi

echo "[build] target=${ESP_RUST_TARGET}"
echo "[build] done: ${OUT_BIN}"
