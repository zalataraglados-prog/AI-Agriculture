use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::db::DbManager;

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

fn env_i32(key: &str, default_value: i32) -> i32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(default_value)
}

fn env_f64(key: &str, default_value: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default_value)
}

fn mock_crown_bbox(local_cx: f64, local_cy: f64, crown_size: f64) -> serde_json::Value {
    let half = crown_size / 2.0;
    serde_json::json!({
        "x": (local_cx - half).max(0.0),
        "y": (local_cy - half).max(0.0),
        "w": crown_size,
        "h": crown_size
    })
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
            let pid = if plantation_id == 0 {
                if let Some(existing_id) = g.get_plantation_by_name("Default Plantation")? {
                    existing_id
                } else {
                    g.insert_plantation("Default Plantation", "oil_palm")?
                }
            } else {
                plantation_id
            };
            g.insert_uav_mission(pid, mission_name)
        });

    match result {
        Ok(id) => respond_json(request, 200, &format!(r#"{{"status":"ok","mission_id":{id}}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_orthomosaic_post(mut request: Request, mission_id: &str, db: Arc<Mutex<DbManager>>) {
    let mid = mission_id.parse().unwrap_or(0);

    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();

    let width = parsed["width"].as_i64().unwrap_or(1000) as i32;
    let height = parsed["height"].as_i64().unwrap_or(1000) as i32;
    let resolution = parsed["resolution"].as_f64().unwrap_or(0.05);
    let image_url = parsed["image_url"].as_str().unwrap_or("");

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.insert_uav_orthomosaic(mid, width, height, resolution, image_url));

    match result {
        Ok(id) => respond_json(request, 200, &format!(r#"{{"status":"ok","orthomosaic_id":{id}}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_tiles_post(mut request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);

    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();

    let tile_size = parsed["tile_size"].as_i64()
        .map(|v| v as i32)
        .unwrap_or_else(|| env_i32("TILE_SIZE", 1024));
    let overlap = parsed["tile_overlap"].as_f64()
        .unwrap_or_else(|| env_f64("TILE_OVERLAP", 0.15));

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let (width, height, _resolution) = g.get_orthomosaic_dimensions(oid)?;

            let stride = ((tile_size as f64) * (1.0 - overlap)).round() as i32;
            if stride <= 0 {
                return Err("invalid stride: tile_size * (1 - overlap) must be > 0".to_string());
            }

            let cols = (width + stride - 1) / stride;
            let rows = (height + stride - 1) / stride;
            let mut tile_ids = Vec::new();

            for row in 0..rows {
                for col in 0..cols {
                    let gox = col * stride;
                    let goy = row * stride;
                    let tw = tile_size.min(width - gox);
                    let th = tile_size.min(height - goy);
                    if tw <= 0 || th <= 0 {
                        continue;
                    }
                    let tid = g.insert_uav_tile_full(oid, col, row, tw, th, gox, goy)?;
                    tile_ids.push(tid);
                }
            }

            Ok((tile_ids, cols, rows))
        });

    match result {
        Ok((tile_ids, cols, rows)) => {
            let json_ids = serde_json::to_string(&tile_ids).unwrap_or_else(|_| "[]".to_string());
            respond_json(request, 200, &format!(
                r#"{{"status":"ok","tile_ids":{},"tile_grid":{{"cols":{},"rows":{}}}}}"#,
                json_ids, cols, rows
            ));
        }
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_detect_palms(request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);
    let nms_threshold = env_f64("NMS_DISTANCE_THRESHOLD", 0.5);

    let result: Result<(Vec<(f64, f64, f64, i32, serde_json::Value, serde_json::Value)>, i32), String> =
        db.lock()
            .map_err(|_| "db lock failed".to_string())
            .and_then(|mut g| {
                let (_width, _height, resolution) = g.get_orthomosaic_dimensions(oid)?;

                let ortho = g.get_orthomosaic_full(oid)?
                    .ok_or("orthomosaic not found")?;

                let mission_id: i32 = ortho["mission_id"].as_i64().unwrap_or(0) as i32;

                let tiles = g.query_tiles_by_orthomosaic(oid)?;
                let tiles_processed = tiles.len() as i32;

                let crown_size = 60.0; // mock crown size in pixels (~1m at 0.0167m/pixel)

                // For each tile, generate 0-3 mock detections deterministically based on tile_id
                let mut raw_dets: Vec<(
                    f64, f64, f64, i32, serde_json::Value, serde_json::Value,
                )> = Vec::new();
                // (global_cx, global_cy, confidence, tile_id, bbox_tile, bbox_global)

                for tile in &tiles {
                    let tile_id = tile["id"].as_i64().unwrap_or(0) as i32;
                    let tw = tile["tile_width"].as_i64().unwrap_or(0) as f64;
                    let th = tile["tile_height"].as_i64().unwrap_or(0) as f64;
                    let gox = tile["global_offset_x"].as_i64().unwrap_or(0) as f64;
                    let goy = tile["global_offset_y"].as_i64().unwrap_or(0) as f64;
                    let tx = tile["tile_x"].as_i64().unwrap_or(0) as u64;
                    let ty = tile["tile_y"].as_i64().unwrap_or(0) as u64;

                    if tw < crown_size || th < crown_size {
                        // Too small for a detection, skip
                        continue;
                    }

                    let margin = crown_size / 2.0;
                    let avail_w = tw - crown_size;
                    let avail_h = th - crown_size;

                    let hash_val = tx.wrapping_mul(31337).wrapping_add(ty.wrapping_mul(21701));

                    let count = if avail_w > crown_size && avail_h > crown_size {
                        ((hash_val % 3) + 1) as usize
                    } else {
                        0
                    };

                    for i in 0..count {
                        let seed = hash_val.wrapping_add((i as u64).wrapping_mul(1299709));
                        let frac_x = ((seed % 1000) as f64) / 1000.0;
                        let frac_y = (((seed >> 16) % 1000) as f64) / 1000.0;
                        let conf = 0.75 + (((seed >> 8) % 240) as f64) / 1000.0; // 0.75 - 0.99

                        let local_cx = margin + frac_x * avail_w;
                        let local_cy = margin + frac_y * avail_h;

                        let global_cx = local_cx + gox;
                        let global_cy = local_cy + goy;

                        let bbox_tile = mock_crown_bbox(local_cx, local_cy, crown_size);
                        let bbox_global = mock_crown_bbox(global_cx, global_cy, crown_size);

                        raw_dets.push((global_cx, global_cy, conf, tile_id, bbox_tile, bbox_global));
                    }
                }

                // NMS requires sorting by confidence descending
                raw_dets.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

                // NMS dedup using crown center distance (in meters)
                let mut keep: Vec<bool> = vec![true; raw_dets.len()];
                let eff_res = if resolution <= 0.0 { 0.05 } else { resolution };
                let nms_pixel_threshold = nms_threshold / eff_res; // convert meters to pixels

                for i in 0..raw_dets.len() {
                    if !keep[i] { continue; }
                    let (cx_i, cy_i, conf_i, _, _, _) = raw_dets[i];
                    for j in (i + 1)..raw_dets.len() {
                        if !keep[j] { continue; }
                        let (cx_j, cy_j, conf_j, _, _, _) = raw_dets[j];
                        let dx = cx_i - cx_j;
                        let dy = cy_i - cy_j;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < nms_pixel_threshold {
                            // Suppress the one with lower confidence
                            if conf_i >= conf_j {
                                keep[j] = false;
                            } else {
                                keep[i] = false;
                                break;
                            }
                        }
                    }
                }

                // Clear previous unconfirmed detections for this orthomosaic to prevent duplicates on retry
                g.clear_pending_detections(oid)?;

                // Insert surviving detections
                for (idx, _) in keep.iter().enumerate() {
                    if !keep[idx] { continue; }
                    let (cx, cy, conf, tile_id, bbox_tile, bbox_global) = &raw_dets[idx];
                    g.insert_uav_detection_full(
                        mission_id, oid, Some(*tile_id), *cx, *cy, *conf,
                        bbox_tile.clone(), bbox_global.clone(),
                    )?;
                }

                Ok((raw_dets.iter().enumerate()
                    .filter(|(i, _)| keep[*i])
                    .map(|(_, d)| d.clone())
                    .collect(), tiles_processed))
            });

    match result {
        Ok((_kept_dets, tiles_processed)) => {
            respond_json(request, 200, &format!(
                r#"{{"status":"ok","detections_created":{},"tiles_processed":{}}}"#,
                _kept_dets.len(), tiles_processed
            ));
        }
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_get_orthomosaic(request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_orthomosaic_full(oid));

    match result {
        Ok(Some(json)) => respond_json(request, 200, &format!(
            r#"{{"status":"ok","orthomosaic":{}}}"#,
            json.to_string()
        )),
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"orthomosaic not found"}"#),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_manual_detection(mut request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);

    let mut body_bytes = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body_bytes);
    let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

    let cx = parsed["crown_center_x"].as_f64().unwrap_or(0.0);
    let cy = parsed["crown_center_y"].as_f64().unwrap_or(0.0);
    let cw = parsed["crown_width"].as_f64().unwrap_or(60.0);
    let ch = parsed["crown_height"].as_f64().unwrap_or(60.0);
    let conf = parsed["confidence"].as_f64().unwrap_or(0.85);

    let bbox = serde_json::json!({
        "x": (cx - cw / 2.0).max(0.0),
        "y": (cy - ch / 2.0).max(0.0),
        "w": cw,
        "h": ch
    });

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let mission_id = g.get_mission_id_by_orthomosaic(oid)?;
            let det_id = g.insert_uav_detection_full(
                mission_id, oid, None, cx, cy, conf,
                bbox.clone(), bbox.clone(),
            )?;
            Ok(det_id)
        });

    match result {
        Ok(det_id) => respond_json(request, 200, &format!(
            r#"{{"status":"ok","detection_id":{}}}"#, det_id
        )),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_mock_detections(request: Request, ortho_id: &str, db: Arc<Mutex<DbManager>>) {
    let oid = ortho_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let mid = g.get_mission_id_by_orthomosaic(oid)?;
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

pub(crate) fn handle_confirm_detection(request: Request, detection_id: &str, db: Arc<Mutex<DbManager>>) {
    let det_id = detection_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let det = g.get_detection_by_id(det_id)?.ok_or("detection not found")?;
            
            let status = det["review_status"].as_str().unwrap_or("");
            let matched_id = det["matched_tree_id"].as_i64();
            
            // Idempotency check
            if status == "confirmed" && matched_id.is_some() {
                // Return existing tree code
                let matched = matched_id.unwrap() as i32;
                if let Some(code) = g.get_tree_code_by_id(matched)? {
                    return Ok(code);
                }
            }
            
            if status == "rejected" {
                return Err("cannot confirm a rejected detection".to_string());
            }

            let oid = det["orthomosaic_id"].as_i64().map(|x| x as i32);
            let cx = det["crown_center_x"].as_f64();
            let cy = det["crown_center_y"].as_f64();
            
            let seq = g.next_tree_code_seq()?;
            let tree_code = format!("OP-{:06}", seq);
            
            let pid = g.get_plantation_id_by_detection(det_id)?;
            let _tree_id = g.confirm_detection_tx(det_id, pid, "oil_palm", &tree_code, cx, cy, oid)?;
            
            Ok(tree_code)
        });

    match result {
        Ok(code) => respond_json(request, 200, &format!(r#"{{"status":"ok","tree_code":"{code}"}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_reject_detection(request: Request, detection_id: &str, db: Arc<Mutex<DbManager>>) {
    let det_id = detection_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.update_detection_status(det_id, "rejected"));

    match result {
        Ok(_) => respond_json(request, 200, r#"{"status":"ok"}"#),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}
