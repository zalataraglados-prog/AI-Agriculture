use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde_json::Value;
use std::env;
use std::process::exit;

fn parse_base_arg() -> String {
    let mut base = "http://127.0.0.1:8088".to_string();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--base" {
            if let Some(value) = args.next() {
                base = value;
            }
        }
    }
    base
}

fn pass(msg: &str) {
    println!("[PASS] {msg}");
}

fn fail(msg: &str, errors: &mut Vec<String>) {
    println!("[FAIL] {msg}");
    errors.push(msg.to_string());
}

fn require(cond: bool, msg: &str, errors: &mut Vec<String>) {
    if cond {
        pass(msg);
    } else {
        fail(msg, errors);
    }
}

fn get_json(
    client: &Client,
    base: &str,
    path: &str,
    params: &[(&str, String)],
) -> Result<(StatusCode, Value), String> {
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let mut req = client.get(url);
    if !params.is_empty() {
        req = req.query(params);
    }
    let resp = req.send().map_err(|e| format!("request {path} failed: {e}"))?;
    let status = resp.status();
    let body = resp
        .json::<Value>()
        .map_err(|e| format!("decode json {path} failed: {e}"))?;
    Ok((status, body))
}

fn get_status(client: &Client, base: &str, path: &str) -> Result<StatusCode, String> {
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("request {path} failed: {e}"))?;
    Ok(resp.status())
}

fn main() {
    let base = parse_base_arg();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("failed to build http client");

    let mut errors: Vec<String> = Vec::new();

    // 1) schema
    match get_json(&client, &base, "/api/v1/sensor/schema", &[]) {
        Ok((status, body)) => {
            require(
                status == StatusCode::OK,
                "GET /api/v1/sensor/schema -> 200",
                &mut errors,
            );
            let has_sensors = body
                .get("sensors")
                .and_then(|v| v.as_array())
                .is_some();
            require(has_sensors, "schema has sensors[]", &mut errors);
        }
        Err(err) => fail(&err, &mut errors),
    }

    // 2) telemetry
    match get_json(
        &client,
        &base,
        "/api/v1/telemetry",
        &[("limit", "3".to_string())],
    ) {
        Ok((status, body)) => {
            require(
                status == StatusCode::OK,
                "GET /api/v1/telemetry -> 200",
                &mut errors,
            );
            require(body.is_array(), "telemetry response is list", &mut errors);
            if let Some(first) = body.as_array().and_then(|arr| arr.first()) {
                let has_fields = ["ts", "device_id", "sensor_id", "fields"]
                    .iter()
                    .all(|k| first.get(*k).is_some());
                require(
                    has_fields,
                    "telemetry row has ts/device_id/sensor_id/fields",
                    &mut errors,
                );
            }
        }
        Err(err) => fail(&err, &mut errors),
    }

    // 3) image uploads + image file
    let mut upload_id: Option<String> = None;
    match get_json(
        &client,
        &base,
        "/api/v1/image/uploads",
        &[("limit", "3".to_string())],
    ) {
        Ok((status, body)) => {
            require(
                status == StatusCode::OK,
                "GET /api/v1/image/uploads -> 200",
                &mut errors,
            );
            require(body.is_array(), "image uploads response is list", &mut errors);
            if let Some(first) = body.as_array().and_then(|arr| arr.first()) {
                for key in [
                    "upload_status",
                    "predicted_class",
                    "disease_rate",
                    "is_diseased",
                ] {
                    require(
                        first.get(key).is_some(),
                        &format!("image row has {key}"),
                        &mut errors,
                    );
                }
                upload_id = first
                    .get("upload_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
        Err(err) => fail(&err, &mut errors),
    }

    if let Some(id) = upload_id {
        let url = format!("{}/api/v1/image/file", base.trim_end_matches('/'));
        match client
            .get(url)
            .query(&[("upload_id", id)])
            .send()
            .map(|r| r.status())
        {
            Ok(status) => require(
                status == StatusCode::OK,
                "GET /api/v1/image/file by upload_id -> 200",
                &mut errors,
            ),
            Err(err) => fail(
                &format!("request /api/v1/image/file failed: {err}"),
                &mut errors,
            ),
        }
    } else {
        pass("image file check skipped (no upload_id in response)");
    }

    // 4) deprecated endpoints
    for path in ["/api/dashboard", "/api/charts", "/api/fields"] {
        match get_status(&client, &base, path) {
            Ok(status) => require(
                status == StatusCode::GONE,
                &format!("{path} returns 410"),
                &mut errors,
            ),
            Err(err) => fail(&err, &mut errors),
        }
    }

    if errors.is_empty() {
        println!("\nWave-0 verification passed.");
        exit(0);
    }

    println!("\nWave-0 verification failed:");
    for (idx, msg) in errors.iter().enumerate() {
        println!("{}. {}", idx + 1, msg);
    }
    exit(1);
}
