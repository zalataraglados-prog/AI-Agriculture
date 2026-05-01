use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tiny_http::{Header, Method, Response, Server};

use crate::ai_client::{infer_image_from_bytes, AiInferenceOutput};
use crate::auth::{AuthManager, AuthSession};
use crate::db::{
    DbManager, ImageInferenceDbRecord, ImageUploadDbRecord, ImageUploadQueryFilter,
    SensorTelemetryQueryFilter,
};
use crate::image_upload::{
    append_image_error_backup, append_image_index_backup, build_upload_ok_response,
    parse_captured_at_utc, parse_multipart_file, parse_tag, save_image_file,
    ImageUploadErrorResponse, ImageUploadOkResponse,
};
use crate::model::{DeviceRegistryFile, FieldType, SensorRule};
use crate::presence::PresenceTracker;
use crate::time_util::now_rfc3339;

const QUERY_CACHE_TTL_SECONDS: u64 = 15;
const QUERY_CACHE_MAX_ENTRIES: usize = 500;
const PERF_METRIC_WINDOW_SIZE: usize = 2048;

fn env_usize(key: &str, default_value: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default_value)
}

fn env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_value)
}

#[derive(Debug, Clone)]
struct QueryCacheEntry {
    value: String,
    expires_at: Instant,
}

#[derive(Debug)]
struct QueryCache {
    entries: HashMap<String, QueryCacheEntry>,
    order: VecDeque<String>,
    ttl: Duration,
    capacity: usize,
}

#[derive(Debug, Default, Clone, Serialize)]
struct StagePercentiles {
    p50_ms: u64,
    p95_ms: u64,
    p99_ms: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
struct PipelineStageReport {
    count: usize,
    last_ms: u64,
    stats: StagePercentiles,
}

#[derive(Debug, Default, Clone, Serialize)]
struct PipelinePerfReport {
    total_requests: u64,
    success_requests: u64,
    failed_requests: u64,
    queue_ms: PipelineStageReport,
    read_body_ms: PipelineStageReport,
    parse_multipart_ms: PipelineStageReport,
    save_file_ms: PipelineStageReport,
    db_store_ms: PipelineStageReport,
    ai_infer_ms: PipelineStageReport,
    db_finalize_ms: PipelineStageReport,
    response_build_ms: PipelineStageReport,
    total_ms: PipelineStageReport,
}

#[derive(Debug, Default, Clone, Serialize)]
struct PerfSnapshotPayload {
    image_upload: PipelinePerfReport,
    chat_proxy: PipelinePerfReport,
    sampled_at: String,
}

#[derive(Debug, Clone)]
struct StageMetric {
    values: VecDeque<u64>,
    capacity: usize,
    last_ms: u64,
}

impl StageMetric {
    fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(capacity),
            capacity,
            last_ms: 0,
        }
    }

    fn push(&mut self, value_ms: u64) {
        self.last_ms = value_ms;
        self.values.push_back(value_ms);
        while self.values.len() > self.capacity {
            self.values.pop_front();
        }
    }

    fn report(&self) -> PipelineStageReport {
        PipelineStageReport {
            count: self.values.len(),
            last_ms: self.last_ms,
            stats: compute_percentiles(&self.values),
        }
    }
}

#[derive(Debug, Clone)]
struct PipelinePerfMetrics {
    total_requests: u64,
    success_requests: u64,
    failed_requests: u64,
    queue_ms: StageMetric,
    read_body_ms: StageMetric,
    parse_multipart_ms: StageMetric,
    save_file_ms: StageMetric,
    db_store_ms: StageMetric,
    ai_infer_ms: StageMetric,
    db_finalize_ms: StageMetric,
    response_build_ms: StageMetric,
    total_ms: StageMetric,
}

impl PipelinePerfMetrics {
    fn new(capacity: usize) -> Self {
        Self {
            total_requests: 0,
            success_requests: 0,
            failed_requests: 0,
            queue_ms: StageMetric::new(capacity),
            read_body_ms: StageMetric::new(capacity),
            parse_multipart_ms: StageMetric::new(capacity),
            save_file_ms: StageMetric::new(capacity),
            db_store_ms: StageMetric::new(capacity),
            ai_infer_ms: StageMetric::new(capacity),
            db_finalize_ms: StageMetric::new(capacity),
            response_build_ms: StageMetric::new(capacity),
            total_ms: StageMetric::new(capacity),
        }
    }

    fn report(&self) -> PipelinePerfReport {
        PipelinePerfReport {
            total_requests: self.total_requests,
            success_requests: self.success_requests,
            failed_requests: self.failed_requests,
            queue_ms: self.queue_ms.report(),
            read_body_ms: self.read_body_ms.report(),
            parse_multipart_ms: self.parse_multipart_ms.report(),
            save_file_ms: self.save_file_ms.report(),
            db_store_ms: self.db_store_ms.report(),
            ai_infer_ms: self.ai_infer_ms.report(),
            db_finalize_ms: self.db_finalize_ms.report(),
            response_build_ms: self.response_build_ms.report(),
            total_ms: self.total_ms.report(),
        }
    }
}

#[derive(Debug, Clone)]
struct PerfMetrics {
    image_upload: PipelinePerfMetrics,
    chat_proxy: PipelinePerfMetrics,
}

impl PerfMetrics {
    fn new(capacity: usize) -> Self {
        Self {
            image_upload: PipelinePerfMetrics::new(capacity),
            chat_proxy: PipelinePerfMetrics::new(capacity),
        }
    }

    fn snapshot(&self) -> PerfSnapshotPayload {
        PerfSnapshotPayload {
            image_upload: self.image_upload.report(),
            chat_proxy: self.chat_proxy.report(),
            sampled_at: now_rfc3339(),
        }
    }
}

fn compute_percentiles(values: &VecDeque<u64>) -> StagePercentiles {
    if values.is_empty() {
        return StagePercentiles::default();
    }
    let mut sorted: Vec<u64> = values.iter().copied().collect();
    sorted.sort_unstable();
    StagePercentiles {
        p50_ms: percentile(&sorted, 50),
        p95_ms: percentile(&sorted, 95),
        p99_ms: percentile(&sorted, 99),
    }
}

fn percentile(sorted: &[u64], p: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) * p) / 100;
    sorted[idx]
}

impl QueryCache {
    fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            ttl,
            capacity,
        }
    }

    fn get(&mut self, key: &str) -> Option<String> {
        let now = Instant::now();
        match self.entries.get(key) {
            Some(entry) if entry.expires_at > now => Some(entry.value.clone()),
            Some(_) => {
                self.entries.remove(key);
                None
            }
            None => None,
        }
    }

    fn insert(&mut self, key: String, value: String) {
        let expires_at = Instant::now() + self.ttl;
        self.entries
            .insert(key.clone(), QueryCacheEntry { value, expires_at });
        self.order.push_back(key);

        while self.entries.len() > self.capacity {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }
}

#[derive(Debug, Serialize)]
struct SensorSchemaPayload {
    sensors: Vec<SensorSchemaItem>,
}

#[derive(Debug, Serialize)]
struct SensorSchemaItem {
    sensor_id: String,
    fields: Vec<SensorFieldSchema>,
    trend_metric: Option<String>,
    category_metric: Option<String>,
}

