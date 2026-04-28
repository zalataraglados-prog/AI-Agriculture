# Wave-0 Baseline Contract Freeze

Updated: 2026-04-23

## 1) Purpose
Wave-0 is a coordination gate. We freeze the mainline data contract before feature fixes to avoid repeated rework.

## 2) Frozen API Contract (Mainline Only)

### 2.1 `GET /api/v1/sensor/schema`
- Returns sensor semantic schema used by frontend rendering.
- Required top-level field: `sensors` (array).
- Sensor item fields: `sensor_id`, `fields[]`, optional `trend_metric`, optional `category_metric`.
- Field item fields: `field`, `label`, `unit`, `data_type`, `required`, optional thresholds.

### 2.2 `GET /api/v1/telemetry`
- Query filters: `device_id`, `sensor_id`, `start_time`, `end_time`, `limit`.
- Returns telemetry array only.
- Row fields: `ts`, `device_id`, `sensor_id`, `fields`.

### 2.3 `GET /api/v1/image/uploads`
- Query filters: `device_id`, `upload_status`, `start_time`, `end_time`, `limit`.
- Required consumer fields (frontend freeze):
  - `upload_status` (`stored|inferred|failed`)
  - `predicted_class`
  - `disease_rate` (0..1)
  - `is_diseased` (bool)
- Supporting fields: `upload_id`, `captured_at`, `saved_path`, `error_message`.

### 2.4 Compatibility/Deprecated Endpoints
- `GET /api/telemetry` remains compatibility alias.
- `GET /api/dashboard`, `GET /api/charts`, `GET /api/fields` must stay deprecated (410) and cannot be used by frontend business logic.

## 3) Field-to-Page Mapping (Single Source of Truth)

| Page/Module | Data Source | Required Fields | Rule |
|---|---|---|---|
| Home sensor cards | `/api/v1/telemetry` + `/api/v1/sensor/schema` | `sensor_id`, `fields`, schema field meta | Dynamic render by schema, no hard-coded unit labels |
| Sensor detail | same as above | schema fields + latest telemetry row | Fault derived from staleness + schema validation |
| Charts | `/api/v1/telemetry` + schema | `ts`, numeric fields | Real time-axis, dynamic Y range expansion |
| Image diagnosis list | `/api/v1/image/uploads` | `upload_status`, `predicted_class`, `disease_rate`, `is_diseased` | No mock fallback; image preview from `/api/v1/image/file` |

## 4) Deployment Consistency Baseline

Runtime baseline (verified on cloud host):
- Service: `ai-agri-cloud-receiver.service`
- Binary: `/opt/ai-agriculture/cloud/bin/ai-agri-cloud-receiver`
- Config: `/opt/ai-agriculture/cloud/config/sensors.toml`
- Frontend static root served by same process: `/opt/ai-agriculture/cloud/frontend/rice`

Rule:
- Source repo directory and runtime directory must be explicitly synchronized per release.
- Any feature marked "done" must include runtime evidence against the actual service/binary path.

## 5) Wave-0 Exit Criteria
- All three frozen APIs respond with valid contract shape.
- Frontend no longer relies on deprecated mock endpoints.
- Frontend diagnosis and charts do not use mock fallback branches.
- Runtime path consistency check is documented in release checklist.

## 6) Change Governance
- Any contract change to sections 2/3 must be announced in Discussion #6 before implementation.
- If contract change is approved, this document must be updated in same PR.

## 7) Verification Command (Rust Only)
- Verifier implementation: `cloud/src/bin/wave0_verify_contract.rs`
- Run:
  - `cargo run --manifest-path cloud/Cargo.toml --bin wave0_verify_contract -- --base http://127.0.0.1:8088`
  - For cloud host check:
    - `cargo run --manifest-path cloud/Cargo.toml --bin wave0_verify_contract -- --base http://8.134.32.223:8088`
