use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::db::DbManager;

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

const ALLOWED_TREE_STATUSES: &[&str] = &["active", "dead", "removed", "replanted"];

pub(crate) fn handle_get_tree(request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_by_code(tree_code));

    match result {
        Ok(Some(json)) => respond_json(request, 200, &format!(r#"{{"status":"ok","tree":{}}}"#, json.to_string())),
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{}"}}"#, e)),
    }
}

pub(crate) fn handle_get_timeline(request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_tree_timeline(tree_code));

    match result {
        Ok(list) => respond_json(request, 200, &format!(
            r#"{{"status":"ok","timeline":{}}}"#,
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
        )),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{}"}}"#, e)),
    }
}

pub(crate) fn handle_update_status(mut request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let status_str = parsed["status"].as_str().unwrap_or("");

    if !ALLOWED_TREE_STATUSES.contains(&status_str) {
        let body = serde_json::json!({
            "status": "error",
            "message": format!("invalid status '{}'", status_str),
            "allowed_statuses": ALLOWED_TREE_STATUSES,
        });
        respond_json(request, 400, &body.to_string());
        return;
    }

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.update_tree_status(tree_code, status_str));

    match result {
        Ok(()) => respond_json(request, 200, r#"{"status":"ok"}"#),
        Err(e) if e.contains("tree not found") => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{}"}}"#, e)),
    }
}

pub(crate) fn handle_list_trees(request: Request, query: &str, db: Arc<Mutex<DbManager>>) {
    let params = parse_query_params(query);
    let plantation_id: i32 = params.get("plantation_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let page: i64 = params.get("page")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let limit: i64 = params.get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
        .min(100)
        .max(1);
    let offset = (page - 1) * limit;

    let mission_id: i32 = params.get("mission_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            if plantation_id > 0 {
                let total = g.count_trees_by_plantation_ext(plantation_id, mission_id)?;
                let trees = g.list_trees_by_plantation_ext(plantation_id, mission_id, limit, offset)?;
                Ok((trees, total))
            } else {
                let total = g.count_all_trees_ext(mission_id)?;
                let trees = g.list_all_trees_ext(mission_id, limit, offset)?;
                Ok((trees, total))
            }
        });

    match result {
        Ok((trees, total)) => {
            let trees_json = serde_json::to_string(&trees).unwrap_or_else(|_| "[]".to_string());
            respond_json(request, 200, &format!(
                r#"{{"status":"ok","trees":{},"page":{},"limit":{},"total":{}}}"#,
                trees_json, page, limit, total
            ));
        }
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{}"}}"#, e)),
    }
}

pub(crate) fn handle_list_plantations(request: Request, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.list_plantations());

    match result {
        Ok(list) => respond_json(request, 200, &format!(
            r#"{{"status":"ok","plantations":{}}}"#,
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
        )),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{}"}}"#, e)),
    }
}

fn parse_query_params(query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}