#[derive(Debug, Serialize)]
struct SensorFieldSchema {
    field: String,
    label: String,
    unit: String,
    data_type: String,
    required: bool,
    threshold_low: Option<f64>,
    threshold_high: Option<f64>,
}

#[derive(Debug, Serialize)]
struct DeviceSummary {
    device_id: String,
    location: String,
    crop_type: String,
    farm_note: String,
    sensors: Vec<String>,
    registered_at_epoch_sec: u64,
}

#[derive(Debug, Serialize)]
struct DevicesPayload {
    devices: Vec<DeviceSummary>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ChatProxyRequest {
    message: String,
    #[serde(default)]
    context: serde_json::Value,
}

#[derive(Debug, serde::Deserialize, Serialize)]
struct ChatProxyResponse {
    reply: String,
}

#[derive(Debug, serde::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponseData {
    token: String,
    username: String,
    issued_at_epoch_sec: u64,
    expires_at_epoch_sec: u64,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    success: bool,
    message: String,
    data: Option<LoginResponseData>,
}

#[derive(Debug, Serialize)]
struct AuthErrorPayload {
    status: String,
    code: String,
    message: String,
}

pub fn start_http_server(
    bind_addr: &str,
    image_store_path: String,
    image_index_path: String,
    image_db_error_store_path: String,
    ai_predict_url: String,
    openclaw_url: String,
    sensor_rules: HashMap<String, SensorRule>,
    registry_path: String,
    db: Arc<Mutex<DbManager>>,
    presence: Arc<Mutex<PresenceTracker>>,
) {
    let server = Server::http(bind_addr).expect("Failed to start HTTP server");
    let sensor_schema_payload = build_sensor_schema_payload(&sensor_rules);
    let query_cache = Arc::new(Mutex::new(QueryCache::new(
        QUERY_CACHE_MAX_ENTRIES,
        Duration::from_secs(QUERY_CACHE_TTL_SECONDS),
    )));
    let auth_enabled = auth_enabled_from_env();
    let perf = Arc::new(Mutex::new(PerfMetrics::new(PERF_METRIC_WINDOW_SIZE)));
    let ai_timeout_sec = env_u64("CLOUD_HTTP_AI_TIMEOUT_SEC", 20);
    let ai_pool_idle = env_usize("CLOUD_HTTP_AI_POOL_MAX_IDLE_PER_HOST", 8);
    let openclaw_timeout_sec = env_u64("CLOUD_HTTP_OPENCLAW_TIMEOUT_SEC", 120);
    let openclaw_pool_idle = env_usize("CLOUD_HTTP_OPENCLAW_POOL_MAX_IDLE_PER_HOST", 4);

    let ai_http_client = Arc::new(
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(ai_timeout_sec))
            .pool_max_idle_per_host(ai_pool_idle)
            .build()
            .expect("Failed to build AI HTTP client"),
    );
    let openclaw_http_client = Arc::new(
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(openclaw_timeout_sec))
            .pool_max_idle_per_host(openclaw_pool_idle)
            .build()
            .expect("Failed to build OpenClaw HTTP client"),
    );
    let auth = Arc::new(Mutex::new(AuthManager::from_env()));
    println!(
        "{} [cloud-http] Listening on http://{}",
        now_rfc3339(),
        bind_addr
    );
    println!(
        "{} [cloud-http] auth_enabled={}",
        now_rfc3339(),
        auth_enabled
    );
    println!(
        "{} [cloud-http] ai_timeout_sec={} ai_pool_idle={} openclaw_timeout_sec={} openclaw_pool_idle={}",
        now_rfc3339(),
        ai_timeout_sec,
        ai_pool_idle,
        openclaw_timeout_sec,
        openclaw_pool_idle
    );

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let accepted_at = Instant::now();
            let url = request.url().to_string();
            let method = request.method().clone();
            let (path, query) = split_query(&url);
            let path = path.to_string();
            let query = query.to_string();

            // Clone shared resources for the per-request worker thread.
            let image_store_path = image_store_path.clone();
            let image_index_path = image_index_path.clone();
            let image_db_error_store_path = image_db_error_store_path.clone();
            let ai_predict_url = ai_predict_url.clone();
            let openclaw_url = openclaw_url.clone();
            let sensor_schema_payload = sensor_schema_payload.clone();
            let registry_path = registry_path.clone();
            let db = db.clone();
            let query_cache = query_cache.clone();
            let presence = presence.clone();
            let perf = perf.clone();
            let ai_http_client = ai_http_client.clone();
            let openclaw_http_client = openclaw_http_client.clone();
            let auth = auth.clone();
            let auth_enabled = auth_enabled;

            // Spawn a dedicated worker thread per request so that slow endpoints
            // (AI inference, DB queries) never block static-file serving.
            thread::spawn(move || {
                let queue_wait_ms = accepted_at.elapsed().as_millis() as u64;
                if path.starts_with("/api/") {
                    handle_api(
                        request,
                        method,
                        &path,
                        &query,
                        &image_store_path,
                        &image_index_path,
                        &image_db_error_store_path,
                        &ai_predict_url,
                        &openclaw_url,
                        &sensor_schema_payload,
                        &registry_path,
                        queue_wait_ms,
                        db,
                        query_cache,
                        presence,
                        auth,
                        auth_enabled,
                        perf,
                        &ai_http_client,
                        &openclaw_http_client,
                    );
                    return;
                }

                let mut file_path = path;
                if file_path == "/" {
                    file_path = "/portal/index.html".to_string();
                } else if file_path == "/rice" || file_path == "/rice/" {
                    file_path = "/rice/rice_dashboard.html".to_string();
                } else if file_path == "/oil_palm" || file_path == "/oil_palm/" {
                    file_path = "/oil_palm/index.html".to_string();
                }

                let path = resolve_static_file_path(&file_path);
                if path.exists() && path.is_file() {
                    let content_type = match path.extension().and_then(|s| s.to_str()) {
                        Some("html") => "text/html; charset=utf-8",
                        Some("css") => "text/css",
                        Some("js") => "application/javascript",
                        Some("png") => "image/png",
                        Some("md") => "text/markdown; charset=utf-8",
                        Some("svg") => "image/svg+xml",
                        _ => "application/octet-stream",
                    };

                    let header =
                        Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap();
                    match File::open(path) {
                        Ok(f) => {
                            let response = Response::from_file(f).with_header(header);
                            let _ = request.respond(response);
                        }
                        Err(_) => {
                            let _ = request
                                .respond(Response::from_string("File Error").with_status_code(500));
                        }
                    }
                } else {
                    let _ =
                        request.respond(Response::from_string("Not Found").with_status_code(404));
                }
            });
        }
    });
}

