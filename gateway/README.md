# gateway

Rust gateway for Linux/WSL/OrangePi. Current version is focused on one industrial sensor path:

- Modbus-RTU (RS485), default query profile
- default slave id `2`, baud `9600`, parity `None (8N1)`
- default read holding registers `1..3` (request start addr `0`, count `3`)

- `run`: managed runtime (serial discovery)
- `diag`: scan and print discovery diagnostics
- `reset`: clear persisted gateway state

## Quick Start

```bash
cargo run -- run --target <cloud-ip:port>
```

If `state/gateway_profile.json` does not exist, the process will prompt for first-time setup and then persist the profile.

## Run Mode

Auto-flash behavior has been removed from the gateway runtime. The process now focuses on discovery, session management, and data forwarding only.

### Managed Modbus Discovery

In default `run` mode, gateway probes serial ports and uses Modbus request-response polling.
Once a valid response is found, it forwards one normalized event per second.

```bash
cargo run -- run \
  --target <cloud-ip:port> \
  --baud-list 9600 \
  --scan-interval-ms 5000 \
  --scan-window-ms 1800
```

Default payload fields:

- `sensor_id=soil_modbus_02`
- `temp_c` = register[1] / 10
- `vwc` = register[2] / 10
- `ec` = register[3]

## TOML Config

You can preload `run` configuration from TOML and still override values on CLI.

```bash
cargo run -- run --config config/gateway.toml --target <cloud-ip:port>
```

`--config` is applied first, then explicit CLI flags take precedence.

Example `config/gateway.toml`:

```toml
[run]
target = "YOUR_CLOUD_IP:9000"
state_dir = "state"
scan_interval_ms = 5000
scan_window_ms = 1800
ack_timeout_ms = 3000
baud_list = [115200, 57600, 9600, 74880]
image_dir = "sample_images"
image_upload_url = "http://YOUR_CLOUD_IP:8088/api/v1/image/upload"
image_interval_ms = 300000
```

Recommended Modbus-only config:

```toml
[run]
target = "YOUR_CLOUD_IP:9000"
state_dir = "state"
scan_interval_ms = 5000
scan_window_ms = 1800
ack_timeout_ms = 3000
baud_list = [9600]
```

## Script Launch

`scripts/run_mq7_gateway.sh` now maps to current CLI and runs in serial-discovery mode.

Managed Modbus mode:

```bash
TARGET=YOUR_CLOUD_IP:9000 MODBUS_PORT=/dev/ttyUSB0 BAUD_LIST=9600 ./scripts/run_mq7_gateway.sh
```

Physical/protocol precheck (recommended):

```bash
mbpoll -m rtu -a 2 -b 9600 -P none -r 1 -c 3 -1 /dev/ttyUSB0
```

## CLI Flags (run)

- `--config <path>`: load `[run]` config from TOML
- `--target <ip:port>`: override cloud target; also persists to profile
- `--state-dir <dir>`: state directory (profile, feature map, device index)
- `--scan-interval-ms <ms>`: interval between recursive serial scans
- `--scan-window-ms <ms>`: serial discovery read window per probe
- `--ack-timeout-ms <ms>`: UDP ACK timeout
- `--baud-list <csv>`: baud candidates for Modbus probe (default `9600`)
- `--image-dir <dir>`: enable simulated image upload from this directory (recursive scan jpg/jpeg/png)
- `--image-upload-url <url>`: override cloud image upload API (default derive from `--target` as `http://<host>:8088/api/v1/image/upload`)
- `--image-interval-ms <ms>`: image upload interval (default `300000`)
- `--image-upload-scheme <http|https>`: upload URL scheme when deriving from target (default `http`)
- `--image-upload-port <1-65535>`: upload URL port when deriving from target (default `8088`)
- `--image-upload-path </api/...>`: upload URL path when deriving from target (default `/api/v1/image/upload`)

Image simulator can also be configured by environment variables:
- `GATEWAY_IMAGE_DIR`
- `GATEWAY_IMAGE_UPLOAD_URL`
- `GATEWAY_IMAGE_UPLOAD_INTERVAL_MS`

Additional environment-based configuration:
- `GATEWAY_DEFAULT_TARGET` (no built-in fixed target now; set this to prefill first setup)
- `GATEWAY_STATE_DIR`
- `GATEWAY_BAUD_LIST`
- `GATEWAY_IMAGE_UPLOAD_SCHEME`, `GATEWAY_IMAGE_UPLOAD_PORT`, `GATEWAY_IMAGE_UPLOAD_PATH`
- `GATEWAY_MODBUS_*` (see `.env.example`)

## Other Subcommands

Diagnostics:

```bash
cargo run -- diag --state-dir state --scan-window-ms 1800 --baud-list 115200,57600,9600
```

Reset state:

```bash
cargo run -- reset --state-dir state
```

## Linux Build Dependencies

On Ubuntu/WSL:

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libudev-dev
```

`serialport` crate depends on `libudev` on Linux.
