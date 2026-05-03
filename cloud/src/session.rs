use std::sync::{Arc, Mutex};

use tiny_http::{Request, Response};

use crate::db::DbManager;
use crate::image_upload::{parse_boundary, parse_multipart_file, save_image_file, ImageUploadTag};
use crate::time_util::now_rfc3339;

const ALLOWED_IMAGE_ROLES: &[&str] = &["fruit", "trunk_base", "crown"];

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_tree_barcode(request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_by_code(tree_code));

    match result {
        Ok(Some(tree)) => {
            let barcode = tree["barcode_value"].as_str().unwrap_or(tree_code);
            respond_json(request, 200, &serde_json::json!({
                "status": "ok",
                "tree_code": tree["tree_code"],
                "barcode_value": barcode
            }).to_string());
        }
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &serde_json::json!({"status":"error","message":e}).to_string()),
    }
}

pub(crate) fn handle_tree_by_barcode(request: Request, barcode_value: &str, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_by_barcode_value(barcode_value));

    match result {
        Ok(Some(tree)) => respond_json(request, 200, &serde_json::json!({
            "status": "ok",
            "tree": tree
        }).to_string()),
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &serde_json::json!({"status":"error","message":e}).to_string()),
    }
}

pub(crate) fn handle_create_session(request: Request, tree_id: &str, db: Arc<Mutex<DbManager>>) {
    let tid = tree_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.create_observation_session(tid));

    match result {
        Ok(session) => respond_json(request, 200, &serde_json::json!({
            "status": "ok",
            "session": session
        }).to_string()),
        Err(e) if e.contains("tree not found") => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &serde_json::json!({"status":"error","message":e}).to_string()),
    }
}

pub(crate) fn handle_add_session_image(
    mut request: Request,
    session_id: &str,
    query: &str,
    image_store_path: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let sid = session_id.parse().unwrap_or(0);
    let content_type = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();
    if !content_type.to_ascii_lowercase().starts_with("multipart/form-data") {
        respond_json(request, 400, r#"{"status":"error","message":"Content-Type must be multipart/form-data"}"#);
        return;
    }

    let mut body = Vec::new();
    if let Err(e) = request.as_reader().read_to_end(&mut body) {
        respond_json(request, 400, &serde_json::json!({"status":"error","message":format!("failed to read request body: {e}")}).to_string());
        return;
    }

    let query_params = crate::http_server::parse_query(query);
    let image_role = query_params
        .get("image_role")
        .cloned()
        .or_else(|| multipart_text_field(&content_type, &body, "image_role"))
        .unwrap_or_default();
    if !ALLOWED_IMAGE_ROLES.contains(&image_role.as_str()) {
        eprintln!("[ERROR] invalid image_role '{}' for session {}. Allowed: {:?}", image_role, session_id, ALLOWED_IMAGE_ROLES);
        respond_json(request, 400, &serde_json::json!({
            "status": "error",
            "message": format!("invalid image_role: '{}'", image_role),
            "allowed_roles": ALLOWED_IMAGE_ROLES
        }).to_string());
        return;
    }

    let file_part = match parse_multipart_file(&content_type, &body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[ERROR] parse_multipart_file failed for session {}: {}", session_id, e);
            respond_json(request, 400, &serde_json::json!({"status":"error","message":format!("multipart parse error: {e}")}).to_string());
            return;
        }
    };

    let session = match db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_observation_session(sid)) {
        Ok(Some(v)) => v,
        Ok(None) => {
            respond_json(request, 404, r#"{"status":"error","message":"session not found"}"#);
            return;
        }
        Err(e) => {
            respond_json(request, 500, &serde_json::json!({"status":"error","message":e}).to_string());
            return;
        }
    };

    let tree_code = session["tree_code"].as_str().unwrap_or("unknown_tree");
    let tag = ImageUploadTag {
        device_id: format!("session_{sid}"),
        ts: now_rfc3339(),
        location: tree_code.to_string(),
        crop_type: "oil_palm".to_string(),
        farm_note: format!("tree_code={tree_code};image_role={image_role}"),
    };
    let persisted = match save_image_file(image_store_path, &tag, &file_part) {
        Ok(v) => v,
        Err(e) => {
            respond_json(request, 400, &serde_json::json!({"status":"error","message":e}).to_string());
            return;
        }
    };

    let mock_analysis = mock_analysis_for_role(&image_role, tree_code);
    let metadata = serde_json::json!({
        "tree_code": tree_code,
        "session_id": sid,
        "session_code": session["session_code"],
        "filename": file_part.filename,
        "mock": true
    });

    let now = Utc::now();
    let db_record = crate::db::ImageUploadDbRecord {
        upload_id: persisted.upload_id.clone(),
        device_id: tag.device_id.clone(),
        captured_at: now,
        received_at: now,
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

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            // 首先存入通用的资产表，确保可以通过 /api/v1/image/file 下载
            g.insert_image_upload(&db_record)?;

            let image_url = format!("/api/v1/image/file?upload_id={}", persisted.upload_id);
            g.insert_session_image(
                sid,
                &image_url,
                &image_role,
                Some(&persisted.upload_id),
                mock_analysis.clone(),
                metadata,
            )
        });

    match result {
        Ok(image) => respond_json(request, 200, &serde_json::json!({
            "status": "ok",
            "image": image,
            "analysis": mock_analysis
        }).to_string()),
        Err(e) => {
            eprintln!("[ERROR] DB insert_session_image failed: {}", e);
            respond_json(request, 500, &serde_json::json!({"status":"error","message":e}).to_string());
        }
    }
}

