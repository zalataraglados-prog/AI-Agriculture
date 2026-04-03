# gateway-wsl

Rust gateway for WSL / Linux:
- fixed payload smoke test (`success`)
- serial ingest mode (read ESP32 MQ-7 serial output and forward via UDP)

## Quick Start

```bash
cargo run -- --target 127.0.0.1:9000
```

Default mode is fixed payload:
- payload: `success`
- interval: 5 seconds
- ACK expected: `ack:success`

## Serial Mode (MQ-7)

Expected serial line format from ESP32 firmware:

```text
MQ7 raw=206 voltage=0.166V
```

Gateway will parse this and forward:

```text
mq7:raw=206,voltage=0.166
```

Run:

```bash
cargo run -- --target 8.134.32.223:9000 --serial-port /dev/ttyUSB0 --serial-baud 115200 --expected-ack ack:mq7
```

Note:
- in serial mode, `--interval-ms` is ignored (send one UDP packet per valid serial line)
- if your cloud ACK still returns `ack:success`, set `--expected-ack ack:success`

## Optional Args

- `--target <ip:port>`: destination address, default `127.0.0.1:9000`
- `--count <n>`: finite packet count; if omitted, send forever
- `--interval-ms <ms>`: interval between packets in milliseconds, default `5000` (fixed mode only)
- `--no-wait-ack`: disable ACK waiting mode
- `--ack-timeout-ms <ms>`: ACK timeout in milliseconds, default `3000`
- `--expected-ack <payload>`: expected ACK payload, default `ack:success`
- `--serial-port <path>`: enable serial ingest mode, e.g. `/dev/ttyUSB0`
- `--serial-baud <baud>`: serial baud rate, default `115200`

Example (5 packets, 500ms interval):

```bash
cargo run -- --target 192.168.1.50:9000 --count 5 --interval-ms 500
```

## Linux Build Dependencies

On Ubuntu/WSL:

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libudev-dev
```

`serialport` crate needs `libudev` on Linux.
