use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::db::DbManager;

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_get_tree(request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_by_code(tree_code));

    match result {
        Ok(Some(json)) => respond_json(request, 200, &format!(r#"{{"status":"ok","tree":{}}}"#, json.to_string())),
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}
