use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response};
use crate::ai_client::analyze_oil_palm_from_bytes;
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

fn env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_value)
}

#[derive(Debug, Clone)]
struct TileInfo {
    id: i32,
    col: i32,
    row: i32,
    width: u32,
    height: u32,
    offset_x: u32,
    offset_y: u32,
}

#[derive(Debug, Clone)]
struct RawDetection {
    global_cx: f64,
    global_cy: f64,
    confidence: f64,
    tile_id: i32,
    bbox_tile: serde_json::Value,
    bbox_global: serde_json::Value,
}

fn tile_info_from_json(tile: &serde_json::Value) -> Result<TileInfo, String> {
    let id = tile["id"].as_i64().unwrap_or(0) as i32;
    let col = tile["tile_x"].as_i64().unwrap_or(0) as i32;
    let row = tile["tile_y"].as_i64().unwrap_or(0) as i32;
    let width = non_negative_u32(tile, "tile_width", id)?;
    let height = non_negative_u32(tile, "tile_height", id)?;
    let offset_x = non_negative_u32(tile, "global_offset_x", id)?;
    let offset_y = non_negative_u32(tile, "global_offset_y", id)?;

    Ok(TileInfo {
        id,
        col,
        row,
        width,
        height,
        offset_x,
        offset_y,
    })
}

fn non_negative_u32(tile: &serde_json::Value, key: &str, tile_id: i32) -> Result<u32, String> {
    let value = tile[key]
        .as_i64()
        .ok_or_else(|| format!("tile {tile_id} missing numeric {key}"))?;
    if value < 0 || value > u32::MAX as i64 {
        return Err(format!("tile {tile_id} has invalid {key}: {value}"));
    }
    Ok(value as u32)
}

fn tile_grid_summary(tiles: &[serde_json::Value]) -> Result<(Vec<i32>, i32, i32), String> {
    let mut tile_ids = Vec::new();
    let mut max_col = -1;
    let mut max_row = -1;

    for tile in tiles {
        let info = tile_info_from_json(tile)?;
        tile_ids.push(info.id);
        max_col = max_col.max(info.col);
        max_row = max_row.max(info.row);
    }

    Ok((tile_ids, max_col + 1, max_row + 1))
}

fn dedupe_tiles(tiles: Vec<TileInfo>) -> Vec<TileInfo> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for tile in tiles {
        let key = (tile.offset_x, tile.offset_y, tile.width, tile.height);
        if seen.insert(key) {
            out.push(tile);
        }
    }

    out
}

