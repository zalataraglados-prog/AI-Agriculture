#!/usr/bin/env bash

# AI-Agriculture cloud deployment helper.
# Defaults are conservative: stop only known project PID files and the named
# systemd service when explicitly requested. Override paths in cloud/.env.

set -Eeuo pipefail

log() {
    printf '[deploy-cloud] %s\n' "$*"
}

die() {
    printf '[deploy-cloud] ERROR: %s\n' "$*" >&2
    exit 1
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

load_env_file() {
    if [[ ! -f "$ENV_FILE" ]]; then
        die "Missing env file: $ENV_FILE"
    fi

    # shellcheck disable=SC1090
    set -a
    source "$ENV_FILE"
    set +a
}

parse_host() {
    local value="$1"
    printf '%s' "${value%:*}"
}

parse_port() {
    local value="$1"
    printf '%s' "${value##*:}"
}

pid_is_running() {
    local pid="$1"
    [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1
}

stop_pid_file() {
    local pid_file="$1"
    local label="$2"

    if [[ ! -f "$pid_file" ]]; then
        return
    fi

    local pid
    pid="$(cat "$pid_file" 2>/dev/null || true)"
    if ! pid_is_running "$pid"; then
        rm -f "$pid_file"
        return
    fi

    log "Stopping $label from pid file: $pid"
    kill "$pid" >/dev/null 2>&1 || true

    for _ in {1..20}; do
        if ! pid_is_running "$pid"; then
            rm -f "$pid_file"
            return
        fi
        sleep 0.2
    done

    log "$label did not exit after SIGTERM; sending SIGKILL to pid $pid"
    kill -9 "$pid" >/dev/null 2>&1 || true
    rm -f "$pid_file"
}

stop_systemd_service() {
    if [[ "${STOP_SYSTEMD:-0}" != "1" ]]; then
        return
    fi

    if command -v systemctl >/dev/null 2>&1; then
        log "Stopping systemd service: $SERVICE_NAME"
        sudo systemctl stop "$SERVICE_NAME" || true
    fi
}

describe_port_users() {
    local port="$1"
    if ! command -v lsof >/dev/null 2>&1; then
        return
    fi
    lsof -nP -iTCP:"$port" -sTCP:LISTEN || true
}

ensure_port_available() {
    local port="$1"
    local label="$2"

    if ! command -v lsof >/dev/null 2>&1; then
        log "lsof is unavailable; skipping $label port ownership check."
        return
    fi

    local pids
    pids="$(lsof -t -nP -iTCP:"$port" -sTCP:LISTEN || true)"
    if [[ -z "$pids" ]]; then
        return
    fi

    log "$label port $port is already in use:"
    describe_port_users "$port"

    if [[ "${FORCE_KILL_PORT:-0}" != "1" ]]; then
        die "Port $port is busy. Stop the process or rerun with FORCE_KILL_PORT=1."
    fi

    log "FORCE_KILL_PORT=1; killing listeners on port $port"
    while IFS= read -r pid; do
        [[ -n "$pid" ]] && kill "$pid" >/dev/null 2>&1 || true
    done <<< "$pids"
    sleep 1
}

wait_for_http() {
    local url="$1"
    local log_file="$2"
    local label="$3"

    if ! command -v curl >/dev/null 2>&1; then
        log "curl is unavailable; skipping $label health check."
        return
    fi

    for _ in {1..30}; do
        if curl -fsS "$url" >/dev/null 2>&1; then
            log "$label is healthy: $url"
            return
        fi
        sleep 1
    done

    log "Last $label log lines:"
    tail -n 40 "$log_file" || true
    die "$label did not pass health check: $url"
}

PROJECT_ROOT="${PROJECT_ROOT:-/opt/src/AI-Agriculture}"
ENV_FILE="${ENV_FILE:-$PROJECT_ROOT/cloud/.env}"
load_env_file

PROJECT_ROOT="${PROJECT_ROOT:-/opt/src/AI-Agriculture}"
INSTALL_ROOT="${INSTALL_ROOT:-/opt/ai-agriculture/cloud}"
BIN_NAME="${BIN_NAME:-ai-agri-cloud-receiver}"
BIN_TARGET="${BIN_TARGET:-$INSTALL_ROOT/bin/$BIN_NAME}"
LOG_DIR="${LOG_DIR:-$PROJECT_ROOT/cloud/logs}"
CLOUD_LOG_FILE="${CLOUD_LOG_FILE:-$LOG_DIR/cloud.log}"
AI_LOG_FILE="${AI_LOG_FILE:-$LOG_DIR/ai_engine.log}"
CLOUD_PID_FILE="${CLOUD_PID_FILE:-$LOG_DIR/cloud.pid}"
AI_PID_FILE="${AI_PID_FILE:-$LOG_DIR/ai_engine.pid}"
SERVICE_NAME="${SERVICE_NAME:-ai-agri-cloud-receiver}"
CLOUD_BIND_ADDR="${CLOUD_BIND_ADDR:-0.0.0.0:8088}"
AI_BIND_ADDR="${AI_BIND_ADDR:-0.0.0.0:8000}"
START_AI_ENGINE="${START_AI_ENGINE:-1}"
STOP_SYSTEMD="${STOP_SYSTEMD:-0}"
FORCE_KILL_PORT="${FORCE_KILL_PORT:-0}"
CROP_PROFILE="${CROP_PROFILE:-oil_palm}"
CLOUD_CONFIG_PATH="${CLOUD_CONFIG_PATH:-$PROJECT_ROOT/cloud/config/sensors.toml}"
CLOUD_MIGRATION_DIR="${CLOUD_MIGRATION_DIR:-$PROJECT_ROOT/cloud/sql/migrations}"
STATIC_SOURCE_FRONTEND="${STATIC_SOURCE_FRONTEND:-$PROJECT_ROOT/frontend}"
STATIC_SOURCE_DASHBOARD="${STATIC_SOURCE_DASHBOARD:-$PROJECT_ROOT/cloud/dashboard}"
AI_PREDICT_URL="${AI_PREDICT_URL:-${AI_ENGINE_URL:-http://127.0.0.1:8000/api/v1/oil-palm/analyze}}"

export PROJECT_ROOT
export CLOUD_MIGRATION_DIR
export STATIC_SOURCE_FRONTEND
export STATIC_SOURCE_DASHBOARD
export CROP_PROFILE
export AI_PREDICT_URL

require_cmd cargo
require_cmd chmod
require_cmd cp
require_cmd kill
require_cmd mkdir
require_cmd nohup
if [[ "$START_AI_ENGINE" == "1" ]]; then
    require_cmd python3
fi

mkdir -p "$(dirname "$BIN_TARGET")" "$LOG_DIR"

CLOUD_PORT="$(parse_port "$CLOUD_BIND_ADDR")"
AI_HOST="$(parse_host "$AI_BIND_ADDR")"
AI_PORT="$(parse_port "$AI_BIND_ADDR")"
AI_HEALTH_URL="${AI_HEALTH_URL:-http://127.0.0.1:$AI_PORT/api/v1/health}"
CLOUD_HEALTH_URL="${CLOUD_HEALTH_URL:-http://127.0.0.1:$CLOUD_PORT/api/v1/plantations}"

log "Using project root: $PROJECT_ROOT"
log "Using env file: $ENV_FILE"

stop_systemd_service
stop_pid_file "$CLOUD_PID_FILE" "cloud backend"
if [[ "$START_AI_ENGINE" == "1" ]]; then
    stop_pid_file "$AI_PID_FILE" "AI engine"
fi

ensure_port_available "$CLOUD_PORT" "cloud backend"
if [[ "$START_AI_ENGINE" == "1" ]]; then
    ensure_port_available "$AI_PORT" "AI engine"
fi

log "Building cloud backend"
(
    cd "$PROJECT_ROOT/cloud"
    cargo build --release
)

log "Installing backend binary: $BIN_TARGET"
cp "$PROJECT_ROOT/target/release/cloud" "$BIN_TARGET"
chmod +x "$BIN_TARGET"

if [[ -n "${DATABASE_URL:-}" ]] && command -v psql >/dev/null 2>&1; then
    log "Checking database connectivity"
    psql "$DATABASE_URL" -c 'select 1' >/dev/null
elif [[ -z "${DATABASE_URL:-}" ]]; then
    log "DATABASE_URL is not set; cloud will use its config file fallback."
else
    log "psql is unavailable; skipping database connectivity preflight."
fi

if [[ "$START_AI_ENGINE" == "1" ]]; then
    log "Starting AI engine on $AI_BIND_ADDR"
    (
        cd "$PROJECT_ROOT"
        export PYTHONPATH="$PROJECT_ROOT${PYTHONPATH:+:$PYTHONPATH}"
        nohup python3 -m uvicorn ai_engine.main:app \
            --host "$AI_HOST" \
            --port "$AI_PORT" \
            > "$AI_LOG_FILE" 2>&1 &
        echo $! > "$AI_PID_FILE"
    )
fi

log "Starting cloud backend on $CLOUD_BIND_ADDR"
(
    cd "$PROJECT_ROOT"
    nohup "$BIN_TARGET" \
        --config "$CLOUD_CONFIG_PATH" \
        --bind "$CLOUD_BIND_ADDR" \
        > "$CLOUD_LOG_FILE" 2>&1 &
    echo $! > "$CLOUD_PID_FILE"
)

if [[ "$START_AI_ENGINE" == "1" ]]; then
    wait_for_http "$AI_HEALTH_URL" "$AI_LOG_FILE" "AI engine"
fi
wait_for_http "$CLOUD_HEALTH_URL" "$CLOUD_LOG_FILE" "cloud backend"

log "Deployment complete."
log "Cloud log: $CLOUD_LOG_FILE"
log "AI log: $AI_LOG_FILE"