fn handle_api(
    request: tiny_http::Request,
    method: Method,
    path: &str,
    query: &str,
    image_store_path: &str,
    image_index_path: &str,
    image_db_error_store_path: &str,
    ai_predict_url: &str,
    openclaw_url: &str,
    sensor_schema_payload: &str,
    registry_path: &str,
    queue_wait_ms: u64,
    db: Arc<Mutex<DbManager>>,
    query_cache: Arc<Mutex<QueryCache>>,
    presence: Arc<Mutex<PresenceTracker>>,
    auth: Arc<Mutex<AuthManager>>,
    auth_enabled: bool,
    perf: Arc<Mutex<PerfMetrics>>,
    ai_http_client: &reqwest::blocking::Client,
    openclaw_http_client: &reqwest::blocking::Client,
) {
    if auth_enabled && requires_auth(&method, path) {
        match extract_bearer_token(&request) {
            Some(token) => match auth.lock() {
                Ok(mut guard) => {
                    if guard.validate(&token).is_none() {
                        let payload = serde_json::to_string(&AuthErrorPayload {
                            status: "error".to_string(),
                            code: "unauthorized".to_string(),
                            message: "invalid or expired token".to_string(),
                        })
                        .unwrap_or_else(|_| {
                            "{\"status\":\"error\",\"code\":\"unauthorized\",\"message\":\"invalid or expired token\"}".to_string()
                        });
                        respond_json_with_status(request, 401, &payload);
                        return;
                    }
                }
                Err(_) => {
                    let payload = serde_json::to_string(&AuthErrorPayload {
                        status: "error".to_string(),
                        code: "auth_internal_error".to_string(),
                        message: "auth state unavailable".to_string(),
                    })
                    .unwrap_or_else(|_| {
                        "{\"status\":\"error\",\"code\":\"auth_internal_error\",\"message\":\"auth state unavailable\"}".to_string()
                    });
                    respond_json_with_status(request, 503, &payload);
                    return;
                }
            },
            None => {
                let payload = serde_json::to_string(&AuthErrorPayload {
                    status: "error".to_string(),
                    code: "unauthorized".to_string(),
                    message: "missing bearer token".to_string(),
                })
                .unwrap_or_else(|_| {
                    "{\"status\":\"error\",\"code\":\"unauthorized\",\"message\":\"missing bearer token\"}".to_string()
                });
                respond_json_with_status(request, 401, &payload);
                return;
            }
        }
    }

    match (method, path) {
        (Method::Post, "/api/login") => {
            handle_login(request, auth);
        }
        (Method::Post, "/api/logout") => {
            handle_logout(request, auth);
        }
        (Method::Get, "/api/session") => {
            handle_session_check(request, auth);
        }
        (Method::Post, "/api/v1/image/upload") => {
            handle_image_upload(
                request,
                query,
                image_store_path,
                image_index_path,
                image_db_error_store_path,
                ai_predict_url,
                queue_wait_ms,
                db,
                perf,
                ai_http_client,
            );
        }
        (Method::Post, "/api/v1/chat") => {
            handle_chat_proxy(
                request,
                openclaw_url,
                queue_wait_ms,
                perf,
                openclaw_http_client,
            );
        }
        (Method::Get, "/api/v1/image/file") => {
            handle_image_file_request(request, query, image_store_path, db);
        }
        (Method::Get, "/api/v1/image/uploads") => {
            handle_image_upload_query(request, query, db, query_cache);
        }
        (Method::Get, "/api/v1/sensor/schema") => {
            respond_json_with_status(request, 200, sensor_schema_payload);
        }
        (Method::Get, "/api/v1/perf/latency") => {
            handle_perf_query(request, perf);
        }
        (Method::Get, "/api/v1/devices") => {
            handle_devices_query(request, registry_path);
        }
        (Method::Get, "/api/v1/presence") => {
            handle_presence_query(request, presence);
        }
        (Method::Get, "/api/v1/telemetry") | (Method::Get, "/api/telemetry") => {
            handle_telemetry_query(request, query, db, query_cache);
        }
        (Method::Post, "/api/send-code") => {
            respond_json_with_status(
                request,
                410,
                r#"{"status":"error","message":"deprecated endpoint: /api/send-code"}"#,
            );
        }
        (Method::Get, "/api/dashboard") => {
            respond_json_with_status(
                request,
                410,
                r#"{"status":"error","message":"deprecated endpoint: /api/dashboard"}"#,
            );
        }
        (Method::Get, "/api/charts") => {
            respond_json_with_status(
                request,
                410,
                r#"{"status":"error","message":"deprecated endpoint: /api/charts"}"#,
            );
        }
        (Method::Get, "/api/fields") => {
            respond_json_with_status(
                request,
                410,
                r#"{"status":"error","message":"deprecated endpoint: /api/fields"}"#,
            );
        }
        (Method::Get, "/api/v1/plantations") => {
            crate::tree::handle_list_plantations(request, db);
        }
        (Method::Get, "/api/v1/trees") => {
            crate::tree::handle_list_trees(request, query, db);
        }
        (method, p) if p.starts_with("/api/v1/uav/") || p.starts_with("/api/v1/trees/") => {
            if method == Method::Post && p == "/api/v1/uav/missions" {
                crate::uav::handle_missions_post(request, db);
            } else if method == Method::Post && p.ends_with("/orthomosaic") {
                let mission_id = extract_path_segment(p, "/missions/").unwrap_or_default();
                crate::uav::handle_orthomosaic_post(request, &mission_id, db);
            } else if method == Method::Post && p.ends_with("/tiles") {
                let ortho_id = extract_path_segment(p, "/orthomosaics/").unwrap_or_default();
                crate::uav::handle_tiles_post(request, &ortho_id, db);
            } else if method == Method::Post && p.ends_with("/detections/mock") {
                let ortho_id = extract_path_segment(p, "/orthomosaics/").unwrap_or_default();
                crate::uav::handle_mock_detections(request, &ortho_id, db);
            } else if method == Method::Post && p.ends_with("/detect-palms") {
                let ortho_id = extract_path_segment(p, "/orthomosaics/").unwrap_or_default();
                crate::uav::handle_detect_palms(request, &ortho_id, db);
            } else if method == Method::Get && p.contains("/detections") {
                let ortho_id = extract_path_segment(p, "/orthomosaics/").unwrap_or_default();
                crate::uav::handle_get_detections(request, &ortho_id, db);
            } else if method == Method::Post && p.ends_with("/confirm") {
                let det_id = extract_path_segment(p, "/detections/").unwrap_or_default();
                crate::uav::handle_confirm_detection(request, &det_id, db);
            } else if method == Method::Post && p.ends_with("/reject") {
                let det_id = extract_path_segment(p, "/detections/").unwrap_or_default();
                crate::uav::handle_reject_detection(request, &det_id, db);
            } else if method == Method::Get && p.starts_with("/api/v1/trees/") && p.ends_with("/timeline") {
                let tree_code = extract_path_segment(p, "/trees/").unwrap_or_default();
                crate::tree::handle_get_timeline(request, &tree_code, db);
            } else if method == Method::Put && p.starts_with("/api/v1/trees/") && p.ends_with("/status") {
                let tree_code = extract_path_segment(p, "/trees/").unwrap_or_default();
                crate::tree::handle_update_status(request, &tree_code, db);
            } else if method == Method::Get && p.starts_with("/api/v1/trees/") {
                let tree_code = extract_path_segment(p, "/trees/").unwrap_or_default();
                crate::tree::handle_get_tree(request, &tree_code, db);
            } else {
                let _ = request.respond(Response::from_string("API Not Found").with_status_code(404));
            }
        }
        _ => {
            let _ = request.respond(Response::from_string("API Not Found").with_status_code(404));
        }
    }
}

fn extract_path_segment(path: &str, after: &str) -> Option<String> {
    let idx = path.find(after)?;
    let rest = &path[idx + after.len()..];
    let segment = rest.split('/').next()?;
    if segment.is_empty() { None } else { Some(segment.to_string()) }
}

