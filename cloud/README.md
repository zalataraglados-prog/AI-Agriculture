# cloud receiver

Rust UDP receiver for cloud-side connectivity testing.
Now supports config-driven sensor rules, so adding a new sensor only requires config updates.

It is designed to pair with the `gateway-wsl` sender:
- fixed smoke packet: `success`
- sensor packet: `sensor_id:key=value,key2=value2` (examples: `dht22:temp_c=28.0,hum=48.9`, `adc:pin=34,raw=523,voltage=0.421`)
- modbus soil packet: `soil_modbus_02:device_id=dev_xxx,vwc=26.9,temp_c=24.8,ec=432,protocol=modbus.rtu.v1,slave_id=2`

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

### Generate register token safely

Prefer generating token with config path so token store matches running service location:

```bash
cargo run --quiet -- token --config /opt/ai-agriculture/cloud/config/sensors.toml
```

Or explicitly pin token store path:

```bash
cargo run --quiet -- token --token-store /opt/ai-agriculture/cloud/state/token_store.json
```

## Config format (`config/sensors.toml`)

```toml
[receiver]
bind = "0.0.0.0:9000"
once = false
timeout_ms = 30000
ack_mismatch = "ack:error"
ack_unknown_sensor = "ack:unknown_sensor"
telemetry_store_path = "state/telemetry.jsonl"
image_store_path = "state/image_uploads"
image_index_path = "state/image_index.jsonl"
image_db_error_store_path = "state/image_upload_errors.jsonl"
database_url = "postgres://postgres@127.0.0.1/ai_agriculture"
ai_predict_url = "http://127.0.0.1:8000/api/v1/predict"
openclaw_url = "http://127.0.0.1:3000"

[[exact_payloads]]
payload = "success"
ack = "ack:success"

[[sensors]]
id = "dht22"
ack = "ack:dht22"
required_fields = ["temp_c", "hum"]

[sensors.field_types]
temp_c = "f32"
hum = "f32"

[[sensors]]
id = "adc"
ack = "ack:adc"
required_fields = ["pin", "raw", "voltage"]

[sensors.field_types]
pin = "u8"
raw = "u16"
voltage = "f32"

[[sensors]]
id = "soil_modbus_02"
ack = "ack:soil_modbus_02"
required_fields = ["vwc", "temp_c", "ec"]

[sensors.field_types]
vwc = "f32"
temp_c = "f32"
ec = "u32"
protocol = "string"
slave_id = "u16"
```

Supported `field_types`:
- `string`, `bool`, `u8`, `u16`, `u32`, `i32`, `f32`, `f64`

## Telemetry query API (DB-only reads)

The cloud receiver appends matched packets to `telemetry_store_path` (`jsonl`) for backup and writes
authoritative records to `sensor_telemetry` in PostgreSQL/TimescaleDB. Query APIs read DB only:

- `GET /api/telemetry`
- Optional query parameters:
  - `device_id`
  - `sensor_id`
  - `limit` (default `100`, max `1000`)

## Image upload API (DB primary + JSONL backup)

- `POST /api/v1/image/upload`
- `Content-Type: multipart/form-data`
- file field names supported: `file` (default), `image`, `photo`
- required query params: `device_id`, `ts`
- optional query params: `location`, `crop_type`, `farm_note`
- inference fields are no longer read from query; cloud invokes AI predict API after DB `stored`.

Response is always JSON with `status`:
- success: includes `upload_id`, `saved_path`, and echoed `tag`
- error: includes readable `message`

Persistence (DB-first):
- image file: `{image_store_path}/{device_id}/{yyyy-mm-dd}/{upload_id}.jpg|png`
- DB primary write: `image_uploads` (`stored -> inferred` or `stored -> failed`)
- AI inference write: `image_inference_results`
- UDP telemetry write: `sensor_telemetry`
- backup line: `{image_index_path}` (JSONL with path/tag/hash/size)
- DB failure backup: `{image_db_error_store_path}` (JSONL errors)

Query API:
- `GET /api/v1/image/uploads`
- Optional query params:
  - `start_time` (RFC3339)
  - `end_time` (RFC3339)
  - `device_id`
  - `crop_type`
  - `upload_status` (`stored|inferred|failed`)
  - `predicted_class`
  - `limit` (default `100`, max `1000`)

## Agent chat proxy API

- `POST /api/v1/chat`
- request body:
  - `message` (required string)
  - `context` (optional JSON object)
- cloud forwards the request to `${openclaw_url}/api/v1/chat` and normalizes response to:
  - `{ "reply": "..." }`

If your OpenClaw runtime does not expose `POST /api/v1/chat`, install and run the lightweight adapter:

```bash
chmod +x scripts/install_openclaw_chat_adapter.sh
./scripts/install_openclaw_chat_adapter.sh
```

