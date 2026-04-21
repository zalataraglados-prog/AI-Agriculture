#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CLOUD_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
PORT="${PORT:-9900}"

cd "${CLOUD_DIR}"

LOG_FILE="$(mktemp /tmp/cloud-config-test.XXXXXX.log)"
echo "[smoke] receiver log: ${LOG_FILE}"

cargo build
target/debug/cloud --config config/sensors.toml --bind "127.0.0.1:${PORT}" --max-packets 3 --timeout-ms 15000 >"${LOG_FILE}" 2>&1 &
receiver_pid=$!

cleanup() {
  if kill -0 "${receiver_pid}" >/dev/null 2>&1; then
    kill "${receiver_pid}" >/dev/null 2>&1 || true
  fi
  if [[ -f "${LOG_FILE}" ]]; then
    echo "[smoke] receiver output:"
    cat "${LOG_FILE}" || true
  fi
}
trap cleanup EXIT

sleep 1

python3 - <<PY
import socket

addr = ("127.0.0.1", ${PORT})
packets = [
    b"success",
    b"dht22:temp_c=28.0,hum=48.9",
    b"adc:pin=34,raw=523,voltage=0.421",
]

for packet in packets:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.settimeout(2)
    sock.sendto(packet, addr)
    data, _ = sock.recvfrom(1024)
    print(packet.decode(), "=>", data.decode())
    sock.close()
PY

wait "${receiver_pid}"
trap - EXIT

echo "[smoke] receiver output:"
cat "${LOG_FILE}"
