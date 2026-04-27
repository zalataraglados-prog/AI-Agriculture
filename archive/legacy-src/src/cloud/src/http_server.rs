use std::fs::File;
use std::path::PathBuf;
use std::thread;
use tiny_http::{Header, Method, Response, Server};

pub fn start_http_server(bind_addr: &str) {
    let server = Server::http(bind_addr.clone()).expect("Failed to start HTTP server");
    println!("[cloud-http] Listening on http://{}", bind_addr);

    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let url = request.url().to_string();
            let method = request.method().clone();

            // Handle API routes
            if url.starts_with("/api/") {
                handle_api(request, method, &url);
                continue;
            }

            // Handle Static Files
            let mut file_path = url;
            if file_path == "/" {
                file_path = "/index.html".to_string();
            }

            // Strip query string if any
            if let Some(pos) = file_path.find('?') {
                file_path.truncate(pos);
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

                let header = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap();
                match File::open(path) {
                    Ok(f) => {
                        let response = Response::from_file(f).with_header(header);
                        let _ = request.respond(response);
                    }
                    Err(_) => {
                        let _ = request.respond(Response::from_string("File Error").with_status_code(500));
                    }
                }
            } else {
                let _ = request.respond(Response::from_string("Not Found").with_status_code(404));
            }
        }
    });
}

fn handle_api(request: tiny_http::Request, method: Method, url: &str) {
    let respond_json = move |json: &str, req: tiny_http::Request| {
        let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap();
        let _ = req.respond(Response::from_string(json).with_header(header));
    };

    match (method, url) {
        (Method::Post, "/api/send-code") => {
            respond_json(r#"{"success": true, "message": "验证码发送成功", "data": null}"#, request);
        }
        (Method::Post, "/api/login") => {
            respond_json(r#"{
                "success": true, 
                "message": "登录成功", 
                "data": {
                    "token": "jwt_token_mock",
                    "userInfo": {"id": 1, "role": "admin"}
                }
            }"#, request);
        }
        (Method::Get, "/api/dashboard") => {
            respond_json(r#"{"totalFields": "68", "avgHumidity": "65%", "todayTemp": "30℃", "deviceOnline": "98%"}"#, request);
        }
        (Method::Get, "/api/charts") => {
            respond_json(r#"{"humidityData": [58, 61, 63, 65, 64, 66, 65], "typesData": [35, 25, 20, 20]}"#, request);
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
        _ => {
            let _ = request.respond(Response::from_string("API Not Found").with_status_code(404));
        }
    }
}