pub(crate) fn handle_get_session_images(request: Request, session_id: &str, db: Arc<Mutex<DbManager>>) {
    let sid = session_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_session_images(sid));

    match result {
        Ok(images) => respond_json(request, 200, &serde_json::json!({
            "status": "ok",
            "images": images
        }).to_string()),
        Err(e) => respond_json(request, 500, &serde_json::json!({"status":"error","message":e}).to_string()),
    }
}

fn mock_analysis_for_role(image_role: &str, tree_code: &str) -> serde_json::Value {
    match image_role {
        "fruit" => serde_json::json!({
            "status": "success",
            "results": [{
                "task": "ffb_maturity_mock",
                "label": "ripe",
                "confidence": 0.72,
                "geometry": {"type": "bbox", "x": 0.42, "y": 0.36, "w": 0.18, "h": 0.2}
            }],
            "metadata": {
                "crop": "oil_palm",
                "tree_code": tree_code,
                "image_role": image_role,
                "advice": "mock: schedule harvest verification"
            },
            "model_version": "oil_palm_mock_session_v1"
        }),
        "trunk_base" => serde_json::json!({
            "status": "success",
            "results": [{
                "task": "ganoderma_risk_mock",
                "label": "suspected_low",
                "confidence": 0.61,
                "geometry": {"type": "bbox", "x": 0.25, "y": 0.58, "w": 0.28, "h": 0.22}
            }],
            "metadata": {
                "crop": "oil_palm",
                "tree_code": tree_code,
                "image_role": image_role,
                "risk_language": "suspected_not_confirmed",
                "advice": "mock: recheck trunk base and keep monitoring"
            },
            "model_version": "oil_palm_mock_session_v1"
        }),
        _ => serde_json::json!({
            "status": "success",
            "results": [{
                "task": "growth_vigor_mock",
                "label": "moderate_vigor",
                "confidence": 0.66,
                "geometry": {"type": "crown_region", "coverage": 0.64}
            }],
            "metadata": {
                "crop": "oil_palm",
                "tree_code": tree_code,
                "image_role": image_role,
                "vigor_index": 0.64,
                "advice": "mock: compare with next UAV or crown observation"
            },
            "model_version": "oil_palm_mock_session_v1"
        }),
    }
}

fn multipart_text_field(content_type: &str, body: &[u8], field_name: &str) -> Option<String> {
    let boundary = parse_boundary(content_type)?;
    let marker = format!("--{boundary}");
    let text = String::from_utf8_lossy(body);
    for part in text.split(&marker) {
        let Some((headers, value)) = part.split_once("\r\n\r\n") else {
            continue;
        };
        if headers.contains(&format!("name=\"{field_name}\"")) {
            return Some(value.trim_matches(&['\r', '\n', '-'][..]).trim().to_string());
        }
    }
    None
}