fn requires_auth(method: &Method, path: &str) -> bool {
    if path == "/api/login" || path == "/api/logout" || path == "/api/session" {
        return false;
    }
    if *method == Method::Post && path == "/api/v1/image/upload" {
        return false;
    }
    path.starts_with("/api/v1/") || path == "/api/telemetry"
}

fn auth_enabled_from_env() -> bool {
    std::env::var("CLOUD_AUTH_ENABLED")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        })
        .unwrap_or(false)
}

fn extract_bearer_token(request: &tiny_http::Request) -> Option<String> {
    let value = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .map(|h| h.value.as_str().trim().to_string())?;
    if value.is_empty() {
        return None;
    }
    if value.len() >= 7 && value[..7].eq_ignore_ascii_case("bearer ") {
        let token = value[7..].trim();
        if token.is_empty() {
            return None;
        }
        return Some(token.to_string());
    }
    None
}

fn handle_login(mut request: tiny_http::Request, auth: Arc<Mutex<AuthManager>>) {
    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        let payload = serde_json::to_string(&LoginResponse {
            success: false,
            message: format!("failed to read request body: {err}"),
            data: None,
        })
        .unwrap_or_else(|_| {
            "{\"success\":false,\"message\":\"bad request\",\"data\":null}".to_string()
        });
        respond_json_with_status(request, 400, &payload);
        return;
    }

    let req = match serde_json::from_slice::<LoginRequest>(&body) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&LoginResponse {
                success: false,
                message: format!("invalid json body: {err}"),
                data: None,
            })
            .unwrap_or_else(|_| {
                "{\"success\":false,\"message\":\"bad request\",\"data\":null}".to_string()
            });
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let login_result = auth
        .lock()
        .map_err(|_| "auth state unavailable".to_string())
        .and_then(|mut guard| guard.login(&req.username, &req.password));
    match login_result {
        Ok(session) => {
            let payload = serde_json::to_string(&LoginResponse {
                success: true,
                message: "login ok".to_string(),
                data: Some(LoginResponseData {
                    token: session.token,
                    username: session.username,
                    issued_at_epoch_sec: session.issued_at_epoch_sec,
                    expires_at_epoch_sec: session.expires_at_epoch_sec,
                }),
            })
            .unwrap_or_else(|_| {
                "{\"success\":true,\"message\":\"login ok\",\"data\":null}".to_string()
            });
            respond_json_with_status(request, 200, &payload);
        }
        Err(err) if err.contains("invalid username or password") => {
            let payload = serde_json::to_string(&LoginResponse {
                success: false,
                message: err,
                data: None,
            })
            .unwrap_or_else(|_| {
                "{\"success\":false,\"message\":\"invalid username or password\",\"data\":null}"
                    .to_string()
            });
            respond_json_with_status(request, 401, &payload);
        }
        Err(err) => {
            let payload = serde_json::to_string(&LoginResponse {
                success: false,
                message: err,
                data: None,
            })
            .unwrap_or_else(|_| {
                "{\"success\":false,\"message\":\"auth unavailable\",\"data\":null}".to_string()
            });
            respond_json_with_status(request, 503, &payload);
        }
    }
}

fn handle_logout(request: tiny_http::Request, auth: Arc<Mutex<AuthManager>>) {
    let Some(token) = extract_bearer_token(&request) else {
        let payload = serde_json::to_string(&LoginResponse {
            success: false,
            message: "missing bearer token".to_string(),
            data: None,
        })
        .unwrap_or_else(|_| {
            "{\"success\":false,\"message\":\"missing bearer token\",\"data\":null}".to_string()
        });
        respond_json_with_status(request, 401, &payload);
        return;
    };

    let removed = auth
        .lock()
        .map_err(|_| "auth state unavailable".to_string())
        .map(|mut guard| guard.logout(&token));
    match removed {
        Ok(_) => {
            let payload = serde_json::to_string(&LoginResponse {
                success: true,
                message: "logout ok".to_string(),
                data: None,
            })
            .unwrap_or_else(|_| {
                "{\"success\":true,\"message\":\"logout ok\",\"data\":null}".to_string()
            });
            respond_json_with_status(request, 200, &payload);
        }
        Err(err) => {
            let payload = serde_json::to_string(&LoginResponse {
                success: false,
                message: err,
                data: None,
            })
            .unwrap_or_else(|_| {
                "{\"success\":false,\"message\":\"auth unavailable\",\"data\":null}".to_string()
            });
            respond_json_with_status(request, 503, &payload);
        }
    }
}

fn handle_session_check(request: tiny_http::Request, auth: Arc<Mutex<AuthManager>>) {
    let Some(token) = extract_bearer_token(&request) else {
        let payload = serde_json::to_string(&AuthErrorPayload {
            status: "error".to_string(),
            code: "unauthorized".to_string(),
            message: "missing bearer token".to_string(),
        })
        .unwrap_or_else(|_| {
            "{\"status\":\"error\",\"code\":\"unauthorized\",\"message\":\"missing bearer token\"}"
                .to_string()
        });
        respond_json_with_status(request, 401, &payload);
        return;
    };

    let session = auth
        .lock()
        .map_err(|_| "auth state unavailable".to_string())
        .ok()
        .and_then(|mut guard| guard.validate(&token));
    let Some(AuthSession {
        username,
        issued_at_epoch_sec,
        expires_at_epoch_sec,
        ..
    }) = session
    else {
        let payload = serde_json::to_string(&AuthErrorPayload {
            status: "error".to_string(),
            code: "unauthorized".to_string(),
            message: "invalid or expired token".to_string(),
        })
        .unwrap_or_else(|_| {
            "{\"status\":\"error\",\"code\":\"unauthorized\",\"message\":\"invalid or expired token\"}".to_string()
        });
        respond_json_with_status(request, 401, &payload);
        return;
    };

    let payload = serde_json::json!({
        "status": "ok",
        "username": username,
        "issued_at_epoch_sec": issued_at_epoch_sec,
        "expires_at_epoch_sec": expires_at_epoch_sec
    })
    .to_string();
    respond_json_with_status(request, 200, &payload);
}

fn handle_telemetry_query(
    request: tiny_http::Request,
    query: &str,
    db: Arc<Mutex<DbManager>>,
    query_cache: Arc<Mutex<QueryCache>>,
) {
    let params = parse_query(query);
    let start_time = match parse_optional_rfc3339(params.get("start_time").map(|v| v.as_str())) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };
    let end_time = match parse_optional_rfc3339(params.get("end_time").map(|v| v.as_str())) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };
    let filter = SensorTelemetryQueryFilter {
        start_time,
        end_time,
        device_id: non_empty(params.get("device_id").cloned()),
        sensor_id: non_empty(params.get("sensor_id").cloned()),
        limit: params
            .get("limit")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100)
            .clamp(1, 1000),
    };

    let cache_key = format!(
        "telemetry|{:?}|{:?}|{:?}|{:?}|{}",
        filter.start_time, filter.end_time, filter.device_id, filter.sensor_id, filter.limit
    );
    if let Ok(mut cache) = query_cache.lock() {
        if let Some(payload) = cache.get(cache_key.as_str()) {
            respond_json_with_status(request, 200, &payload);
            return;
        }
    }

    let db_result = db
        .lock()
        .map_err(|_| "db lock poisoned".to_string())
        .and_then(|mut guard| guard.query_sensor_telemetry(&filter));
    match db_result {
        Ok(rows) => {
            let body = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string());
            if let Ok(mut cache) = query_cache.lock() {
                cache.insert(cache_key, body.clone());
            }
            respond_json_with_status(request, 200, &body);
        }
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("database query failed: {err}"),
            })
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"database query failed\"}".to_string()
            });
            respond_json_with_status(request, 503, &payload);
        }
    }
}

