use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use tiny_http::{Request, Response};

use crate::ai_client::detect_oil_palm_a0_from_bytes;
use crate::db::DbManager;
use crate::image_upload::{parse_boundary, parse_multipart_file, save_image_file, ImageUploadTag};
use crate::time_util::now_rfc3339;
use sha2::{Digest, Sha256};

const ALLOWED_IMAGE_ROLES: &[&str] = &["fruit", "trunk_base", "crown"];
const A0_MASK_SOURCE: &str = "a0_user_confirmation_v1";

#[derive(Debug, serde::Deserialize)]
struct ConfirmSelectionRequest {
    selected_candidate_ids: Vec<String>,
}

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
    let _ = request.respond(response);
}

pub(crate) fn handle_tree_barcode(request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let result = db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_by_code(tree_code));

    match result {
        Ok(Some(tree)) => {
            let barcode = tree["barcode_value"].as_str().unwrap_or(tree_code);
            respond_json(
                request,
                200,
                &serde_json::json!({
                    "status": "ok",
                    "tree_code": tree["tree_code"],
                    "barcode_value": barcode
                })
                .to_string(),
            );
        }
        Ok(None) => respond_json(
            request,
            404,
            r#"{"status":"error","message":"tree not found"}"#,
        ),
        Err(e) => respond_json(
            request,
            500,
            &serde_json::json!({"status":"error","message":e}).to_string(),
        ),
    }
}

pub(crate) fn handle_tree_by_barcode(
    request: Request,
    barcode_value: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let result = db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_by_barcode_value(barcode_value));

    match result {
        Ok(Some(tree)) => respond_json(
            request,
            200,
            &serde_json::json!({
                "status": "ok",
                "tree": tree
            })
            .to_string(),
        ),
        Ok(None) => respond_json(
            request,
            404,
            r#"{"status":"error","message":"tree not found"}"#,
        ),
        Err(e) => respond_json(
            request,
            500,
            &serde_json::json!({"status":"error","message":e}).to_string(),
        ),
    }
}

pub(crate) fn handle_create_session(request: Request, tree_id: &str, db: Arc<Mutex<DbManager>>) {
    let tid = tree_id.parse().unwrap_or(0);
    let result = db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.create_observation_session(tid));

    match result {
        Ok(session) => respond_json(
            request,
            200,
            &serde_json::json!({
                "status": "ok",
                "session": session
            })
            .to_string(),
        ),
        Err(e) if e.contains("tree not found") => respond_json(
            request,
            404,
            r#"{"status":"error","message":"tree not found"}"#,
        ),
        Err(e) => respond_json(
            request,
            500,
            &serde_json::json!({"status":"error","message":e}).to_string(),
        ),
    }
}