It starts `openclaw-chat-adapter` on `127.0.0.1:3000` and bridges chat requests to:
`openclaw agent --local --agent main --message ... --json`.

## AI-ag Ops CLI

Install the `AI-ag` command (whitelist ops wrapper described in `doc/AI-ag-agent-skill.md`):

```bash
chmod +x scripts/install_ai_ag_cli.sh
./scripts/install_ai_ag_cli.sh
```

Quick checks:

```bash
AI-ag server
AI-ag token
AI-ag ls-gateway
AI-ag help
```

## Database layout

- Migrations run in order: `0001` + `0002` + `0003`.
- `0003_timescale_rewrite.sql` enables TimescaleDB and converts:
  - `sensor_telemetry(ts)` -> hypertable (`2 hours` chunks)
  - `image_uploads(captured_at)` -> hypertable (`2 hours` chunks)
- Image upload/inference linkage uses `(upload_id, captured_at)` to keep partition-safe uniqueness.

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
- `OVERWRITE_CONFIG` (default `0`; set to `1` only when you want to replace existing config)
- `STATIC_SOURCE_FRONTEND` (default `${SCRIPT_DIR}/../frontend/rice`)
- `STATIC_SOURCE_DASHBOARD` (default `${SCRIPT_DIR}/dashboard`)
- `STATIC_TARGET_FRONTEND` (default `${INSTALL_ROOT}/frontend/rice`)
- `STATIC_TARGET_DASHBOARD` (default `${INSTALL_ROOT}/dashboard`)

The script will:

1. Build release binary
2. Install it under `${INSTALL_ROOT}/bin`
3. Keep existing config by default (install default only if missing; set `OVERWRITE_CONFIG=1` to replace)
4. Sync static files (`frontend/rice` as primary, `dashboard` as fallback)
5. Prefer systemd service deployment (fallback to `nohup` if systemd is unavailable)

## Quick test from another machine

Use your gateway sender to send one fixed packet:

```bash
cargo run -- --target <cloud-ip>:9000 --count 1
```

Or send sensor packet (from your updated gateway serial mode):

```text
dht22:temp_c=28.0,hum=48.9
adc:pin=34,raw=523,voltage=0.421
```

You should see `MATCH` and ACK logs in cloud receiver output.

## Local config smoke test

```bash
chmod +x scripts/local_config_smoke_test.sh
./scripts/local_config_smoke_test.sh
```

This validates:
- `success -> ack:success`
- `dht22:temp_c=28.0,hum=48.9 -> ack:dht22`
- `adc:pin=34,raw=523,voltage=0.421 -> ack:adc`
- invalid typed packet -> `ack:error`

## Image upload stress CLI (hot frequency change)

Use custom CLI to pressure test image upload and change frequency without restarting:

```bash
python3 scripts/image_stress_cli.py run \
  --endpoint http://8.134.32.223:8088/api/v1/image/upload \
  --device-id dev_stress_01 \
  --interval-sec 5 \
  --control-file /tmp/image_stress_control.json
```

In another terminal, hot change rate:

```bash
python3 scripts/image_stress_cli.py set-rate --hz 1 --control-file /tmp/image_stress_control.json
python3 scripts/image_stress_cli.py set-rate --interval-sec 0.2 --control-file /tmp/image_stress_control.json
python3 scripts/image_stress_cli.py show-rate --control-file /tmp/image_stress_control.json
python3 scripts/image_stress_cli.py stop --control-file /tmp/image_stress_control.json
```

Notes:
- `run` reads `control-file` every loop, so frequency changes are applied live.
- Use `--image-path <file>` to upload a real test image; if omitted, it uses a built-in tiny PNG.
- Use `--max-uploads <n>` for bounded test runs in CI/local smoke.

## Performance observability (P50/P95/P99)

Cloud now exposes in-process latency breakdown metrics for the critical upload pipeline:

- `GET /api/v1/perf/latency`

Returned stages include:
- `queue_ms`
- `read_body_ms`
- `parse_multipart_ms`
- `save_file_ms`
- `db_store_ms`
- `ai_infer_ms`
- `db_finalize_ms`
- `response_build_ms`
- `total_ms`

Each stage reports rolling-window `p50/p95/p99` plus last sample and count.

Example:

```bash
curl -s http://127.0.0.1:8088/api/v1/perf/latency | jq .
```

Recommended quick baseline procedure for issue #60:

```bash
# 1) start pressure run
python3 scripts/image_stress_cli.py run \
  --endpoint http://127.0.0.1:8088/api/v1/image/upload \
  --device-id dev_perf_01 \
  --interval-sec 1 \
  --control-file /tmp/image_stress_control.json

# 2) snapshot perf (in another terminal)
curl -s http://127.0.0.1:8088/api/v1/perf/latency | jq '.image_upload.total_ms.stats'

# 3) stop pressure
python3 scripts/image_stress_cli.py stop --control-file /tmp/image_stress_control.json
```
