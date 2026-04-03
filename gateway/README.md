# gateway-wsl

Rust sender for WSL to simulate Orange Pi Zero3 packet transmission.

Current goal: connectivity smoke test with fixed packet payload `success`.

## Quick Start

```bash
cargo run -- --target 127.0.0.1:9000
```

By default, it runs in an infinite loop and sends one packet every 5 seconds.
It also waits for ACK payload `ack:success` after each packet.

## Optional Args

- `--target <ip:port>`: destination address, default `127.0.0.1:9000`
- `--count <n>`: finite packet count; if omitted, send forever
- `--interval-ms <ms>`: interval between packets in milliseconds, default `5000`
- `--no-wait-ack`: disable ACK waiting mode
- `--ack-timeout-ms <ms>`: ACK timeout in milliseconds, default `3000`
- `--expected-ack <payload>`: expected ACK payload, default `ack:success`

Example (5 packets, 500ms interval):

```bash
cargo run -- --target 192.168.1.50:9000 --count 5 --interval-ms 500
```

## Packet

- protocol: UDP
- payload: fixed string `success`