pub(crate) fn handle_add_session_image(
    mut request: Request,
    session_id: &str,
    query: &str,
    image_store_path: &str,
    ai_oil_palm_a0_detect_url: Option<&str>,
    ai_http_client: &Client,
    db: Arc<Mutex<DbManager>>,
) {
    let sid = session_id.parse().unwrap_or(0);
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
        respond_json(
            request,
            400,
            r#"{"status":"error","message":"Content-Type must be multipart/form-data"}"#,
        );
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
    let mock_detect_role = query_params
        .get("mock_detect_role")
        .cloned()
        .or_else(|| multipart_text_field(&content_type, &body, "mock_detect_role"));
    if !ALLOWED_IMAGE_ROLES.contains(&image_role.as_str()) {
        eprintln!(
            "[ERROR] invalid image_role '{}' for session {}. Allowed: {:?}",
            image_role, session_id, ALLOWED_IMAGE_ROLES
        );
        respond_json(
            request,
            400,
            &serde_json::json!({
                "status": "error",
                "message": format!("invalid image_role: '{}'", image_role),
                "allowed_roles": ALLOWED_IMAGE_ROLES
            })
            .to_string(),
        );
        return;
    }

    let file_part = match parse_multipart_file(&content_type, &body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[ERROR] parse_multipart_file failed for session {}: {}",
                session_id, e
            );
            respond_json(request, 400, &serde_json::json!({"status":"error","message":format!("multipart parse error: {e}")}).to_string());
            return;
        }
    };

    let session = match db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_observation_session(sid))
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            respond_json(
                request,
                404,
                r#"{"status":"error","message":"session not found"}"#,
            );
            return;
        }
        Err(e) => {
            respond_json(
                request,
                500,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
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
            respond_json(
                request,
                400,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
            return;
        }
    };

    let a0_review = detect_a0_or_mock(
        ai_http_client,
        ai_oil_palm_a0_detect_url,
        &file_part.body,
        file_part.filename.as_deref(),
        &persisted.image_type,
        &image_role,
        tree_code,
        sid,
        mock_detect_role.as_deref(),
    );
    let a0_candidates = a0_review["metadata"]["a0_candidates"].clone();
    let mut metadata = a0_review["metadata"].clone();
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert("tree_code".to_string(), serde_json::json!(tree_code));
        obj.insert("session_id".to_string(), serde_json::json!(sid));
        obj.insert("session_code".to_string(), session["session_code"].clone());
        obj.insert("filename".to_string(), serde_json::json!(file_part.filename));
        obj.insert("a0_candidates".to_string(), a0_candidates);
        obj.insert(
            "route_status".to_string(),
            a0_review["metadata"]["route_status"].clone(),
        );
        obj.insert(
            "downstream_status".to_string(),
            serde_json::json!("pending_user_confirmation"),
        );
    }

    let now = chrono::Utc::now();
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

    let result = db
        .lock()
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
                a0_review.clone(),
                metadata,
            )
        });

    match result {
        Ok(image) => respond_json(
            request,
            200,
            &serde_json::json!({
                "status": "ok",
                "requires_confirmation": a0_review["metadata"]["requires_user_confirmation"].as_bool().unwrap_or(false),
                "image": image,
                "analysis": a0_review
            })
            .to_string(),
        ),
        Err(e) => {
            eprintln!("[ERROR] DB insert_session_image failed: {}", e);
            respond_json(
                request,
                500,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
        }
    }
}

pub(crate) fn handle_get_session_images(
    request: Request,
    session_id: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let sid = session_id.parse().unwrap_or(0);
    let result = db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_session_images(sid));

    match result {
        Ok(images) => respond_json(
            request,
            200,
            &serde_json::json!({
                "status": "ok",
                "images": images
            })
            .to_string(),
        ),
        Err(e) => respond_json(
            request,
            500,
            &serde_json::json!({"status":"error","message":e}).to_string(),
        ),
    }
}