fn handle_chat_proxy(
    mut request: tiny_http::Request,
    openclaw_url: &str,
    queue_wait_ms: u64,
    perf: Arc<Mutex<PerfMetrics>>,
    http_client: &reqwest::blocking::Client,
) {
    let req_started = Instant::now();
    let mut body = Vec::new();
    let read_started = Instant::now();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        if let Ok(mut m) = perf.lock() {
            m.chat_proxy.total_requests += 1;
            m.chat_proxy.failed_requests += 1;
            m.chat_proxy.queue_ms.push(queue_wait_ms);
            m.chat_proxy
                .read_body_ms
                .push(read_started.elapsed().as_millis() as u64);
            m.chat_proxy
                .total_ms
                .push(req_started.elapsed().as_millis() as u64);
        }
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("failed to read request body: {err}"),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 400, &payload);
        return;
    }

    let req: ChatProxyRequest = match serde_json::from_slice::<ChatProxyRequest>(&body) {
        Ok(v) if !v.message.trim().is_empty() => v,
        Ok(_) => {
            if let Ok(mut m) = perf.lock() {
                m.chat_proxy.total_requests += 1;
                m.chat_proxy.failed_requests += 1;
                m.chat_proxy.queue_ms.push(queue_wait_ms);
                m.chat_proxy
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.chat_proxy
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: "message must not be empty".to_string(),
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.chat_proxy.total_requests += 1;
                m.chat_proxy.failed_requests += 1;
                m.chat_proxy.queue_ms.push(queue_wait_ms);
                m.chat_proxy
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.chat_proxy
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("invalid json body: {err}"),
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let forward_url = format!("{}/api/v1/chat", openclaw_url.trim_end_matches('/'));
    let upstream_started = Instant::now();
    let upstream = http_client.post(forward_url).json(&req).send();
    let upstream_ms = upstream_started.elapsed().as_millis() as u64;

    let upstream = match upstream {
        Ok(v) => v,
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.chat_proxy.total_requests += 1;
                m.chat_proxy.failed_requests += 1;
                m.chat_proxy.queue_ms.push(queue_wait_ms);
                m.chat_proxy
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.chat_proxy.ai_infer_ms.push(upstream_ms);
                m.chat_proxy
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("openclaw request failed: {err}"),
            })
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"upstream failed\"}".to_string()
            });
            respond_json_with_status(request, 503, &payload);
            return;
        }
    };

    let status = upstream.status();
    let text = match upstream.text() {
        Ok(v) => v,
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.chat_proxy.total_requests += 1;
                m.chat_proxy.failed_requests += 1;
                m.chat_proxy.queue_ms.push(queue_wait_ms);
                m.chat_proxy
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.chat_proxy.ai_infer_ms.push(upstream_ms);
                m.chat_proxy
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("failed to read openclaw response: {err}"),
            })
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"upstream failed\"}".to_string()
            });
            respond_json_with_status(request, 503, &payload);
            return;
        }
    };

    if !status.is_success() {
        if let Ok(mut m) = perf.lock() {
            m.chat_proxy.total_requests += 1;
            m.chat_proxy.failed_requests += 1;
            m.chat_proxy.queue_ms.push(queue_wait_ms);
            m.chat_proxy
                .read_body_ms
                .push(read_started.elapsed().as_millis() as u64);
            m.chat_proxy.ai_infer_ms.push(upstream_ms);
            m.chat_proxy
                .total_ms
                .push(req_started.elapsed().as_millis() as u64);
        }
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("openclaw returned {}", status.as_u16()),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"upstream failed\"}".to_string());
        respond_json_with_status(request, 503, &payload);
        return;
    }

    if let Ok(parsed) = serde_json::from_str::<ChatProxyResponse>(&text) {
        if let Ok(mut m) = perf.lock() {
            m.chat_proxy.total_requests += 1;
            m.chat_proxy.success_requests += 1;
            m.chat_proxy.queue_ms.push(queue_wait_ms);
            m.chat_proxy
                .read_body_ms
                .push(read_started.elapsed().as_millis() as u64);
            m.chat_proxy.ai_infer_ms.push(upstream_ms);
            m.chat_proxy
                .total_ms
                .push(req_started.elapsed().as_millis() as u64);
        }
        let payload =
            serde_json::to_string(&parsed).unwrap_or_else(|_| "{\"reply\":\"\"}".to_string());
        respond_json_with_status(request, 200, &payload);
        return;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(reply) = v
            .get("reply")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                v.get("message")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
        {
            if let Ok(mut m) = perf.lock() {
                m.chat_proxy.total_requests += 1;
                m.chat_proxy.success_requests += 1;
                m.chat_proxy.queue_ms.push(queue_wait_ms);
                m.chat_proxy
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.chat_proxy.ai_infer_ms.push(upstream_ms);
                m.chat_proxy
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ChatProxyResponse { reply })
                .unwrap_or_else(|_| "{\"reply\":\"\"}".to_string());
            respond_json_with_status(request, 200, &payload);
            return;
        }
    }

    if let Ok(mut m) = perf.lock() {
        m.chat_proxy.total_requests += 1;
        m.chat_proxy.failed_requests += 1;
        m.chat_proxy.queue_ms.push(queue_wait_ms);
        m.chat_proxy
            .read_body_ms
            .push(read_started.elapsed().as_millis() as u64);
        m.chat_proxy.ai_infer_ms.push(upstream_ms);
        m.chat_proxy
            .total_ms
            .push(req_started.elapsed().as_millis() as u64);
    }
    let payload = serde_json::to_string(&ImageUploadErrorResponse {
        status: "error".to_string(),
        message: "openclaw response missing reply field".to_string(),
    })
    .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"upstream bad response\"}".to_string());
    respond_json_with_status(request, 503, &payload);
}

