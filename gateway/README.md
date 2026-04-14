# gateway

Rust gateway for Linux/WSL/OrangePi. Current CLI uses subcommands:

- `run`: managed runtime (serial discovery or native GPIO)
- `diag`: scan and print discovery diagnostics
- `reset`: clear persisted gateway state

## Quick Start

```bash
cargo run -- run --target 127.0.0.1:9000
```

If `state/gateway_profile.json` does not exist, the process will prompt for first-time setup and then persist the profile.

## Run Mode

Auto-flash behavior has been removed from the gateway runtime. The process now focuses on discovery, session management, and data forwarding only.

### Managed Serial Discovery

In default `run` mode, gateway recursively scans serial ports and baud rates, auto-detects managed protocol devices, and starts forwarding sensor packets.

If no usable serial device session is launched for several consecutive scan rounds, gateway will automatically try to switch to direct sensor mode (native GPIO) when available.

```bash
cargo run -- run \
  --target 8.134.32.223:9000 \
  --baud-list 115200,57600,9600,74880 \
  --scan-interval-ms 5000 \
  --scan-window-ms 1800
```

### Native GPIO Mode (Direct PH7 / PC11)

When sensors are directly wired to OrangePi GPIO (no external serial bridge):

```bash
cargo run -- run --native-gpio --target 127.0.0.1:9000
```

Default native mapping:

- PH7 -> gpio231 -> sensor id `needle_ph7`
- PC11 -> gpio75 -> sensor id `needle_pc11`
- round interval: `800ms`

Override GPIO mapping and interval with CLI flags:

```bash
cargo run -- run --native-gpio --target 127.0.0.1:9000 \
  --gpio-ph7 231 \
  --gpio-pc11 75 \
  --gpio-interval-ms 800
```

Or environment variables:

```bash
export GATEWAY_NATIVE_GPIO_PH7=231
export GATEWAY_NATIVE_GPIO_PC11=75
export GATEWAY_NATIVE_GPIO_INTERVAL_MS=800
```

## TOML Config

You can preload `run` configuration from TOML and still override values on CLI.

```bash
cargo run -- run --config config/gateway.toml --target 10.0.0.5:9000
```

`--config` is applied first, then explicit CLI flags take precedence.

Example `config/gateway.toml`:

```toml
[run]
target = "127.0.0.1:9000"
state_dir = "state"
scan_interval_ms = 5000
scan_window_ms = 1800
ack_timeout_ms = 3000
baud_list = [115200, 57600, 9600, 74880]
native_gpio = false
gpio_ph7 = 231
gpio_pc11 = 75
gpio_interval_ms = 800
```

## Script Launch

`scripts/run_mq7_gateway.sh` now maps to current CLI and supports both serial-discovery and native GPIO run modes.

Managed serial discovery mode:

```bash
TARGET=8.134.32.223:9000 BAUD_LIST=115200,57600,9600 ./scripts/run_mq7_gateway.sh
```

Native GPIO mode:

```bash
TARGET=8.134.32.223:9000 NATIVE_GPIO=1 GPIO_PH7=231 GPIO_PC11=75 ./scripts/run_mq7_gateway.sh
```

## CLI Flags (run)

- `--config <path>`: load `[run]` config from TOML
- `--target <ip:port>`: override cloud target; also persists to profile
- `--state-dir <dir>`: state directory (profile, feature map, device index)
- `--scan-interval-ms <ms>`: interval between recursive serial scans
- `--scan-window-ms <ms>`: serial discovery read window per probe
- `--ack-timeout-ms <ms>`: UDP ACK timeout
- `--baud-list <csv>`: baud candidates for auto-discovery
- `--native-gpio`: disable serial discovery and use direct GPIO source
- `--gpio-ph7 <num>`: native PH7 GPIO number override
- `--gpio-pc11 <num>`: native PC11 GPIO number override
- `--gpio-interval-ms <ms>`: native GPIO polling interval override

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