pub(crate) fn handle_confirm_session_image(
    mut request: Request,
    session_id: &str,
    image_id: &str,
    image_store_path: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let sid = session_id.parse().unwrap_or(0);
    let iid = image_id.parse().unwrap_or(0);
    let mut body = Vec::new();
    if let Err(e) = request.as_reader().read_to_end(&mut body) {
        respond_json(request, 400, &serde_json::json!({"status":"error","message":format!("failed to read request body: {e}")}).to_string());
        return;
    }
    let parsed: ConfirmSelectionRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            respond_json(request, 400, &serde_json::json!({"status":"error","message":format!("invalid confirmation JSON: {e}")}).to_string());
            return;
        }
    };
    if parsed.selected_candidate_ids.is_empty() {
        respond_json(
            request,
            400,
            r#"{"status":"error","message":"selected_candidate_ids must not be empty"}"#,
        );
        return;
    }

    let (session, session_image) = match db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let session = g
                .get_observation_session(sid)?
                .ok_or_else(|| "session not found".to_string())?;
            let image = g
                .get_session_image(sid, iid)?
                .ok_or_else(|| "session image not found".to_string())?;
            Ok((session, image))
        }) {
        Ok(v) => v,
        Err(e) if e.contains("not found") => {
            respond_json(
                request,
                404,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
            return;
        }
        Err(e) => {
            respond_json(
                request,
                500,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
            return;
        }
    };

    let image_role = session_image["image_role"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let tree_code = session["tree_code"]
        .as_str()
        .unwrap_or("unknown_tree")
        .to_string();
    let upload_id = match session_image["upload_id"].as_str() {
        Some(v) => v.to_string(),
        None => {
            respond_json(
                request,
                400,
                r#"{"status":"error","message":"session image has no upload_id"}"#,
            );
            return;
        }
    };
    let metadata = session_image["metadata"].clone();
    let route_status = metadata["route_status"].as_str().unwrap_or("");
    if route_status != "needs_user_confirmation" {
        respond_json(request, 400, &serde_json::json!({
            "status": "error",
            "message": format!("session image is not confirmable in route_status '{}'", route_status)
        }).to_string());
        return;
    }
    let candidates = metadata["a0_candidates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let known_ids: Vec<String> = candidates
        .iter()
        .filter_map(|item| item["candidate_id"].as_str().map(|v| v.to_string()))
        .collect();
    let selected_ids: Vec<String> = parsed
        .selected_candidate_ids
        .into_iter()
        .filter(|id| known_ids.iter().any(|known| known == id))
        .collect();
    if selected_ids.is_empty() {
        respond_json(
            request,
            400,
            r#"{"status":"error","message":"no selected candidate ids match A0 candidates"}"#,
        );
        return;
    }
    let rejected_ids: Vec<String> = known_ids
        .iter()
        .filter(|id| !selected_ids.iter().any(|selected| selected == *id))
        .cloned()
        .collect();

    let source_path = match db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_saved_path_by_upload_id(&upload_id))
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            respond_json(
                request,
                404,
                r#"{"status":"error","message":"source upload file not found"}"#,
            );
            return;
        }
        Err(e) => {
            respond_json(
                request,
                500,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
            return;
        }
    };

    let masked = match create_masked_image(
        image_store_path,
        &source_path,
        &upload_id,
        &candidates,
        &rejected_ids,
    ) {
        Ok(v) => v,
        Err(e) => {
            respond_json(
                request,
                500,
                &serde_json::json!({"status":"error","message":e}).to_string(),
            );
            return;
        }
    };

    let now = chrono::Utc::now();
    let tag = ImageUploadTag {
        device_id: format!("session_{sid}"),
        ts: now.to_rfc3339(),
        location: tree_code.clone(),
        crop_type: "oil_palm".to_string(),
        farm_note: format!("tree_code={tree_code};image_role={image_role};mask_source={A0_MASK_SOURCE};source_upload_id={upload_id}"),
    };
    let db_record = crate::db::ImageUploadDbRecord {
        upload_id: masked.upload_id.clone(),
        device_id: tag.device_id,
        captured_at: now,
        received_at: now,
        location: tag.location,
        crop_type: tag.crop_type,
        farm_note: tag.farm_note,
        saved_path: masked.saved_path.clone(),
        sha256: masked.sha256.clone(),
        image_type: "png".to_string(),
        file_size: masked.file_size as i64,
        upload_status: "stored".to_string(),
        error_message: None,
    };

    let downstream = mock_analysis_for_role(
        &image_role,
        &tree_code,
        &upload_id,
        &masked.upload_id,
        &selected_ids,
    );
    let updated_metadata = merge_confirmation_metadata(
        metadata,
        &selected_ids,
        &rejected_ids,
        &masked.upload_id,
        &upload_id,
    );

    let result = db
        .lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            g.insert_image_upload(&db_record)?;
            g.update_session_image_analysis(sid, iid, downstream.clone(), updated_metadata.clone())
        });

    match result {
        Ok(image) => respond_json(
            request,
            200,
            &serde_json::json!({
                "status": "ok",
                "image": image,
                "analysis": downstream,
                "masked_upload_id": masked.upload_id,
                "masked_image_url": format!("/api/v1/image/file?upload_id={}", masked.upload_id),
                "selected_candidate_ids": selected_ids,
                "rejected_candidate_ids": rejected_ids
            })
            .to_string(),
        ),
        Err(e) => respond_json(
            request,
            500,
            &serde_json::json!({"status":"error","message":e}).to_string(),
        ),
    }
}

