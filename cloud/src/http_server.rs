use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::{DateTime, Utc};
use tiny_http::{Header, Method, Response, Server};

use crate::db::{DbManager, ImageInferenceDbRecord, ImageUploadDbRecord, ImageUploadQueryFilter};
use crate::image_upload::{
    append_image_error_backup, append_image_index_backup, build_upload_ok_response,
    parse_captured_at_utc, parse_multipart_file, parse_tag, save_image_file,
    ImageUploadErrorResponse, ImageUploadOkResponse,
};
use crate::telemetry::load_records;
use crate::time_util::now_rfc3339;

pub fn start_http_server(
    bind_addr: &str,
    telemetry_store_path: String,
    image_store_path: String,
    image_index_path: String,
    image_db_error_store_path: String,
    db: Arc<Mutex<DbManager>>,
) {
    let server = Server::http(bind_addr).expect("Failed to start HTTP server");
    println!(
        "{} [cloud-http] Listening on http://{}",
        now_rfc3339(),
        bind_addr
    );

    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let url = request.url().to_string();
            let method = request.method().clone();
            let (path, query) = split_query(&url);

            if path.starts_with("/api/") {
                handle_api(
                    request,
                    method,
                    path,
                    query,
                    &telemetry_store_path,
                    &image_store_path,
                    &image_index_path,
                    &image_db_error_store_path,
                    db.clone(),
                );
                continue;
            }

            let mut file_path = path.to_string();
            if file_path == "/" {
                file_path = "/index.html".to_string();
            }

            let path = PathBuf::from(format!("dashboard{}", file_path));
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
    telemetry_store_path: &str,
    image_store_path: &str,
    image_index_path: &str,
    image_db_error_store_path: &str,
    db: Arc<Mutex<DbManager>>,
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
                db,
            );
        }
        (Method::Get, "/api/v1/image/uploads") => {
            handle_image_upload_query(request, query, db);
        }
        (Method::Post, "/api/send-code") => {
            respond_json(
                r#"{"success": true, "message": "验证码发送成功", "data": null}"#,
                request,
            );
        }
        (Method::Post, "/api/login") => {
            respond_json(
                r#"{
                "success": true, 
                "message": "登录成功", 
                "data": {
                    "token": "jwt_token_mock",
                    "userInfo": {"id": 1, "role": "admin"}
                }
            }"#,
                request,
            );
        }
        (Method::Get, "/api/dashboard") => {
            respond_json(
                r#"{"totalFields": "68", "avgHumidity": "65%", "todayTemp": "30℃", "deviceOnline": "98%"}"#,
                request,
            );
        }
        (Method::Get, "/api/charts") => {
            respond_json(
                r#"{"humidityData": [58, 61, 63, 65, 64, 66, 65], "typesData": [35, 25, 20, 20]}"#,
                request,
            );
        }
        (Method::Get, "/api/fields") => {
            let fields_json = r#"[
                {"id":"D001","location":"东区一号田","humidity":"62%","temperature":"29℃","status":"正常","color":"green"},
                {"id":"D002","location":"西区试验田","humidity":"58%","temperature":"30℃","status":"正常","color":"green"},
                {"id":"D003","location":"北区高产田","humidity":"71%","temperature":"28℃","status":"偏高","color":"yellow"}
            ]"#;
            respond_json(fields_json, request);
        }
        (Method::Get, "/api/telemetry") => {
            let params = parse_query(query);
            let device_filter = params.get("device_id").map(|v| v.as_str()).unwrap_or("");
            let sensor_filter = params.get("sensor_id").map(|v| v.as_str()).unwrap_or("");
            let limit = params
                .get("limit")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(100)
                .clamp(1, 1000);

            let mut records = load_records(telemetry_store_path).unwrap_or_default();
            records.retain(|record| {
                let device_ok = device_filter.is_empty() || record.device_id == device_filter;
                let sensor_ok = sensor_filter.is_empty() || record.sensor_id == sensor_filter;
                device_ok && sensor_ok
            });

            if records.len() > limit {
                records = records.split_off(records.len() - limit);
            }

            let body = serde_json::to_string(&records).unwrap_or_else(|_| "[]".to_string());
            respond_json(&body, request);
        }
        _ => {
            let _ = request.respond(Response::from_string("API Not Found").with_status_code(404));
        }
    }
}

fn handle_image_upload(
    mut request: tiny_http::Request,
    query: &str,
    image_store_path: &str,
    image_index_path: &str,
    image_db_error_store_path: &str,
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
            respond_json_with_status(request, 200, &payload);
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
        respond_json_with_status(request, 200, &payload);
        return;
    }

    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        let payload = serde_json::to_string(&ImageUploadErrorResponse {
            status: "error".to_string(),
            message: format!("failed to read request body: {err}"),
        })
        .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
        respond_json_with_status(request, 200, &payload);
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
            respond_json_with_status(request, 200, &payload);
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
            respond_json_with_status(request, 200, &payload);
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
            respond_json_with_status(request, 200, &payload);
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
        respond_json_with_status(request, 200, &payload);
        return;
    }

    if let Some(inference_record) = build_optional_inference_record(&persisted.upload_id, query) {
        let inference_result = db
            .lock()
            .map_err(|_| "db lock poisoned".to_string())
            .and_then(|mut guard| guard.insert_image_inference(&inference_record));
        if let Err(err) = inference_result {
            eprintln!(
                "{} [cloud-http] WARN: insert image inference failed: {}",
                now_rfc3339(),
                err
            );
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

fn handle_image_upload_query(request: tiny_http::Request, query: &str, db: Arc<Mutex<DbManager>>) {
    let params = parse_query(query);
    let start_time = match parse_optional_rfc3339(params.get("start_time").map(|v| v.as_str())) {
        Ok(v) => v,
        Err(err) => {
            let payload = serde_json::to_string(&ImageUploadErrorResponse {
                status: "error".to_string(),
                message: err,
            })
            .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"bad request\"}".to_string());
            respond_json_with_status(request, 200, &payload);
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
            respond_json_with_status(request, 200, &payload);
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
            .unwrap_or(100),
    };

    let rows = db
        .lock()
        .map_err(|_| "db lock poisoned".to_string())
        .and_then(|mut guard| guard.query_image_uploads(&filter));
    match rows {
        Ok(items) => {
            let payload = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
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
            respond_json_with_status(request, 200, &payload);
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

fn build_optional_inference_record(upload_id: &str, query: &str) -> Option<ImageInferenceDbRecord> {
    let params = parse_query(query);
    let predicted_class = non_empty(params.get("predicted_class").cloned());
    if predicted_class.is_none() {
        return None;
    }

    let confidence = params.get("confidence").and_then(|v| v.parse::<f64>().ok());
    let model_version = non_empty(params.get("model_version").cloned());
    let latency_ms = params.get("latency_ms").and_then(|v| v.parse::<i32>().ok());
    let advice_code = non_empty(params.get("advice_code").cloned());

    let topk_json = params
        .get("topk_json")
        .and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok())
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    let metadata_json = params
        .get("metadata_json")
        .and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let geometry_json = params
        .get("geometry_json")
        .and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok());

    Some(ImageInferenceDbRecord {
        upload_id: upload_id.to_string(),
        predicted_class,
        confidence,
        model_version,
        topk_json,
        metadata_json,
        geometry_json,
        latency_ms,
        advice_code,
    })
}

fn split_query(url: &str) -> (&str, &str) {
    match url.split_once('?') {
        Some((path, query)) => (path, query),
        None => (url, ""),
    }
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
        out.insert(key.to_string(), value.to_string());
    }
    out
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
