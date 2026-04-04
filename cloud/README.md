# cloud receiver

Rust UDP receiver for cloud-side connectivity testing.
Now supports config-driven sensor rules, so adding a new sensor only requires config updates.

It is designed to pair with the `gateway-wsl` sender:
- fixed smoke packet: `success`
- sensor packet: `sensor_id:key=value,key2=value2` (examples: `mq7:raw=206,voltage=0.166`, `dht22:temp_c=28.0,hum=48.9`)

For each received packet, it sends an ACK back to the sender.

## Run locally

```bash
cargo run -- --config config/sensors.toml --bind 0.0.0.0:9000 --timeout-ms 0
```

### Optional args

- `--config <path>`: TOML rules file (default `config/sensors.toml`)
- `--bind <ip:port>`: listen address (default `0.0.0.0:9000`)
- `--ack-mismatch <payload>`: ACK when payload mismatches config (override)
- `--ack-unknown-sensor <payload>`: ACK when sensor id has no rule (override)
- `--once`: exit after first successful match
- `--max-packets <n>`: stop after receiving `n` packets
- `--timeout-ms <ms>`: read timeout in milliseconds, `0` means no timeout
- legacy compatibility:
  - `--expected <payload>` and `--ack-match <payload>` add one temporary exact rule at runtime

## Config format (`config/sensors.toml`)

```toml
[receiver]
bind = "0.0.0.0:9000"
once = false
timeout_ms = 30000
ack_mismatch = "ack:error"
ack_unknown_sensor = "ack:unknown_sensor"

[[exact_payloads]]
payload = "success"
ack = "ack:success"

[[sensors]]
id = "mq7"
ack = "ack:mq7"
required_fields = ["raw", "voltage"]

[sensors.field_types]
raw = "u16"
voltage = "f32"

[[sensors]]
id = "dht22"
ack = "ack:dht22"
required_fields = ["temp_c", "hum"]

[sensors.field_types]
temp_c = "f32"
hum = "f32"
```

Supported `field_types`:
- `string`, `bool`, `u8`, `u16`, `u32`, `i32`, `f32`, `f64`

## Add a new sensor (no Rust code change)

1. Add a new `[[sensors]]` block in `config/sensors.toml`
2. Set `id`, `ack`, `required_fields`, and optional `field_types`
3. Redeploy (`./deploy.sh`) or replace config and restart service

## One-click deploy (Linux cloud server)

```bash
chmod +x deploy.sh
./deploy.sh
```

Environment variables:

- `INSTALL_ROOT` (default `/opt/ai-agriculture/cloud`)
- `SERVICE_NAME` (default `ai-agri-cloud-receiver`)
- `BIND_ADDR` (default `0.0.0.0:9000`)
- `CONFIG_PATH` (default `${INSTALL_ROOT}/config/sensors.toml`)

The script will:

1. Build release binary
2. Install it under `${INSTALL_ROOT}/bin`
3. Install config under `${INSTALL_ROOT}/config/sensors.toml`
4. Prefer systemd service deployment (fallback to `nohup` if systemd is unavailable)

## Quick test from another machine

Use your gateway sender to send one fixed packet:

```bash
cargo run -- --target <cloud-ip>:9000 --count 1
```

Or send sensor packet (from your updated gateway serial mode):

```text
mq7:raw=206,voltage=0.166
dht22:temp_c=28.0,hum=48.9
```

You should see `MATCH` and ACK logs in cloud receiver output.

## Local config smoke test

```bash
chmod +x scripts/local_config_smoke_test.sh
./scripts/local_config_smoke_test.sh
```

This validates:
- `success -> ack:success`
- `mq7:raw=206,voltage=0.166 -> ack:mq7`
- `dht22:temp_c=28.0,hum=48.9 -> ack:dht22`
- invalid typed packet -> `ack:error`