fn mock_analysis_for_role(
    image_role: &str,
    tree_code: &str,
    source_upload_id: &str,
    masked_upload_id: &str,
    selected_candidate_ids: &[String],
) -> serde_json::Value {
    let shared_metadata = serde_json::json!({
        "crop": "oil_palm",
        "tree_code": tree_code,
        "image_role": image_role,
        "source_upload_id": source_upload_id,
        "masked_upload_id": masked_upload_id,
        "selected_candidate_ids": selected_candidate_ids,
        "mask_source": A0_MASK_SOURCE
    });
    match image_role {
        "fruit" => serde_json::json!({
            "status": "success",
            "results": [{
                "task": "ffb_maturity_mock",
                "label": "ripe",
                "confidence": 0.72,
                "geometry": {"type": "bbox", "x": 0.42, "y": 0.36, "w": 0.18, "h": 0.2}
            }],
            "metadata": merge_json(shared_metadata.clone(), serde_json::json!({
                "advice": "mock: schedule harvest verification"
            })),
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
            "metadata": merge_json(shared_metadata.clone(), serde_json::json!({
                "risk_language": "suspected_not_confirmed",
                "advice": "mock: recheck trunk base and keep monitoring"
            })),
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
            "metadata": merge_json(shared_metadata, serde_json::json!({
                "vigor_index": 0.64,
                "advice": "mock: compare with next UAV or crown observation"
            })),
            "model_version": "oil_palm_mock_session_v1"
        }),
    }
}

fn detect_a0_or_mock(
    client: &Client,
    detect_url: Option<&str>,
    image_bytes: &[u8],
    filename: Option<&str>,
    image_type: &str,
    image_role: &str,
    tree_code: &str,
    session_id: i32,
    mock_detect_role: Option<&str>,
) -> serde_json::Value {
    let Some(url) = detect_url else {
        let mut review = a0_review_for_role(image_role, tree_code, mock_detect_role);
        annotate_a0_metadata(
            &mut review,
            tree_code,
            session_id,
            "local_mock_no_ai_url",
            None,
            None,
        );
        return review;
    };

    match detect_oil_palm_a0_from_bytes(
        client,
        url,
        image_bytes,
        filename,
        image_type,
        image_role,
        Some(tree_code),
        Some(&session_id.to_string()),
        mock_detect_role,
    ) {
        Ok(mut review) => {
            annotate_a0_metadata(
                &mut review,
                tree_code,
                session_id,
                "ai_engine",
                Some(url),
                None,
            );
            review
        }
        Err(e) => {
            eprintln!(
                "[WARN] A0 AI Engine call failed for tree {} session {}: {}. Falling back to local mock A0.",
                tree_code, session_id, e
            );
            let mut review = a0_review_for_role(image_role, tree_code, mock_detect_role);
            annotate_a0_metadata(
                &mut review,
                tree_code,
                session_id,
                "local_mock_ai_error",
                Some(url),
                Some(&e),
            );
            review
        }
    }
}

