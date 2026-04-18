use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tiny_http::{Header, Method, Response, Server};

use crate::ai_client::{infer_image_from_file, AiInferenceOutput};
use crate::db::{
    DbManager, ImageInferenceDbRecord, ImageUploadDbRecord, ImageUploadQueryFilter,
    SensorTelemetryQueryFilter,
};
use crate::image_upload::{
    append_image_error_backup, append_image_index_backup, build_upload_ok_response,
    parse_captured_at_utc, parse_multipart_file, parse_tag, save_image_file,
    ImageUploadErrorResponse, ImageUploadOkResponse,
};
use crate::model::{FieldType, SensorRule};
use crate::time_util::now_rfc3339;

const QUERY_CACHE_TTL_SECONDS: u64 = 15;
const QUERY_CACHE_MAX_ENTRIES: usize = 500;

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

pub fn start_http_server(
    bind_addr: &str,
    image_store_path: String,
    image_index_path: String,
    image_db_error_store_path: String,
    ai_predict_url: String,
    openclaw_url: String,
    sensor_rules: HashMap<String, SensorRule>,
    db: Arc<Mutex<DbManager>>,
) {
    let server = Server::http(bind_addr).expect("Failed to start HTTP server");
    let sensor_schema_payload = build_sensor_schema_payload(&sensor_rules);
    let query_cache = Arc::new(Mutex::new(QueryCache::new(
        QUERY_CACHE_MAX_ENTRIES,
        Duration::from_secs(QUERY_CACHE_TTL_SECONDS),
    )));
    println!(
        "{} [cloud-http] Listening on http://{}",
        now_rfc3339(),
        bind_addr
    );

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let method = request.method().clone();
            let (path, query) = split_query(&url);

            if path.starts_with("/api/") {
                handle_api(
                    request,
                    method,
                    path,
                    query,
                    &image_store_path,
                    &image_index_path,
                    &image_db_error_store_path,
                    &ai_predict_url,
                    &openclaw_url,
                    &sensor_schema_payload,
                    db.clone(),
                    query_cache.clone(),
                );
                continue;
            }

            let mut file_path = path.to_string();
            if file_path == "/" {
                file_path = "/index.html".to_string();
            }

            let path = resolve_static_file_path(&file_path);
            if path.exists() && path.is_file() {
                let content_type = match path.extension().and_then(|s| s.to_str()) {
                    Some("html") => "text/html; charset=utf-8",
                    Some("css") => "text/css",
                    Some("js") => "application/javascript",
                    Some("png") => "image/png",
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
                let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
            }
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
    db: Arc<Mutex<DbManager>>,
    query_cache: Arc<Mutex<QueryCache>>,
) {
    let respond_json = move |json: &str, req: tiny_http::Request| {
        let header = Header::from_bytes(
            &b"Content-Type"[..],
            &b"application/json; charset=utf-8"[..],
        )
        .unwrap();
        let _ = req.respond(Response::from_string(json).with_header(header));
    };

    match (method, path) {
        (Method::Post, "/api/v1/image/upload") => {
            handle_image_upload(
                request,
                query,
                image_store_path,
                image_index_path,
                image_db_error_store_path,
                ai_predict_url,
                db,
            );
        }
        (Method::Post, "/api/v1/chat") => {
            handle_chat_proxy(request, openclaw_url);
        }
        (Method::Get, "/api/v1/image/uploads") => {
            handle_image_upload_query(request, query, db, query_cache);
        }
        (Method::Get, "/api/v1/sensor/schema") => {
            respond_json_with_status(request, 200, sensor_schema_payload);
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
        (Method::Post, "/api/login") => {
            respond_json(
                r#"{
                "success": true, 
                "message": "login compatibility endpoint", 
                "data": null
            }"#,
                request,
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
        _ => {
            let _ = request.respond(Response::from_string("API Not Found").with_status_code(404));
        }
    }
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

fn handle_chat_proxy(mut request: tiny_http::Request, openclaw_url: &str) {
    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
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
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: "message must not be empty".to_string(),
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 400, &payload);
            return;
        }
        Err(err) => {
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
    let upstream = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .and_then(|client| client.post(forward_url).json(&req).send());

    let upstream = match upstream {
        Ok(v) => v,
        Err(err) => {
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
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("openclaw returned {}", status.as_u16()),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"upstream failed\"}".to_string());
        respond_json_with_status(request, 503, &payload);
        return;
    }

    if let Ok(parsed) = serde_json::from_str::<ChatProxyResponse>(&text) {
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
            let payload = serde_json::to_string(&ChatProxyResponse { reply })
                .unwrap_or_else(|_| "{\"reply\":\"\"}".to_string());
            respond_json_with_status(request, 200, &payload);
            return;
        }
    }

    let payload = serde_json::to_string(&ImageUploadErrorResponse {
        status: "error".to_string(),
        message: "openclaw response missing reply field".to_string(),
    })
    .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"upstream bad response\"}".to_string());
    respond_json_with_status(request, 503, &payload);
}

fn handle_image_upload(
    mut request: tiny_http::Request,
    query: &str,
    image_store_path: &str,
    image_index_path: &str,
    image_db_error_store_path: &str,
    ai_predict_url: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let tag = match parse_tag(&parse_query(query)) {
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
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: "Content-Type must be multipart/form-data".to_string(),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 400, &payload);
        return;
    }

    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("failed to read request body: {err}"),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 400, &payload);
        return;
    }

    let file_part = match parse_multipart_file(&content_type, &body) {
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

    let persisted = match save_image_file(image_store_path, &tag, &file_part) {
        Ok(v) => v,
        Err(err) => {
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
    let db_result = db
        .lock()
        .map_err(|_| "db lock poisoned".to_string())
        .and_then(|mut guard| guard.insert_image_upload(&db_record));
    if let Err(err) = db_result {
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

    let infer_result =
        infer_image_from_file(ai_predict_url, &persisted.saved_path, &persisted.image_type);
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
        }
    }

    if let Err(err) = append_image_index_backup(image_index_path, &tag, &persisted) {
        eprintln!(
            "{} [cloud-http] WARN: append image index backup failed: {}",
            now_rfc3339(),
            err
        );
    }

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
            let (label, unit, threshold_low, threshold_high) = infer_field_display(field, *ty);
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

fn infer_field_display(
    field: &str,
    _field_type: FieldType,
) -> (&'static str, &'static str, Option<f64>, Option<f64>) {
    match field {
        "vwc" => ("土壤湿度", "%", Some(20.0), Some(70.0)),
        "temp_c" => ("温度", "℃", Some(0.0), Some(45.0)),
        "ec" => ("电导率", "μS/cm", Some(0.0), Some(5000.0)),
        "hum" => ("空气湿度", "%", Some(30.0), Some(85.0)),
        "voltage" => ("电压", "V", Some(0.0), Some(5.0)),
        "raw" => ("原始值", "", None, None),
        "ain0" => ("AIN0", "", None, None),
        "ain1" => ("AIN1", "", None, None),
        "ain2" => ("AIN2", "", None, None),
        "ain3" => ("AIN3", "", None, None),
        "slave_id" => ("从站ID", "", None, None),
        "protocol" => ("协议", "", None, None),
        "pin" => ("引脚", "", None, None),
        "addr" => ("地址", "", None, None),
        _ => ("字段", "", None, None),
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
    let preferred = PathBuf::from("frontend_v2_premium").join(normalized);
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
