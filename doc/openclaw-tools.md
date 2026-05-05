# OpenClaw Tools Integration

This document describes the repository-side contract for Sprint F. It does not
replace the real cloud host OpenClaw configuration under `/root/.openclaw/`.

## Goal

OpenClaw should answer agricultural questions from structured cloud facts
instead of guessing from chat history. The cloud service exposes read-only tool
APIs under:

```text
http://127.0.0.1:8088/api/v1/openclaw/tools
```

The tools reuse existing tree, session, UAV, assessment, plantation dashboard,
and block report data. They do not run real models and do not write database
state.

## Tool Endpoints

```text
GET  /api/v1/openclaw/tools/manifest
GET  /api/v1/openclaw/tools/tree-profile?tree_code=OP-000001&limit=10
GET  /api/v1/openclaw/tools/tree-timeline?tree_code=OP-000001&limit=20
GET  /api/v1/openclaw/tools/plantation-report?plantation_id=1&limit=50
GET  /api/v1/openclaw/tools/missing-evidence?tree_code=OP-000001
GET  /api/v1/openclaw/tools/patrol-report?plantation_id=1&limit=50
POST /api/v1/openclaw/tools/explain-prediction
```

`limit` is capped at 50 to keep OpenClaw context bounded.

## Safety Contract

- The tools are read-only.
- The tools do not call OpenClaw.
- The tools do not train or run real models.
- OpenClaw should not connect directly to the database.
- Disease language must stay in `suspected` / risk wording unless a future
  verified expert workflow says otherwise.
- Real `/root/.openclaw/openclaw.json` files, model credentials, and provider
  settings must not be committed to this repository.

## Cloud Authentication

These endpoints are under `/api/v1/*`, so when `CLOUD_AUTH_ENABLED=true` they
follow the existing cloud auth gate. On the cloud host, either keep access local
to `127.0.0.1` in a trusted service boundary or configure OpenClaw/adapter to
send the required cloud auth token.

Do not add a public unauthenticated path for these tools.

## Local Verification

After deploying cloud, verify the tool layer before changing OpenClaw config:

```bash
curl "http://127.0.0.1:8088/api/v1/openclaw/tools/manifest"
curl "http://127.0.0.1:8088/api/v1/openclaw/tools/tree-profile?tree_code=OP-000001"
curl "http://127.0.0.1:8088/api/v1/openclaw/tools/missing-evidence?tree_code=OP-000001"
curl "http://127.0.0.1:8088/api/v1/openclaw/tools/patrol-report?plantation_id=1"
```

Only after these return structured JSON should OpenClaw chat integration be
updated.

## Adapter Tool Context Mode

OpenClaw v2026.3.28 uses strict validation for `openclaw.json` and may reject
custom tool entries under `tools` or `agents.defaults`. For that runtime, do not
force tool definitions into the OpenClaw config. Use the chat adapter as a
tool-context bridge instead.

The adapter receives `/api/v1/chat`, detects simple agriculture intents, calls
the cloud tool API on `127.0.0.1:8088`, and injects a bounded `[tool_context]`
block into the prompt before invoking:

```text
openclaw agent --local --agent main --message ... --json
```

Supported intent hints:

- `OP-000048` + general profile wording -> `query_tree_profile`
- `OP-000048` + missing/evidence wording -> `query_missing_evidence`
- `OP-000048` + timeline/history wording -> `query_tree_timeline`
- patrol/priority wording + `plantation_id` -> `generate_patrol_report`
- plantation/report/dashboard wording + `plantation_id` -> `query_plantation_report`

Adapter environment variables:

```text
CLOUD_TOOL_BASE_URL=http://127.0.0.1:8088/api/v1/openclaw/tools
CLOUD_TOOL_TIMEOUT_SEC=5
CLOUD_TOOL_CONTEXT_MAX_CHARS=12000
OPENCLAW_DEFAULT_PLANTATION_ID=1
```

`OPENCLAW_DEFAULT_PLANTATION_ID` is optional. Prefer passing an explicit
`plantation_id` in the question or frontend context.

## OpenClaw Registration Template

Use `doc/openclaw-tool-manifest.example.json` as a template for the cloud-side
tool registration only if the installed OpenClaw version supports external tool
registration. For strict-schema versions, treat it as a contract document and use
adapter tool-context mode instead. Keep the real file on the cloud host.

Suggested behavior for the main agent:

- For tree questions, call `query_tree_profile` first.
- For evidence gaps, call `query_missing_evidence`.
- For patrol planning, call `generate_patrol_report`.
- For plantation summaries, call `query_plantation_report`.
- For AI result explanation, call `explain_prediction`.
- If a tool returns `status=error`, report the error and ask for a valid
  `tree_code` or `plantation_id`; do not invent facts.

## Example Questions

- `查询 OP-000001 的树档案`
- `这棵树缺少哪些证据？`
- `今天优先巡检哪些树？`
- `解释这张 trunk_base 图片的 mock 结果`