fn annotate_a0_metadata(
    review: &mut serde_json::Value,
    tree_code: &str,
    session_id: i32,
    source: &str,
    detect_url: Option<&str>,
    error: Option<&str>,
) {
    if !review.is_object() {
        *review = serde_json::json!({
            "status": "success",
            "results": [],
            "geometry": [],
            "metadata": {},
            "model_version": "oil_palm_a0_detector_mock_v1"
        });
    }
    if review.get("metadata").and_then(|v| v.as_object()).is_none() {
        review["metadata"] = serde_json::json!({});
    }
    if let Some(obj) = review["metadata"].as_object_mut() {
        obj.insert("crop".to_string(), serde_json::json!("oil_palm"));
        obj.insert("tree_code".to_string(), serde_json::json!(tree_code));
        obj.insert("session_id".to_string(), serde_json::json!(session_id));
        obj.insert(
            "downstream_status".to_string(),
            serde_json::json!("pending_user_confirmation"),
        );
        obj.insert("a0_source".to_string(), serde_json::json!(source));
        if let Some(url) = detect_url {
            obj.insert("a0_detect_url".to_string(), serde_json::json!(url));
        }
        if let Some(message) = error {
            obj.insert("a0_runtime_fallback".to_string(), serde_json::json!(true));
            obj.insert("a0_runtime_error".to_string(), serde_json::json!(message));
        }
        let requires_confirmation = obj
            .get("route_status")
            .and_then(|v| v.as_str())
            .map(|v| v == "needs_user_confirmation")
            .unwrap_or(false);
        obj.insert(
            "requires_user_confirmation".to_string(),
            serde_json::json!(requires_confirmation),
        );
    }
}

fn a0_review_for_role(
    image_role: &str,
    tree_code: &str,
    mock_detect_role: Option<&str>,
) -> serde_json::Value {
    let detected_role = mock_detect_role.unwrap_or(image_role);
    let candidates = a0_candidates_for_role(detected_role);
    let requested_label = a0_label_for_role(image_role);
    let detected_labels: Vec<String> = candidates
        .iter()
        .filter_map(|candidate| candidate["label"].as_str().map(|v| v.to_string()))
        .collect();
    let route_status = if candidates.is_empty() {
        "no_supported_structure_detected"
    } else if requested_label
        .map(|label| !detected_labels.iter().any(|detected| detected == label))
        .unwrap_or(true)
    {
        "role_mismatch"
    } else {
        "needs_user_confirmation"
    };
    let results: Vec<serde_json::Value> = candidates
        .iter()
        .map(|candidate| {
            serde_json::json!({
                "task": "a0_structure_detection",
                "label": candidate["label"],
                "confidence": candidate["confidence"],
                "geometry": candidate["geometry"],
                "metadata": {
                    "candidate_id": candidate["candidate_id"],
                    "suggested_role": candidate["suggested_role"],
                    "selected_by_default": true
                }
            })
        })
        .collect();
    let geometry: Vec<serde_json::Value> = candidates
        .iter()
        .map(|candidate| candidate["geometry"].clone())
        .collect();
    serde_json::json!({
        "status": "success",
        "results": results,
        "geometry": geometry,
        "metadata": {
            "crop": "oil_palm",
            "tree_code": tree_code,
            "image_role": image_role,
            "requested_image_role": image_role,
            "detected_image_role": detected_role,
            "route_status": route_status,
            "requires_user_confirmation": route_status == "needs_user_confirmation",
            "a0_candidates": candidates,
            "downstream_status": "pending_user_confirmation",
            "mock": true
        },
        "model_version": "oil_palm_a0_detector_mock_v1"
    })
}

fn a0_candidates_for_role(image_role: &str) -> Vec<serde_json::Value> {
    match image_role {
        "fruit" => vec![
            serde_json::json!({"candidate_id":"a0_fruit_001","label":"fruit_bunch","suggested_role":"fruit","confidence":0.91,"geometry":{"type":"bbox","x":0.18,"y":0.34,"w":0.18,"h":0.20}}),
            serde_json::json!({"candidate_id":"a0_fruit_002","label":"fruit_bunch","suggested_role":"fruit","confidence":0.86,"geometry":{"type":"bbox","x":0.52,"y":0.28,"w":0.20,"h":0.24}}),
            serde_json::json!({"candidate_id":"a0_fruit_003","label":"fruit_bunch","suggested_role":"fruit","confidence":0.78,"geometry":{"type":"bbox","x":0.68,"y":0.56,"w":0.16,"h":0.18}}),
        ],
        "trunk_base" => vec![
            serde_json::json!({"candidate_id":"a0_trunk_base_001","label":"trunk_base","suggested_role":"trunk_base","confidence":0.88,"geometry":{"type":"bbox","x":0.38,"y":0.52,"w":0.24,"h":0.34}}),
        ],
        "crown" => vec![
            serde_json::json!({"candidate_id":"a0_crown_001","label":"crown_region","suggested_role":"crown","confidence":0.82,"geometry":{"type":"bbox","x":0.18,"y":0.10,"w":0.64,"h":0.48}}),
        ],
        _ => Vec::new(),
    }
}

