use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::db::DbManager;

pub(crate) fn handle_missions_post(mut request: Request, _db: Arc<Mutex<DbManager>>) {
    // TODO: Parse body and insert into DB
    let response = Response::from_string(r#"{"status":"ok", "mission_id": 1}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_orthomosaic_post(mut request: Request, _mission_id: &str, _db: Arc<Mutex<DbManager>>) {
    // TODO: Parse body and insert into DB
    let response = Response::from_string(r#"{"status":"ok", "orthomosaic_id": 1}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_tiles_post(mut request: Request, _ortho_id: &str, _db: Arc<Mutex<DbManager>>) {
    let response = Response::from_string(r#"{"status":"ok", "tile_id": 1}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_mock_detections(mut request: Request, _ortho_id: &str, _db: Arc<Mutex<DbManager>>) {
    let response = Response::from_string(r#"{"status":"ok", "detections_created": 3}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_get_detections(request: Request, _ortho_id: &str, _db: Arc<Mutex<DbManager>>) {
    let response = Response::from_string(r#"{"status":"ok", "detections": []}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_confirm_detection(mut request: Request, _detection_id: &str, _db: Arc<Mutex<DbManager>>) {
    // TODO: Create tree and tree_code
    let response = Response::from_string(r#"{"status":"ok", "tree_code": "OP-000001"}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_reject_detection(mut request: Request, _detection_id: &str, _db: Arc<Mutex<DbManager>>) {
    let response = Response::from_string(r#"{"status":"ok"}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}
