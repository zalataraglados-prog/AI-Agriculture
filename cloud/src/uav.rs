use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::db::DbManager;

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_missions_post(mut request: Request, db: Arc<Mutex<DbManager>>) {
    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();

    let plantation_id = parsed["plantation_id"].as_i64().unwrap_or(0) as i32;
    let mission_name = parsed["mission_name"].as_str().unwrap_or("unnamed");

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            if plantation_id == 0 {
                let pid = g.insert_plantation("test_plantation", "oil_palm")?;
                g.insert_uav_mission(pid, mission_name)
            } else {
                g.insert_uav_mission(plantation_id, mission_name)
            }
        });

    match result {
        Ok(id) => respond_json(request, 200, &format!(r#"{{"status":"ok","mission_id":{id}}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_orthomosaic_post(mut request: Request, mission_id: &str, db: Arc<Mutex<DbManager>>) {
    let mid = mission_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.insert_uav_orthomosaic(mid, 1000, 1000));

    match result {
        Ok(id) => respond_json(request, 200, &format!(r#"{{"status":"ok","orthomosaic_id":{id}}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_tiles_post(mut request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.insert_uav_tile(oid, 0, 0));

    match result {
        Ok(id) => respond_json(request, 200, &format!(r#"{{"status":"ok","tile_id":{id}}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_mock_detections(mut request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            // we assume a fixed mission_id for the mock, maybe 1
            let mid = 1;
            g.insert_uav_detection(mid, oid, 10.0, 20.0, 0.95)?;
            g.insert_uav_detection(mid, oid, 30.0, 40.0, 0.92)?;
            g.insert_uav_detection(mid, oid, 50.0, 60.0, 0.88)?;
            Ok(3)
        });

    match result {
        Ok(count) => respond_json(request, 200, &format!(r#"{{"status":"ok","detections_created":{count}}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_get_detections(request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.query_detections_by_orthomosaic(oid));

    match result {
        Ok(list) => respond_json(request, 200, &format!(r#"{{"status":"ok","detections":{}}}"#, serde_json::to_string(&list).unwrap())),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_confirm_detection(mut request: Request, detection_id: &str, db: Arc<Mutex<DbManager>>) {
    let det_id = detection_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let det = g.get_detection_by_id(det_id)?.ok_or("detection not found")?;
            let oid = det["orthomosaic_id"].as_i64().map(|x| x as i32);
            let cx = det["crown_center_x"].as_f64();
            let cy = det["crown_center_y"].as_f64();
            
            let seq = g.get_max_tree_seq("OP-")? + 1;
            let tree_code = format!("OP-{:06}", seq);
            
            // fixed plantation_id for now
            let pid = 1; 
            let tree_id = g.insert_tree(pid, "oil_palm", &tree_code, cx, cy, oid)?;
            
            g.update_detection_status(det_id, "confirmed")?;
            g.link_detection_to_tree(det_id, tree_id)?;
            
            Ok(tree_code)
        });

    match result {
        Ok(code) => respond_json(request, 200, &format!(r#"{{"status":"ok","tree_code":"{code}"}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_reject_detection(mut request: Request, detection_id: &str, db: Arc<Mutex<DbManager>>) {
    let det_id = detection_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.update_detection_status(det_id, "rejected"));

    match result {
        Ok(_) => respond_json(request, 200, r#"{"status":"ok"}"#),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}
