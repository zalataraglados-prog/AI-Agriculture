# gateway-wsl

Rust sender for WSL to simulate Orange Pi Zero3 packet transmission.

Current goal: connectivity smoke test with fixed packet payload `success`.

## Quick Start

```bash
cargo run -- --target 127.0.0.1:9000
```

## Optional Args

- `--target <ip:port>`: destination address, default `127.0.0.1:9000`
- `--count <n>`: number of packets, default `1`
- `--interval-ms <ms>`: interval between packets in milliseconds, default `1000`

Example (5 packets, 500ms interval):

```bash
cargo run -- --target 192.168.1.50:9000 --count 5 --interval-ms 500
```

## Packet

- protocol: UDP
- payload: fixed string `success`