pub(crate) fn handle_missions_post(mut request: Request, db: Arc<Mutex<DbManager>>) {
    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();

    let plantation_id = parsed["plantation_id"].as_i64().unwrap_or(0) as i32;
    let mission_name = parsed["mission_name"].as_str().unwrap_or("unnamed");
    let plantation_name = parsed["plantation_name"].as_str().unwrap_or("Default Plantation");

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let pid = if plantation_id == 0 {
                if let Some(existing_id) = g.get_plantation_by_name(plantation_name)? {
                    existing_id
                } else {
                    g.insert_plantation(plantation_name, "oil_palm")?
                }
            } else {
                plantation_id
            };
            g.insert_uav_mission(pid, mission_name).map(|mid| (mid, pid))
        });

    match result {
        Ok((mid, pid)) => respond_json(request, 200, &format!(r#"{{"status":"ok","mission_id":{},"plantation_id":{}}}"#, mid, pid)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_missions_get(request: Request, query: &str, db: Arc<Mutex<DbManager>>) {
    let params = crate::http_server::parse_query(query);
    let pid: i32 = params.get("plantation_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
        
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.query_uav_missions_by_plantation(pid));
        
    match result {
        Ok(missions) => respond_json(request, 200, &format!(r#"{{"status":"ok","missions":{}}}"#, serde_json::to_string(&missions).unwrap())),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_orthomosaics_get(request: Request, query: &str, db: Arc<Mutex<DbManager>>) {
    let params = crate::http_server::parse_query(query);
    let plantation_id: i32 = params.get("plantation_id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let limit: i64 = params.get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.query_uav_orthomosaics(plantation_id, limit));

    match result {
        Ok(orthomosaics) => respond_json(
            request,
            200,
            &serde_json::json!({"status":"ok","orthomosaics":orthomosaics}).to_string(),
        ),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_mission_orthomosaic_get(request: Request, mission_id: &str, db: Arc<Mutex<DbManager>>) {
    let mid = mission_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| g.get_latest_orthomosaic_by_mission(mid));

    match result {
        Ok(Some(orthomosaic)) => respond_json(
            request,
            200,
            &serde_json::json!({
                "status": "ok",
                "id": orthomosaic["id"],
                "orthomosaic_id": orthomosaic["orthomosaic_id"],
                "orthomosaic": orthomosaic
            }).to_string(),
        ),
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"orthomosaic not found"}"#),
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

            let existing_tiles = g.query_tiles_by_orthomosaic(oid)?;
            if !existing_tiles.is_empty() {
                let (tile_ids, cols, rows) = tile_grid_summary(&existing_tiles)?;
                return Ok((tile_ids, cols, rows, true));
            }

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

            Ok((tile_ids, cols, rows, false))
        });

    match result {
        Ok((tile_ids, cols, rows, reused)) => {
            let json_ids = serde_json::to_string(&tile_ids).unwrap_or_else(|_| "[]".to_string());
            respond_json(request, 200, &format!(
                r#"{{"status":"ok","tile_ids":{},"tile_grid":{{"cols":{},"rows":{}}},"tiles_reused":{}}}"#,
                json_ids, cols, rows, reused
            ));
        }
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_detect_palms(
    request: Request,
    ortho_id: &str,
    image_store_path: &str,
    ai_oil_palm_analyze_url: &str,
    ai_http_client: &reqwest::blocking::Client,
    db: Arc<Mutex<DbManager>>,
) {
    let oid = ortho_id.parse().unwrap_or(0);
    let nms_threshold = env_f64("NMS_DISTANCE_THRESHOLD", 0.5);

    if ai_oil_palm_analyze_url.trim().is_empty() {
        respond_json(
            request,
            503,
            r#"{"status":"error","message":"AI_OIL_PALM_ANALYZE_URL is required for /detect-palms; use /detections/mock for explicit mock detections"}"#,
        );
        return;
    }

    let result: Result<(usize, i32), String> = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let (width, height, resolution) = g.get_orthomosaic_dimensions(oid)?;
            let ortho = g.get_orthomosaic_full(oid)?
                .ok_or("orthomosaic not found")?;
            let mission_id: i32 = ortho["mission_id"].as_i64().unwrap_or(0) as i32;
            let image_url = ortho["image_url"].as_str().unwrap_or("").trim().to_string();
            if image_url.is_empty() {
                return Err("orthomosaic image_url is required for AI detection".to_string());
            }

            let tiles_json = g.query_tiles_by_orthomosaic(oid)?;
            let tiles = tiles_json
                .iter()
                .map(tile_info_from_json)
                .collect::<Result<Vec<_>, _>>()?;
            let tiles = dedupe_tiles(tiles);
            if tiles.is_empty() {
                return Err("orthomosaic has no tiles; call /api/v1/uav/orthomosaics/{id}/tiles before /detect-palms".to_string());
            }

            Ok((mission_id, width, height, resolution, image_url, tiles))
        })
        .and_then(|(mission_id, width, height, resolution, image_url, tiles)| {
            let max_pixels = env_u64("UAV_ORTHOMOSAIC_MAX_DECODE_PIXELS", 120_000_000);
            if (width as u64).saturating_mul(height as u64) > max_pixels {
                return Err(format!(
                    "orthomosaic dimensions {}x{} exceed UAV_ORTHOMOSAIC_MAX_DECODE_PIXELS={}; use pre-generated physical tiles or raise the configured limit",
                    width, height, max_pixels
                ));
            }

            let img_bytes = load_orthomosaic_image_bytes(&image_url, image_store_path, ai_http_client)?;
            let img = image::load_from_memory(&img_bytes)
                .map_err(|e| format!("failed to decode orthomosaic image: {e}"))?;
            let actual_pixels = (img.width() as u64).saturating_mul(img.height() as u64);
            if actual_pixels > max_pixels {
                return Err(format!(
                    "decoded orthomosaic dimensions {}x{} exceed UAV_ORTHOMOSAIC_MAX_DECODE_PIXELS={}",
                    img.width(),
                    img.height(),
                    max_pixels
                ));
            }

            let tiles_processed = tiles.len() as i32;
            let mut raw_dets = Vec::new();
            for tile in &tiles {
                raw_dets.extend(process_tile_for_ai(
                    &img,
                    tile,
                    ai_oil_palm_analyze_url,
                    ai_http_client,
                )?);
            }

            let kept_dets = nms_by_center(raw_dets, resolution, nms_threshold);
            let created = kept_dets.len();

            db.lock()
                .map_err(|_| "db lock failed".to_string())
                .and_then(|mut g| {
                    g.clear_pending_detections(oid)?;
                    for det in &kept_dets {
                        g.insert_uav_detection_full(
                            mission_id,
                            oid,
                            Some(det.tile_id),
                            det.global_cx,
                            det.global_cy,
                            det.confidence,
                            det.bbox_tile.clone(),
                            det.bbox_global.clone(),
                        )?;
                    }
                    Ok(())
                })?;

            Ok((created, tiles_processed))
        });

    match result {
        Ok((created, tiles_processed)) => {
            respond_json(request, 200, &format!(
                r#"{{"status":"ok","detections_created":{},"tiles_processed":{}}}"#,
                created, tiles_processed
            ));
        }
        Err(e) => respond_json(request, detect_error_status(&e), &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

fn detect_error_status(message: &str) -> u16 {
    if message.contains("AI_OIL_PALM_ANALYZE_URL") {
        503
    } else if message.contains("oil palm analyze API")
        || message.contains("failed to call")
        || message.contains("failed to parse oil palm analyze")
    {
        502
    } else if message.contains("has no tiles") || message.contains("image_url is required") {
        409
    } else {
        500
    }
}

fn load_orthomosaic_image_bytes(
    image_url: &str,
    image_store_path: &str,
    http_client: &reqwest::blocking::Client,
) -> Result<Vec<u8>, String> {
    let trimmed = image_url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let response = http_client
            .get(trimmed)
            .send()
            .map_err(|e| format!("failed to download orthomosaic image {trimmed}: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("orthomosaic image download returned HTTP {status}: {trimmed}"));
        }
        return response
            .bytes()
            .map(|b| b.to_vec())
            .map_err(|e| format!("failed to read orthomosaic image body: {e}"));
    }

    let path = resolve_orthomosaic_local_path(trimmed, image_store_path)
        .ok_or_else(|| format!("failed to resolve orthomosaic image path: {trimmed}"))?;
    std::fs::read(&path)
        .map_err(|e| format!("failed to read orthomosaic image {}: {e}", path.display()))
}

fn resolve_orthomosaic_local_path(image_url: &str, image_store_path: &str) -> Option<PathBuf> {
    let path_text = image_url.strip_prefix("file://").unwrap_or(image_url).trim();
    if path_text.is_empty() {
        return None;
    }

    let direct = PathBuf::from(path_text);
    if direct.exists() {
        return Some(direct);
    }

    let normalized = path_text.trim_start_matches('/');
    let mut candidates = Vec::new();

    if let Ok(project_root) = std::env::var("PROJECT_ROOT") {
        push_orthomosaic_path_candidates(&mut candidates, Path::new(&project_root), normalized);
    }
    for key in [
        "STATIC_SOURCE_FRONTEND",
        "STATIC_TARGET_FRONTEND",
        "STATIC_SOURCE_DASHBOARD",
        "STATIC_TARGET_DASHBOARD",
    ] {
        if let Ok(root) = std::env::var(key) {
            push_orthomosaic_path_candidates(&mut candidates, Path::new(&root), normalized);
        }
    }
    if !image_store_path.trim().is_empty() {
        push_orthomosaic_path_candidates(&mut candidates, Path::new(image_store_path), normalized);
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_orthomosaic_path_candidates(&mut candidates, &cwd, normalized);
    }

    candidates.into_iter().find(|path| path.exists())
}

fn push_orthomosaic_path_candidates(candidates: &mut Vec<PathBuf>, root: &Path, normalized: &str) {
    candidates.push(root.join(normalized));
    if let Some(static_path) = normalized.strip_prefix("static/") {
        candidates.push(root.join(static_path));
    }
    candidates.push(root.join("frontend").join(normalized));
    if let Some(static_path) = normalized.strip_prefix("static/") {
        candidates.push(root.join("frontend").join(static_path));
    }
}

fn process_tile_for_ai(
    img: &image::DynamicImage,
    tile: &TileInfo,
    ai_oil_palm_analyze_url: &str,
    ai_http_client: &reqwest::blocking::Client,
) -> Result<Vec<RawDetection>, String> {
    if tile.width < 32 || tile.height < 32 {
        return Ok(Vec::new());
    }
    let end_x = tile.offset_x.checked_add(tile.width)
        .ok_or_else(|| format!("tile {} x bounds overflow", tile.id))?;
    let end_y = tile.offset_y.checked_add(tile.height)
        .ok_or_else(|| format!("tile {} y bounds overflow", tile.id))?;
    if end_x > img.width() || end_y > img.height() {
        return Err(format!(
            "tile {} crop bounds x={} y={} w={} h={} exceed decoded image {}x{}",
            tile.id,
            tile.offset_x,
            tile.offset_y,
            tile.width,
            tile.height,
            img.width(),
            img.height()
        ));
    }

    let tile_img = image::imageops::crop_imm(
        img,
        tile.offset_x,
        tile.offset_y,
        tile.width,
        tile.height,
    ).to_image();
    let tile_bytes = encode_tile_as_jpeg(tile_img, tile.id)?;

    let ai_json = analyze_oil_palm_from_bytes(
        ai_http_client,
        ai_oil_palm_analyze_url,
        &tile_bytes,
        Some(&format!("uav_tile_{}.jpg", tile.id)),
        "jpg",
        "uav_tile",
        None,
        None,
    )?;

    detections_from_ai_response(&ai_json, tile)
}

fn encode_tile_as_jpeg(tile_img: image::RgbaImage, tile_id: i32) -> Result<Vec<u8>, String> {
    let rgb_img = image::DynamicImage::ImageRgba8(tile_img).to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(rgb_img)
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| format!("failed to encode tile {tile_id}: {e}"))?;
    Ok(buf.into_inner())
}

fn detections_from_ai_response(
    ai_json: &serde_json::Value,
    tile: &TileInfo,
) -> Result<Vec<RawDetection>, String> {
    let Some(results) = ai_json["results"].as_array() else {
        return Err("oil palm analyze response missing results[]".to_string());
    };

    let mut out = Vec::new();
    for (idx, det) in results.iter().enumerate() {
        let conf = det["confidence"]
            .as_f64()
            .ok_or_else(|| format!("tile {} detection {} missing confidence", tile.id, idx))?;
        let geom = det["geometry"]
            .as_object()
            .ok_or_else(|| format!("tile {} detection {} missing geometry object", tile.id, idx))?;
        if geom.get("type").and_then(|v| v.as_str()) != Some("bbox") {
            return Err(format!("tile {} detection {} geometry must be bbox", tile.id, idx));
        }

        let nx = normalized_number(geom.get("x"), "x", tile.id, idx)?;
        let ny = normalized_number(geom.get("y"), "y", tile.id, idx)?;
        let nw = normalized_number(geom.get("w"), "w", tile.id, idx)?;
        let nh = normalized_number(geom.get("h"), "h", tile.id, idx)?;
        if nw <= 0.0 || nh <= 0.0 {
            continue;
        }

        let local_x = nx * (tile.width as f64);
        let local_y = ny * (tile.height as f64);
        let w = nw * (tile.width as f64);
        let h = nh * (tile.height as f64);
        let local_cx = local_x + w / 2.0;
        let local_cy = local_y + h / 2.0;
        let global_x = local_x + tile.offset_x as f64;
        let global_y = local_y + tile.offset_y as f64;
        let global_cx = local_cx + tile.offset_x as f64;
        let global_cy = local_cy + tile.offset_y as f64;

        let bbox_tile = serde_json::json!({
            "type": "bbox",
            "x": local_x,
            "y": local_y,
            "w": w,
            "h": h,
            "coordinate_scope": "tile_pixels"
        });
        let bbox_global = serde_json::json!({
            "type": "bbox",
            "x": global_x,
            "y": global_y,
            "w": w,
            "h": h,
            "coordinate_scope": "orthomosaic_pixels"
        });

        out.push(RawDetection {
            global_cx,
            global_cy,
            confidence: conf,
            tile_id: tile.id,
            bbox_tile,
            bbox_global,
        });
    }

    Ok(out)
}

fn normalized_number(
    value: Option<&serde_json::Value>,
    key: &str,
    tile_id: i32,
    detection_idx: usize,
) -> Result<f64, String> {
    let number = value
        .and_then(|v| v.as_f64())
        .ok_or_else(|| format!("tile {tile_id} detection {detection_idx} missing geometry.{key}"))?;
    if !number.is_finite() || !(0.0..=1.0).contains(&number) {
        return Err(format!(
            "tile {tile_id} detection {detection_idx} has out-of-range geometry.{key}: {number}"
        ));
    }
    Ok(number)
}

fn nms_by_center(
    mut raw_dets: Vec<RawDetection>,
    resolution: f64,
    nms_threshold_meters: f64,
) -> Vec<RawDetection> {
    raw_dets.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut keep = vec![true; raw_dets.len()];
    let eff_res = if resolution <= 0.0 { 0.05 } else { resolution };
    let nms_pixel_threshold = nms_threshold_meters / eff_res;

    for i in 0..raw_dets.len() {
        if !keep[i] {
            continue;
        }
        for j in (i + 1)..raw_dets.len() {
            if !keep[j] {
                continue;
            }
            let dx = raw_dets[i].global_cx - raw_dets[j].global_cx;
            let dy = raw_dets[i].global_cy - raw_dets[j].global_cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < nms_pixel_threshold {
                keep[j] = false;
            }
        }
    }

    raw_dets
        .into_iter()
        .enumerate()
        .filter_map(|(idx, det)| keep[idx].then_some(det))
        .collect()
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

pub(crate) fn handle_match_existing_trees(request: Request, mission_id: &str, db: Arc<Mutex<DbManager>>) {
    let mid = mission_id.parse().unwrap_or(0);
    let match_threshold_meters = env_f64("MATCH_DISTANCE_THRESHOLD", 1.5);

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let detections = g.get_detections_by_mission(mid)?;
            let pid = g.get_plantation_id_by_mission(mid)?;

            let mut auto_matched = 0u32;
            let mut ambiguous = 0u32;
            let mut unmatched = 0u32;

            for det in &detections {
                let det_id = det["id"].as_i64().unwrap_or(0) as i32;
                let cx = match det["crown_center_x"].as_f64() {
                    Some(v) => v,
                    None => { unmatched += 1; continue; }
                };
                let cy = match det["crown_center_y"].as_f64() {
                    Some(v) => v,
                    None => { unmatched += 1; continue; }
                };
                let bbox = det["bbox_global_json"].clone();
                let confidence = det["confidence"].as_f64().unwrap_or(0.5);

                let oid = det["orthomosaic_id"].as_i64().map(|x| x as i32);
                let resolution = match oid {
                    Some(oid_val) => g.get_orthomosaic_dimensions(oid_val).ok().map(|(_, _, r)| r).unwrap_or(0.05),
                    None => 0.05,
                };

                let max_pixel_dist = match_threshold_meters / resolution;
                let nearby = g.find_nearby_trees(pid, cx, cy, max_pixel_dist, 3)?;

                if nearby.is_empty() {
                    unmatched += 1;
                    continue;
                }

                if nearby.len() == 1 {
                    let tree_id = nearby[0]["tree_id"].as_i64().unwrap_or(0) as i32;
                    let dist = nearby[0]["distance_pixels"].as_f64().unwrap_or(0.0);
                    let shift_meters = dist * resolution;
                    let crown_bbox = if bbox.is_null() { None } else { Some(bbox) };

                    g.match_detection_to_tree_tx(
                        det_id, tree_id, mid, cx, cy, shift_meters,
                        Some(confidence), crown_bbox,
                    )?;
                    auto_matched += 1;
                } else {
                    ambiguous += 1;
                }
            }

            Ok((auto_matched, ambiguous, unmatched))
        });

    match result {
        Ok((auto, amb, unm)) => respond_json(request, 200, &format!(
            r#"{{"status":"ok","auto_matched":{},"ambiguous":{},"unmatched":{}}}"#,
            auto, amb, unm
        )),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_match_review(request: Request, mission_id: &str, db: Arc<Mutex<DbManager>>) {
    let mid = mission_id.parse().unwrap_or(0);
    let match_threshold_meters = env_f64("MATCH_DISTANCE_THRESHOLD", 1.5);

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let detections = g.get_detections_by_mission(mid)?;
            let pid = g.get_plantation_id_by_mission(mid)?;
            let mut review_list: Vec<serde_json::Value> = Vec::new();

            for det in &detections {
                let det_id = det["id"].as_i64().unwrap_or(0) as i32;
                let cx = match det["crown_center_x"].as_f64() { Some(v) => v, None => continue };
                let cy = match det["crown_center_y"].as_f64() { Some(v) => v, None => continue };
                let confidence = det["confidence"].as_f64().unwrap_or(0.5);

                let oid = det["orthomosaic_id"].as_i64().map(|x| x as i32);
                let resolution = oid.and_then(|oid_val| {
                    g.get_orthomosaic_dimensions(oid_val).ok().map(|(_, _, r)| r)
                }).unwrap_or(0.05);

                let max_pixel_dist = match_threshold_meters / resolution;
                let nearby = g.find_nearby_trees(pid, cx, cy, max_pixel_dist, 5)?;

                if nearby.len() >= 2 {
                    review_list.push(serde_json::json!({
                        "detection_id": det_id,
                        "crown_center_x": cx,
                        "crown_center_y": cy,
                        "confidence": confidence,
                        "candidates": nearby
                    }));
                }
            }

            Ok(review_list)
        });

    match result {
        Ok(list) => {
            let json_list = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
            respond_json(request, 200, &format!(r#"{{"status":"ok","reviews":{}}}"#, json_list));
        }
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

pub(crate) fn handle_match_to_tree(mut request: Request, detection_id: &str, db: Arc<Mutex<DbManager>>) {
    let det_id = detection_id.parse().unwrap_or(0);

    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let tree_id = parsed["tree_id"].as_i64().unwrap_or(0) as i32;

    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| {
            let det = g.get_detection_by_id(det_id)?.ok_or("detection not found")?;
            let status = det["review_status"].as_str().unwrap_or("");
            if status != "pending" {
                return Err("detection is not in pending state".to_string());
            }
            let cx = det["crown_center_x"].as_f64().unwrap_or(0.0);
            let cy = det["crown_center_y"].as_f64().unwrap_or(0.0);
            let confidence = det["confidence"].as_f64().unwrap_or(0.5);

            let tree = g.get_tree_by_id(tree_id)?.ok_or("tree not found")?;
            let tree_cx = tree["crown_center_x"].as_f64().unwrap_or(cx);
            let tree_cy = tree["crown_center_y"].as_f64().unwrap_or(cy);
            let dx = cx - tree_cx;
            let dy = cy - tree_cy;
            let shift = (dx * dx + dy * dy).sqrt();

            let mid = g.get_detection_mission_id(det_id)?;
            g.match_detection_to_tree_tx(det_id, tree_id, mid, cx, cy, shift, Some(confidence), None)?;

            Ok(tree["tree_code"].as_str().unwrap_or("?").to_string())
        });

    match result {
        Ok(code) => respond_json(request, 200, &format!(r#"{{"status":"ok","tree_code":"{code}"}}"#)),
        Err(e) => respond_json(request, 500, &format!(r#"{{"status":"error","message":"{e}"}}"#)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tile_info_uses_global_offsets_as_crop_origin() {
        let tile = json!({
            "id": 7,
            "tile_x": 1,
            "tile_y": 2,
            "tile_width": 1024,
            "tile_height": 512,
            "global_offset_x": 870,
            "global_offset_y": 1740
        });

        let info = tile_info_from_json(&tile).expect("valid tile");
        assert_eq!(info.col, 1);
        assert_eq!(info.row, 2);
        assert_eq!(info.offset_x, 870);
        assert_eq!(info.offset_y, 1740);
    }

    #[test]
    fn detections_from_ai_response_restores_global_pixel_coordinates() {
        let tile = TileInfo {
            id: 3,
            col: 1,
            row: 1,
            width: 1000,
            height: 500,
            offset_x: 100,
            offset_y: 200,
        };
        let ai_json = json!({
            "status": "success",
            "results": [{
                "task": "uav_tree_crown",
                "label": "oil_palm_crown",
                "confidence": 0.9,
                "geometry": {"type": "bbox", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4}
            }],
            "metadata": {"task": "uav_tree_crown"},
            "model_version": "test"
        });

        let dets = detections_from_ai_response(&ai_json, &tile).expect("valid detections");
        assert_eq!(dets.len(), 1);
        assert_eq!(dets[0].global_cx, 350.0);
        assert_eq!(dets[0].global_cy, 400.0);
        assert_eq!(dets[0].bbox_global["x"].as_f64().unwrap(), 200.0);
        assert_eq!(dets[0].bbox_global["y"].as_f64().unwrap(), 300.0);
    }

    #[test]
    fn center_nms_keeps_highest_confidence_detection() {
        let bbox = json!({"type": "bbox", "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0});
        let kept = nms_by_center(
            vec![
                RawDetection {
                    global_cx: 100.0,
                    global_cy: 100.0,
                    confidence: 0.7,
                    tile_id: 1,
                    bbox_tile: bbox.clone(),
                    bbox_global: bbox.clone(),
                },
                RawDetection {
                    global_cx: 102.0,
                    global_cy: 100.0,
                    confidence: 0.9,
                    tile_id: 2,
                    bbox_tile: bbox.clone(),
                    bbox_global: bbox.clone(),
                },
                RawDetection {
                    global_cx: 200.0,
                    global_cy: 200.0,
                    confidence: 0.6,
                    tile_id: 3,
                    bbox_tile: bbox.clone(),
                    bbox_global: bbox,
                },
            ],
            0.05,
            0.5,
        );

        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].tile_id, 2);
        assert!(kept.iter().any(|det| det.tile_id == 3));
    }

    #[test]
    fn encode_tile_as_jpeg_accepts_rgba_tiles() {
        let rgba = image::RgbaImage::from_pixel(64, 64, image::Rgba([12, 34, 56, 180]));
        let encoded = encode_tile_as_jpeg(rgba, 121).expect("rgba tile should encode as jpeg");

        assert!(encoded.starts_with(&[0xFF, 0xD8, 0xFF]));
    }
}