fn handle_image_file_request(
    request: tiny_http::Request,
    query: &str,
    image_store_path: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let q = parse_query(query);
    let saved_path_raw = q
        .get("upload_id")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .and_then(|upload_id| {
            db.lock()
                .ok()
                .and_then(|mut guard| guard.get_saved_path_by_upload_id(upload_id).ok())
                .flatten()
        })
        .or_else(|| {
            q.get("saved_path")
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
        .or_else(|| {
            q.get("path")
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        });
    let Some(saved_path_raw) = saved_path_raw else {
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: "missing saved_path/upload_id".to_string(),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 400, &payload);
        return;
    };

    let cwd = match std::env::current_dir() {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("cannot resolve working dir: {err}"),
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"server error\"}".to_string());
            respond_json_with_status(request, 500, &payload);
            return;
        }
    };

    let store_root = {
        let root = PathBuf::from(image_store_path);
        if root.is_absolute() {
            root
        } else {
            cwd.join(root)
        }
    };
    let store_root = match fs::canonicalize(store_root) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("image store path not available: {err}"),
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"server error\"}".to_string());
            respond_json_with_status(request, 500, &payload);
            return;
        }
    };

    let candidate = {
        let p = PathBuf::from(saved_path_raw);
        if p.is_absolute() {
            p
        } else {
            cwd.join(p)
        }
    };
    let candidate = match fs::canonicalize(candidate) {
        Ok(v) => v,
        Err(_) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: "file not found".to_string(),
            })
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"file not found\"}".to_string()
            });
            respond_json_with_status(request, 404, &payload);
            return;
        }
    };

    if !candidate.starts_with(&store_root) {
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: "saved_path out of allowed image_store_path".to_string(),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"forbidden\"}".to_string());
        respond_json_with_status(request, 403, &payload);
        return;
    }

    let bytes = match fs::read(&candidate) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("failed to read file: {err}"),
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"server error\"}".to_string());
            respond_json_with_status(request, 500, &payload);
            return;
        }
    };

    let content_type = match candidate
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
    {
        Some(ext) if ext == "png" => "image/png",
        Some(ext) if ext == "jpg" || ext == "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    };

    let response = Response::from_data(bytes).with_header(
        Header::from_bytes("Content-Type", content_type).unwrap_or_else(|_| {
            Header::from_bytes("Content-Type", "application/octet-stream")
                .expect("static content-type header")
        }),
    );
    let _ = request.respond(response);
}

fn handle_image_upload(
    mut request: tiny_http::Request,
    query: &str,
    image_store_path: &str,
    image_index_path: &str,
    image_db_error_store_path: &str,
    ai_predict_url: &str,
    queue_wait_ms: u64,
    db: Arc<Mutex<DbManager>>,
    perf: Arc<Mutex<PerfMetrics>>,
    ai_http_client: &reqwest::blocking::Client,
) {
    let req_started = Instant::now();
    let tag = match parse_tag(&parse_query(query)) {
        Ok(v) => v,
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.image_upload.total_requests += 1;
                m.image_upload.failed_requests += 1;
                m.image_upload.queue_ms.push(queue_wait_ms);
                m.image_upload
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let content_type = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();
    if !content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        if let Ok(mut m) = perf.lock() {
            m.image_upload.total_requests += 1;
            m.image_upload.failed_requests += 1;
            m.image_upload.queue_ms.push(queue_wait_ms);
            m.image_upload
                .total_ms
                .push(req_started.elapsed().as_millis() as u64);
        }
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: "Content-Type must be multipart/form-data".to_string(),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 400, &payload);
        return;
    }

    let mut body = Vec::new();
    let read_started = Instant::now();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        if let Ok(mut m) = perf.lock() {
            m.image_upload.total_requests += 1;
            m.image_upload.failed_requests += 1;
            m.image_upload.queue_ms.push(queue_wait_ms);
            m.image_upload
                .read_body_ms
                .push(read_started.elapsed().as_millis() as u64);
            m.image_upload
                .total_ms
                .push(req_started.elapsed().as_millis() as u64);
        }
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("failed to read request body: {err}"),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 400, &payload);
        return;
    }

    let parse_started = Instant::now();
    let file_part = match parse_multipart_file(&content_type, &body) {
        Ok(v) => v,
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.image_upload.total_requests += 1;
                m.image_upload.failed_requests += 1;
                m.image_upload.queue_ms.push(queue_wait_ms);
                m.image_upload
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.image_upload
                    .parse_multipart_ms
                    .push(parse_started.elapsed().as_millis() as u64);
                m.image_upload
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let save_started = Instant::now();
    let persisted = match save_image_file(image_store_path, &tag, &file_part) {
        Ok(v) => v,
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.image_upload.total_requests += 1;
                m.image_upload.failed_requests += 1;
                m.image_upload.queue_ms.push(queue_wait_ms);
                m.image_upload
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.image_upload
                    .parse_multipart_ms
                    .push(parse_started.elapsed().as_millis() as u64);
                m.image_upload
                    .save_file_ms
                    .push(save_started.elapsed().as_millis() as u64);
                m.image_upload
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let _ = append_image_error_backup(image_db_error_store_path, &tag, &err, None);
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let captured_at = match parse_captured_at_utc(&tag.ts) {
        Ok(v) => v,
        Err(err) => {
            if let Ok(mut m) = perf.lock() {
                m.image_upload.total_requests += 1;
                m.image_upload.failed_requests += 1;
                m.image_upload.queue_ms.push(queue_wait_ms);
                m.image_upload
                    .read_body_ms
                    .push(read_started.elapsed().as_millis() as u64);
                m.image_upload
                    .parse_multipart_ms
                    .push(parse_started.elapsed().as_millis() as u64);
                m.image_upload
                    .save_file_ms
                    .push(save_started.elapsed().as_millis() as u64);
                m.image_upload
                    .total_ms
                    .push(req_started.elapsed().as_millis() as u64);
            }
            let _ =
                append_image_error_backup(image_db_error_store_path, &tag, &err, Some(&persisted));
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let db_record = ImageUploadDbRecord {
        upload_id: persisted.upload_id.clone(),
        device_id: tag.device_id.clone(),
        captured_at,
        received_at: Utc::now(),
        location: tag.location.clone(),
        crop_type: tag.crop_type.clone(),
        farm_note: tag.farm_note.clone(),
        saved_path: persisted.saved_path.clone(),
        sha256: persisted.sha256.clone(),
        image_type: persisted.image_type.clone(),
        file_size: persisted.file_size as i64,
        upload_status: "stored".to_string(),
        error_message: None,
    };
    let db_store_started = Instant::now();
    let db_result = db
        .lock()
        .map_err(|_| "db lock poisoned".to_string())
        .and_then(|mut guard| guard.insert_image_upload(&db_record));
    if let Err(err) = db_result {
        if let Ok(mut m) = perf.lock() {
            m.image_upload.total_requests += 1;
            m.image_upload.failed_requests += 1;
            m.image_upload.queue_ms.push(queue_wait_ms);
            m.image_upload
                .read_body_ms
                .push(read_started.elapsed().as_millis() as u64);
            m.image_upload
                .parse_multipart_ms
                .push(parse_started.elapsed().as_millis() as u64);
            m.image_upload
                .save_file_ms
                .push(save_started.elapsed().as_millis() as u64);
            m.image_upload
                .db_store_ms
                .push(db_store_started.elapsed().as_millis() as u64);
            m.image_upload
                .total_ms
                .push(req_started.elapsed().as_millis() as u64);
        }
        let _ = append_image_error_backup(image_db_error_store_path, &tag, &err, Some(&persisted));
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("database write failed: {err}"),
        })
        .unwrap_or_else(|_| {
            "{\"status\":\"error\",\"message\":\"database write failed\"}".to_string()
        });
        respond_json_with_status(request, 503, &payload);
        return;
    }

    let ai_started = Instant::now();
    let infer_result = infer_image_from_bytes(
        ai_http_client,
        ai_predict_url,
        &file_part.body,
        file_part.filename.as_deref(),
        &persisted.image_type,
    );
    let ai_infer_ms = ai_started.elapsed().as_millis() as u64;
    let db_finalize_started = Instant::now();
    let mut pipeline_ok = true;
    match infer_result {
        Ok(ai) => {
            let inference_record = to_inference_record(&persisted.upload_id, captured_at, ai);
            let write_result = db
                .lock()
                .map_err(|_| "db lock poisoned".to_string())
                .and_then(|mut guard| guard.insert_inference_and_mark_inferred(&inference_record));
            if let Err(err) = write_result {
                let _ = db
                    .lock()
                    .map_err(|_| "db lock poisoned".to_string())
                    .and_then(|mut guard| {
                        guard.update_upload_status(
                            &persisted.upload_id,
                            captured_at,
                            "failed",
                            Some(format!("db write inference failed: {err}")),
                        )
                    });
                let _ = append_image_error_backup(
                    image_db_error_store_path,
                    &tag,
                    &format!("db write inference failed: {err}"),
                    Some(&persisted),
                );
                pipeline_ok = false;
            }
        }
        Err(err) => {
            let _ = db
                .lock()
                .map_err(|_| "db lock poisoned".to_string())
                .and_then(|mut guard| {
                    guard.update_upload_status(
                        &persisted.upload_id,
                        captured_at,
                        "failed",
                        Some(err.clone()),
                    )
                });
            let _ =
                append_image_error_backup(image_db_error_store_path, &tag, &err, Some(&persisted));
            pipeline_ok = false;
        }
    }

    if let Err(err) = append_image_index_backup(image_index_path, &tag, &persisted) {
        eprintln!(
            "{} [cloud-http] WARN: append image index backup failed: {}",
            now_rfc3339(),
            err
        );
    }

    let response_started = Instant::now();
    let ok = build_upload_ok_response(&tag, &persisted, file_part.filename.as_deref());
    let payload = serde_json::to_string(&ok).unwrap_or_else(|_| {
        serde_json::to_string(&ImageUploadOkResponse {
            status: "success".to_string(),
            message: "image upload accepted".to_string(),
            upload_id: String::new(),
            saved_path: String::new(),
            tag,
        })
        .unwrap_or_else(|_| "{\"status\":\"success\"}".to_string())
    });
    respond_json_with_status(request, 200, &payload);

    if let Ok(mut m) = perf.lock() {
        m.image_upload.total_requests += 1;
        if pipeline_ok {
            m.image_upload.success_requests += 1;
        } else {
            m.image_upload.failed_requests += 1;
        }
        m.image_upload.queue_ms.push(queue_wait_ms);
        m.image_upload
            .read_body_ms
            .push(read_started.elapsed().as_millis() as u64);
        m.image_upload
            .parse_multipart_ms
            .push(parse_started.elapsed().as_millis() as u64);
        m.image_upload
            .save_file_ms
            .push(save_started.elapsed().as_millis() as u64);
        m.image_upload
            .db_store_ms
            .push(db_store_started.elapsed().as_millis() as u64);
        m.image_upload.ai_infer_ms.push(ai_infer_ms);
        m.image_upload
            .db_finalize_ms
            .push(db_finalize_started.elapsed().as_millis() as u64);
        m.image_upload
            .response_build_ms
            .push(response_started.elapsed().as_millis() as u64);
        m.image_upload
            .total_ms
            .push(req_started.elapsed().as_millis() as u64);
    }
}

fn handle_image_upload_query(
    request: tiny_http::Request,
    query: &str,
    db: Arc<Mutex<DbManager>>,
    query_cache: Arc<Mutex<QueryCache>>,
) {
    let params = parse_query(query);
    let start_time = match parse_optional_rfc3339(params.get("start_time").map(|v| v.as_str())) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };
    let end_time = match parse_optional_rfc3339(params.get("end_time").map(|v| v.as_str())) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
    };

    let filter = ImageUploadQueryFilter {
        start_time,
        end_time,
        device_id: non_empty(params.get("device_id").cloned()),
        crop_type: non_empty(params.get("crop_type").cloned()),
        upload_status: non_empty(params.get("upload_status").cloned()),
        predicted_class: non_empty(params.get("predicted_class").cloned()),
        limit: params
            .get("limit")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100)
            .clamp(1, 1000),
    };

    let cache_key = format!(
        "image_uploads|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}",
        filter.start_time,
        filter.end_time,
        filter.device_id,
        filter.crop_type,
        filter.upload_status,
        filter.predicted_class,
        filter.limit
    );
    if let Ok(mut cache) = query_cache.lock() {
        if let Some(payload) = cache.get(cache_key.as_str()) {
            respond_json_with_status(request, 200, &payload);
            return;
        }
    }

    let rows = db
        .lock()
        .map_err(|_| "db lock poisoned".to_string())
        .and_then(|mut guard| guard.query_image_uploads(&filter));
    match rows {
        Ok(items) => {
            let payload = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
            if let Ok(mut cache) = query_cache.lock() {
                cache.insert(cache_key, payload.clone());
            }
            respond_json_with_status(request, 200, &payload);
        }
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: format!("database query failed: {err}"),
            })
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"database query failed\"}".to_string()
            });
            respond_json_with_status(request, 503, &payload);
        }
    }
}

