use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::db::DbManager;

pub(crate) fn handle_get_tree(request: Request, _tree_code: &str, _db: Arc<Mutex<DbManager>>) {
    let response = Response::from_string(r#"{"status":"ok", "tree": {"tree_code": "OP-000001", "species": "oil_palm"}}"#)
        .with_status_code(200)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}
