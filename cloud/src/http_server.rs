use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::thread;
use tiny_http::{Header, Method, Response, Server};

use crate::telemetry::load_records;
use crate::time_util::now_rfc3339;

pub fn start_http_server(bind_addr: &str, telemetry_store_path: String) {
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

            // Handle API routes
            if path.starts_with("/api/") {
                handle_api(request, method, path, query, &telemetry_store_path);
                continue;
            }

            // Handle Static Files
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
                {
                    "id": "D001",
                    "location": "东区一号田",
                    "humidity": "62%",
                    "temperature": "29℃",
                    "status": "正常",
                    "color": "green"
                },
                {
                    "id": "D002",
                    "location": "西区试验田",
                    "humidity": "58%",
                    "temperature": "30℃",
                    "status": "正常",
                    "color": "green"
                },
                {
                    "id": "D003",
                    "location": "北区高产田",
                    "humidity": "71%",
                    "temperature": "28℃",
                    "status": "偏高",
                    "color": "yellow"
                }
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