fn handle_perf_query(request: tiny_http::Request, perf: Arc<Mutex<PerfMetrics>>) {
    let snapshot = match perf.lock() {
        Ok(metrics) => metrics.snapshot(),
        Err(_) => {
            respond_json_with_status(
                request,
                500,
                r#"{"status":"error","message":"perf metrics lock poisoned"}"#,
            );
            return;
        }
    };

    let payload = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
    respond_json_with_status(request, 200, &payload);
}

fn parse_optional_rfc3339(raw: Option<&str>) -> Result<Option<DateTime<Utc>>, String> {
    let Some(value) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|e| format!("invalid RFC3339 timestamp '{value}': {e}"))?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

fn non_empty(raw: Option<String>) -> Option<String> {
    raw.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn handle_devices_query(request: tiny_http::Request, registry_path: &str) {
    let devices: Vec<DeviceSummary> = fs::read_to_string(registry_path)
        .ok()
        .and_then(|content| serde_json::from_str::<DeviceRegistryFile>(&content).ok())
        .map(|file| {
            let mut list: Vec<DeviceSummary> = file
                .devices
                .into_values()
                .map(|d| DeviceSummary {
                    device_id: d.device_id,
                    location: d.location,
                    crop_type: d.crop_type,
                    farm_note: d.farm_note,
                    sensors: d.sensors,
                    registered_at_epoch_sec: d.registered_at_epoch_sec,
                })
                .collect();
            list.sort_by(|a, b| a.device_id.cmp(&b.device_id));
            list
        })
        .unwrap_or_default();

    let payload = serde_json::to_string(&DevicesPayload { devices })
        .unwrap_or_else(|_| r#"{"devices":[]}"#.to_string());
    respond_json_with_status(request, 200, &payload);
}

fn handle_presence_query(request: tiny_http::Request, presence: Arc<Mutex<PresenceTracker>>) {
    let snapshot = match presence.lock() {
        Ok(guard) => guard.snapshot(),
        Err(_) => {
            respond_json_with_status(
                request,
                500,
                r#"{"status":"error","message":"presence state unavailable"}"#,
            );
            return;
        }
    };
    let payload = serde_json::to_string(&snapshot).unwrap_or_else(|_| "[]".to_string());
    respond_json_with_status(request, 200, &payload);
}

fn build_sensor_schema_payload(sensor_rules: &HashMap<String, SensorRule>) -> String {
    let mut entries = sensor_rules.iter().collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let sensors = entries
        .into_iter()
        .map(|(sensor_id, rule)| to_sensor_schema_item(sensor_id, rule))
        .collect::<Vec<_>>();

    serde_json::to_string(&SensorSchemaPayload { sensors }).unwrap_or_else(|_| {
        "{\"sensors\":[],\"status\":\"error\",\"message\":\"schema serialize failed\"}".to_string()
    })
}

fn to_sensor_schema_item(sensor_id: &str, rule: &SensorRule) -> SensorSchemaItem {
    let mut fields = rule.field_types.iter().collect::<Vec<_>>();
    fields.sort_by(|a, b| a.0.cmp(b.0));

    let fields = fields
        .into_iter()
        .map(|(field, ty)| {
            let required = rule.required_fields.iter().any(|x| x == field);
            let (label, unit, threshold_low, threshold_high) = get_field_metadata(field, *ty);
            SensorFieldSchema {
                field: field.clone(),
                label: label.to_string(),
                unit: unit.to_string(),
                data_type: field_type_name(*ty).to_string(),
                required,
                threshold_low,
                threshold_high,
            }
        })
        .collect::<Vec<_>>();

    SensorSchemaItem {
        sensor_id: sensor_id.to_string(),
        trend_metric: infer_trend_metric(sensor_id, &fields),
        category_metric: infer_category_metric(sensor_id, &fields),
        fields,
    }
}

fn field_type_name(value: FieldType) -> &'static str {
    match value {
        FieldType::String => "string",
        FieldType::Bool => "bool",
        FieldType::U8 => "u8",
        FieldType::U16 => "u16",
        FieldType::U32 => "u32",
        FieldType::I32 => "i32",
        FieldType::F32 => "f32",
        FieldType::F64 => "f64",
    }
}

fn get_field_metadata(
    field: &str,
    _field_type: FieldType,
) -> (&'static str, &'static str, Option<f64>, Option<f64>) {
    match field {
        "vwc" => ("Soil Humidity", "%", Some(20.0), Some(70.0)),
        "temp_c" => ("Temperature", "C", Some(0.0), Some(45.0)),
        "ec" => ("Conductivity", "mS/cm", Some(0.0), Some(5000.0)),
        "hum" => ("Air Humidity", "%", Some(30.0), Some(85.0)),
        "voltage" => ("Voltage", "V", Some(0.0), Some(5.0)),
        "raw" => ("Raw Value", "", None, None),
        "ain0" => ("AIN0", "", None, None),
        "ain1" => ("AIN1", "", None, None),
        "ain2" => ("AIN2", "", None, None),
        "ain3" => ("AIN3", "", None, None),
        "slave_id" => ("Slave ID", "", None, None),
        "protocol" => ("Protocol", "", None, None),
        "pin" => ("Pin", "", None, None),
        "addr" => ("Address", "", None, None),
        _ => ("Field", "", None, None),
    }
}

fn infer_trend_metric(sensor_id: &str, fields: &[SensorFieldSchema]) -> Option<String> {
    if sensor_id == "soil_modbus_02" {
        return Some("ec".to_string());
    }
    for candidate in ["temp_c", "hum", "vwc", "ec", "voltage", "raw"] {
        if fields.iter().any(|f| f.field == candidate) {
            return Some(candidate.to_string());
        }
    }
    fields
        .iter()
        .find(|f| f.data_type != "string" && f.data_type != "bool")
        .map(|f| f.field.clone())
}

fn infer_category_metric(sensor_id: &str, fields: &[SensorFieldSchema]) -> Option<String> {
    if sensor_id == "soil_modbus_02" && fields.iter().any(|f| f.field == "slave_id") {
        return Some("slave_id".to_string());
    }
    if fields.iter().any(|f| f.field == "protocol") {
        return Some("protocol".to_string());
    }
    None
}

fn to_inference_record(
    upload_id: &str,
    captured_at: DateTime<Utc>,
    ai: AiInferenceOutput,
) -> ImageInferenceDbRecord {
    ImageInferenceDbRecord {
        upload_id: upload_id.to_string(),
        captured_at,
        predicted_class: ai.predicted_class,
        confidence: ai.confidence,
        model_version: ai.model_version,
        topk_json: ai.topk_json,
        metadata_json: ai.metadata_json,
        geometry_json: ai.geometry_json,
        latency_ms: ai.latency_ms,
        advice_code: ai.advice_code,
    }
}

fn split_query(url: &str) -> (&str, &str) {
    match url.split_once('?') {
        Some((path, query)) => (path, query),
        None => (url, ""),
    }
}

fn resolve_static_file_path(file_path: &str) -> PathBuf {
    let normalized = file_path.trim_start_matches('/');
    let preferred = PathBuf::from("frontend").join(normalized);
    if preferred.exists() {
        return preferred;
    }
    PathBuf::from("dashboard").join(normalized)
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (pair.trim(), ""),
        };
        if key.is_empty() {
            continue;
        }
        out.insert(
            decode_query_component(key).unwrap_or_else(|| key.to_string()),
            decode_query_component(value).unwrap_or_else(|| value.to_string()),
        );
    }
    out
}