fn a0_label_for_role(image_role: &str) -> Option<&'static str> {
    match image_role {
        "fruit" => Some("fruit_bunch"),
        "trunk_base" => Some("trunk_base"),
        "crown" => Some("crown_region"),
        _ => None,
    }
}

struct MaskedImage {
    upload_id: String,
    saved_path: String,
    file_size: usize,
    sha256: String,
}

fn create_masked_image(
    image_store_path: &str,
    source_path: &str,
    source_upload_id: &str,
    candidates: &[serde_json::Value],
    rejected_ids: &[String],
) -> Result<MaskedImage, String> {
    let bytes = fs::read(source_path)
        .map_err(|e| format!("failed to read source image {source_path}: {e}"))?;
    let mut img = image::load_from_memory(&bytes)
        .map_err(|e| format!("failed to decode source image for masking: {e}"))?
        .to_rgba8();
    let width = img.width();
    let height = img.height();

    for candidate in candidates {
        let Some(candidate_id) = candidate["candidate_id"].as_str() else {
            continue;
        };
        if !rejected_ids.iter().any(|id| id == candidate_id) {
            continue;
        }
        if let Some((x0, y0, x1, y1)) = bbox_pixels(&candidate["geometry"], width, height, 0.10) {
            let fill = average_color(&img, x0, y0, x1, y1);
            for y in y0..y1 {
                for x in x0..x1 {
                    img.put_pixel(x, y, fill);
                }
            }
        }
    }

    let upload_id = format!(
        "mask_{}_{}",
        source_upload_id,
        chrono::Utc::now().timestamp_millis()
    );
    let relative_path = format!("derived/{source_upload_id}/{upload_id}.png");
    let saved_path = Path::new(image_store_path)
        .join(relative_path)
        .to_string_lossy()
        .to_string();
    if let Some(parent) = Path::new(&saved_path).parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create derived image directory {}: {e}",
                parent.display()
            )
        })?;
    }
    img.save(&saved_path)
        .map_err(|e| format!("failed to save masked image {saved_path}: {e}"))?;
    let out_bytes = fs::read(&saved_path)
        .map_err(|e| format!("failed to read masked image {saved_path}: {e}"))?;
    let mut sha = Sha256::new();
    sha.update(&out_bytes);
    Ok(MaskedImage {
        upload_id,
        saved_path,
        file_size: out_bytes.len(),
        sha256: format!("{:x}", sha.finalize()),
    })
}

fn bbox_pixels(
    geometry: &serde_json::Value,
    width: u32,
    height: u32,
    padding_ratio: f64,
) -> Option<(u32, u32, u32, u32)> {
    if geometry["type"].as_str()? != "bbox" {
        return None;
    }
    let x = geometry["x"].as_f64()?.clamp(0.0, 1.0);
    let y = geometry["y"].as_f64()?.clamp(0.0, 1.0);
    let w = geometry["w"].as_f64()?.clamp(0.0, 1.0);
    let h = geometry["h"].as_f64()?.clamp(0.0, 1.0);
    let pad_x = w * padding_ratio;
    let pad_y = h * padding_ratio;
    let x0 = ((x - pad_x).max(0.0) * width as f64).floor() as u32;
    let y0 = ((y - pad_y).max(0.0) * height as f64).floor() as u32;
    let x1 = ((x + w + pad_x).min(1.0) * width as f64).ceil() as u32;
    let y1 = ((y + h + pad_y).min(1.0) * height as f64).ceil() as u32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0, y0, x1.min(width), y1.min(height)))
}

