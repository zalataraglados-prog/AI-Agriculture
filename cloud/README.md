# cloud receiver

Rust UDP receiver for cloud-side connectivity testing.

It is designed to pair with the `gateway-wsl` sender that transmits payload `success`.
For each received packet, it sends an ACK back to the sender.

## Run locally

```bash
cargo run -- --bind 0.0.0.0:9000 --expected success --once
```

### Optional args

- `--bind <ip:port>`: listen address (default `0.0.0.0:9000`)
- `--expected <payload>`: expected payload (default `success`)
- `--ack-match <payload>`: ACK when payload matches (default `ack:success`)
- `--ack-mismatch <payload>`: ACK when payload mismatches (default `ack:error`)
- `--once`: exit after first successful match
- `--max-packets <n>`: stop after receiving `n` packets
- `--timeout-ms <ms>`: read timeout in milliseconds, `0` means no timeout

## One-click deploy (Linux cloud server)

```bash
chmod +x deploy.sh
./deploy.sh
```

Environment variables:

- `INSTALL_ROOT` (default `/opt/ai-agriculture/cloud`)
- `SERVICE_NAME` (default `ai-agri-cloud-receiver`)
- `BIND_ADDR` (default `0.0.0.0:9000`)
- `EXPECTED_PAYLOAD` (default `success`)

The script will:

1. Build release binary
2. Install it under `${INSTALL_ROOT}/bin`
3. Prefer systemd service deployment (fallback to `nohup` if systemd is unavailable)

## Quick test from another machine

Use your gateway sender to send:

```bash
cargo run -- --target <cloud-ip>:9000 --count 1
```

You should see `MATCH` and ACK logs in cloud receiver output.