fn decode_query_component(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return Some(String::new());
    }

    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_val(bytes[i + 1])?;
                let lo = hex_val(bytes[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b'%' => return None,
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(ch: u8) -> Option<u8> {
    match ch {
        b'0'..=b'9' => Some(ch - b'0'),
        b'a'..=b'f' => Some(ch - b'a' + 10),
        b'A'..=b'F' => Some(ch - b'A' + 10),
        _ => None,
    }
}

fn respond_json_with_status(request: tiny_http::Request, code: u16, payload: &str) {
    let header = Header::from_bytes(
        &b"Content-Type"[..],
        &b"application/json; charset=utf-8"[..],
    )
    .unwrap();
    let _ = request.respond(
        Response::from_string(payload.to_string())
            .with_header(header)
            .with_status_code(code),
    );
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::model::{FieldType, SensorRule};

    use super::parse_query;

    #[test]
    fn parse_query_decodes_percent_encoded_values() {
        let params = parse_query("ts=2026-04-18T03%3A35%3A22.813%2B08%3A00&location=test+plot");
        assert_eq!(
            params.get("ts").map(String::as_str),
            Some("2026-04-18T03:35:22.813+08:00")
        );
        assert_eq!(
            params.get("location").map(String::as_str),
            Some("test plot")
        );
    }

    #[test]
    fn build_sensor_schema_payload_contains_fields() {
        let mut rules = HashMap::new();
        let mut field_types = HashMap::new();
        field_types.insert("temp_c".to_string(), FieldType::F32);
        field_types.insert("hum".to_string(), FieldType::F32);
        rules.insert(
            "dht22".to_string(),
            SensorRule {
                ack: "ack:dht22".to_string(),
                required_fields: vec!["temp_c".to_string()],
                field_types,
            },
        );
        let payload = super::build_sensor_schema_payload(&rules);
        assert!(payload.contains("\"sensor_id\":\"dht22\""));
        assert!(payload.contains("\"field\":\"temp_c\""));
        assert!(payload.contains("\"required\":true"));
    }
}