fn average_color(img: &image::RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) -> image::Rgba<u8> {
    let mut total = [0_u64; 3];
    let mut count = 0_u64;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = img.get_pixel(x, y);
            total[0] += p[0] as u64;
            total[1] += p[1] as u64;
            total[2] += p[2] as u64;
            count += 1;
        }
    }
    if count == 0 {
        return image::Rgba([64, 64, 64, 255]);
    }
    image::Rgba([
        (total[0] / count) as u8,
        (total[1] / count) as u8,
        (total[2] / count) as u8,
        255,
    ])
}

fn merge_confirmation_metadata(
    mut metadata: serde_json::Value,
    selected_ids: &[String],
    rejected_ids: &[String],
    masked_upload_id: &str,
    source_upload_id: &str,
) -> serde_json::Value {
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert(
            "selected_candidate_ids".to_string(),
            serde_json::json!(selected_ids),
        );
        obj.insert(
            "rejected_candidate_ids".to_string(),
            serde_json::json!(rejected_ids),
        );
        obj.insert("route_status".to_string(), serde_json::json!("confirmed"));
        obj.insert(
            "downstream_status".to_string(),
            serde_json::json!("mock_analysis_complete"),
        );
        obj.insert(
            "source_upload_id".to_string(),
            serde_json::json!(source_upload_id),
        );
        obj.insert(
            "masked_upload_id".to_string(),
            serde_json::json!(masked_upload_id),
        );
        obj.insert(
            "masked_image_url".to_string(),
            serde_json::json!(format!("/api/v1/image/file?upload_id={masked_upload_id}")),
        );
        obj.insert("mask_source".to_string(), serde_json::json!(A0_MASK_SOURCE));
    }
    metadata
}

fn merge_json(mut left: serde_json::Value, right: serde_json::Value) -> serde_json::Value {
    if let (Some(left_obj), Some(right_obj)) = (left.as_object_mut(), right.as_object()) {
        for (key, value) in right_obj {
            left_obj.insert(key.clone(), value.clone());
        }
    }
    left
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
            return Some(
                value
                    .trim_matches(&['\r', '\n', '-'][..])
                    .trim()
                    .to_string(),
            );
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn a0_review_uses_yolo_structure_labels() {
        let review = super::a0_review_for_role("fruit", "OP-000001", None);
        assert_eq!(
            review["metadata"]["route_status"].as_str(),
            Some("needs_user_confirmation")
        );
        let candidates = review["metadata"]["a0_candidates"].as_array().unwrap();
        assert!(candidates.len() >= 2);
        assert!(candidates
            .iter()
            .all(|item| item["label"].as_str() == Some("fruit_bunch")));
        assert!(candidates
            .iter()
            .all(|item| item["label"].as_str() != Some("unknown")));
    }

    #[test]
    fn a0_review_reports_role_mismatch() {
        let review = super::a0_review_for_role("fruit", "OP-000001", Some("trunk_base"));
        assert_eq!(
            review["metadata"]["route_status"].as_str(),
            Some("role_mismatch")
        );
        assert_eq!(review["results"][0]["label"].as_str(), Some("trunk_base"));
    }

    #[test]
    fn bbox_pixels_expands_and_clamps_normalized_box() {
        let geometry = serde_json::json!({"type":"bbox","x":0.9,"y":0.9,"w":0.2,"h":0.2});
        let (x0, y0, x1, y1) = super::bbox_pixels(&geometry, 100, 80, 0.10).unwrap();
        assert!(x0 < x1);
        assert!(y0 < y1);
        assert!(x1 <= 100);
        assert!(y1 <= 80);
    }
}
