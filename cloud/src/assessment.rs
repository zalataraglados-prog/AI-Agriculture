use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use tiny_http::{Request, Response};

use crate::db::DbManager;

fn respond_json(request: Request, status: u16, body: &str) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}

pub(crate) fn handle_tree_assessment(request: Request, tree_code: &str, db: Arc<Mutex<DbManager>>) {
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| build_tree_assessment_by_code(&mut g, tree_code));

    match result {
        Ok(Some(assessment)) => respond_json(request, 200, &json!({
            "status": "ok",
            "assessment": assessment
        }).to_string()),
        Ok(None) => respond_json(request, 404, r#"{"status":"error","message":"tree not found"}"#),
        Err(e) => respond_json(request, 500, &json!({"status":"error","message":e}).to_string()),
    }
}

pub(crate) fn handle_plantation_dashboard(request: Request, plantation_id: &str, db: Arc<Mutex<DbManager>>) {
    let pid = plantation_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| build_plantation_dashboard(&mut g, pid));

    match result {
        Ok(dashboard) => respond_json(request, 200, &json!({
            "status": "ok",
            "dashboard": dashboard
        }).to_string()),
        Err(e) => respond_json(request, 500, &json!({"status":"error","message":e}).to_string()),
    }
}

pub(crate) fn handle_blocks_report(request: Request, plantation_id: &str, db: Arc<Mutex<DbManager>>) {
    let pid = plantation_id.parse().unwrap_or(0);
    let result = db.lock()
        .map_err(|_| "db lock failed".to_string())
        .and_then(|mut g| build_blocks_report(&mut g, pid));

    match result {
        Ok(report) => respond_json(request, 200, &json!({
            "status": "ok",
            "report": report
        }).to_string()),
        Err(e) => respond_json(request, 500, &json!({"status":"error","message":e}).to_string()),
    }
}

pub(crate) fn build_tree_assessment_by_code(g: &mut DbManager, tree_code: &str) -> Result<Option<Value>, String> {
    let Some(tree) = g.get_tree_by_code(tree_code)? else {
        return Ok(None);
    };
    let tree_id = tree["id"].as_i64().unwrap_or(0) as i32;
    let images = g.get_session_images_by_tree_id(tree_id)?;
    let history = g.get_tree_coordinate_history_by_tree_id(tree_id)?;
    Ok(Some(build_tree_assessment(tree, images, history)))
}

pub(crate) fn build_plantation_dashboard(g: &mut DbManager, plantation_id: i32) -> Result<Value, String> {
    let tree_refs = g.list_assessment_trees_by_plantation(plantation_id)?;
    let mut trees = Vec::new();
    let mut harvest_recommended = 0;
    let mut disease_risk = 0;
    let mut abnormal = 0;
    let mut missing_evidence = 0;
    let mut complete = 0;
    let mut active = 0;
    let mut priority = Vec::new();

    for tree_ref in tree_refs {
        let tree_code = tree_ref["tree_code"].as_str().unwrap_or("").to_string();
        let Some(assessment) = build_tree_assessment_by_code(g, &tree_code)? else {
            continue;
        };
        let is_active = assessment["tree"]["current_status"].as_str() == Some("active");
        let dashboard_action = if is_active {
            assessment["recommended_action"].as_str().unwrap_or("wait")
        } else {
            "inactive"
        };

        if is_active {
            active += 1;
            if assessment["dimensions"]["fruit"]["status"].as_str() == Some("harvest_ready") {
                harvest_recommended += 1;
            }
            if ["medium", "high"].contains(&assessment["dimensions"]["disease"]["risk_level"].as_str().unwrap_or("")) {
                disease_risk += 1;
            }
            if assessment["dimensions"]["uav"]["status"].as_str() == Some("watch") {
                abnormal += 1;
            }
            if assessment["missing_evidence"].as_array().map(|v| !v.is_empty()).unwrap_or(false) {
                missing_evidence += 1;
            }
            if assessment["completeness"].as_str() == Some("complete") {
                complete += 1;
            }
            if let Some(action) = assessment["recommended_action"].as_str() {
                if action != "wait" {
                    priority.push(json!({
                        "tree_code": tree_code,
                        "recommended_action": action,
                        "summary": assessment["summary"]
                    }));
                }
            }
        }

        trees.push(json!({
            "tree_code": tree_code,
            "block_id": assessment["tree"]["block_id"],
            "current_status": assessment["tree"]["current_status"],
            "completeness": assessment["completeness"],
            "recommended_action": dashboard_action,
            "summary": assessment["summary"],
            "valid_until": assessment["valid_until"]
        }));
    }

    Ok(json!({
        "plantation_id": plantation_id,
        "generated_at": Utc::now().to_rfc3339(),
        "stats": {
            "total_trees": trees.len(),
            "active_trees": active,
            "complete_assessments": complete,
            "harvest_recommended": harvest_recommended,
            "disease_risk": disease_risk,
            "abnormal_or_shift_watch": abnormal,
            "missing_evidence": missing_evidence
        },
        "priority_trees": priority.into_iter().take(50).collect::<Vec<_>>(),
        "trees": trees
    }))
}

pub(crate) fn build_blocks_report(g: &mut DbManager, plantation_id: i32) -> Result<Value, String> {
    let dashboard = build_plantation_dashboard(g, plantation_id)?;
    let mut blocks: BTreeMap<String, Value> = BTreeMap::new();
    for tree in dashboard["trees"].as_array().cloned().unwrap_or_default() {
        let block = tree["block_id"].as_str().unwrap_or("unassigned").to_string();
        let entry = blocks.entry(block.clone()).or_insert_with(|| json!({
            "block_id": block,
            "total_trees": 0,
            "harvest_recommended": 0,
            "disease_risk": 0,
            "missing_evidence": 0,
            "trees": []
        }));
        entry["total_trees"] = json!(entry["total_trees"].as_i64().unwrap_or(0) + 1);
        if tree["recommended_action"].as_str() == Some("harvest") {
            entry["harvest_recommended"] = json!(entry["harvest_recommended"].as_i64().unwrap_or(0) + 1);
        }
        if tree["recommended_action"].as_str() == Some("inspect_disease") {
            entry["disease_risk"] = json!(entry["disease_risk"].as_i64().unwrap_or(0) + 1);
        }
        if tree["completeness"].as_str() == Some("partial") {
            entry["missing_evidence"] = json!(entry["missing_evidence"].as_i64().unwrap_or(0) + 1);
        }
        entry["trees"].as_array_mut().unwrap().push(tree);
    }

    Ok(json!({
        "plantation_id": plantation_id,
        "generated_at": Utc::now().to_rfc3339(),
        "blocks": blocks.into_values().collect::<Vec<_>>()
    }))
}

fn build_tree_assessment(tree: Value, images: Vec<Value>, history: Vec<Value>) -> Value {
    let fruit = dimension_from_latest_image(&images, "fruit");
    let disease = dimension_from_latest_image(&images, "trunk_base");
    let growth = dimension_from_latest_image(&images, "crown");
    let uav = dimension_from_latest_uav(&history);

    let mut missing = Vec::new();
    for (name, dim) in [("fruit", &fruit), ("disease", &disease), ("growth", &growth), ("uav", &uav)] {
        if dim["evidence_status"].as_str() != Some("present") {
            missing.push(json!(name));
        }
    }
    let completeness = if missing.is_empty() { "complete" } else { "partial" };
    let last_evidence_at = latest_timestamp(&images, &history).or_else(|| tree["updated_at"].as_str().map(|s| s.to_string()));
    let valid_until = last_evidence_at
        .as_deref()
        .and_then(parse_rfc3339)
        .map(|dt| (dt + Duration::days(30)).to_rfc3339());
    let recommended_action = recommended_action(&fruit, &disease, &growth, &uav, &missing);
    let summary = summary_for_action(recommended_action, &missing);

    json!({
        "tree": {
            "id": tree["id"],
            "tree_code": tree["tree_code"],
            "plantation_name": tree["plantation_name"],
            "block_id": tree["block_id"],
            "current_status": tree["current_status"]
        },
        "completeness": completeness,
        "valid_until": valid_until,
        "last_evidence_at": last_evidence_at,
        "missing_evidence": missing,
        "recommended_action": recommended_action,
        "summary": summary,
        "dimensions": {
            "fruit": fruit,
            "disease": disease,
            "growth": growth,
            "uav": uav
        },
        "metadata": {
            "mock": true,
            "assessment_version": "tree_assessment_mock_v1"
        }
    })
}

fn dimension_from_latest_image(images: &[Value], role: &str) -> Value {
    let latest = images.iter()
        .filter(|img| img["image_role"].as_str() == Some(role))
        .max_by_key(|img| img["created_at"].as_str().unwrap_or("").to_string());
    let Some(img) = latest else {
        return json!({"evidence_status":"missing","status":"unknown","label":null,"confidence":null});
    };
    let analysis = &img["mock_analysis"];
    let first = analysis["results"].as_array().and_then(|items| items.first()).cloned().unwrap_or_else(|| json!({}));
    let label = first["label"].as_str().unwrap_or("unknown");
    let confidence = first["confidence"].as_f64();

    match role {
        "fruit" => json!({
            "evidence_status": "present",
            "status": if label == "ripe" || label == "overripe" { "harvest_ready" } else { "monitor" },
            "label": label,
            "confidence": confidence,
            "evidence_at": img["created_at"],
            "image_id": img["id"],
            "image_url": img["image_url"],
            "advice": analysis["metadata"]["advice"]
        }),
        "trunk_base" => {
            let risk = disease_risk_level(label);
            json!({
                "evidence_status": "present",
                "status": if risk == "low" { "monitor" } else { "risk_watch" },
                "risk_level": risk,
                "label": label,
                "confidence": confidence,
                "evidence_at": img["created_at"],
                "image_id": img["id"],
                "image_url": img["image_url"],
                "risk_language": "suspected_not_confirmed",
                "advice": analysis["metadata"]["advice"]
            })
        }
        _ => {
            let vigor = analysis["metadata"]["vigor_index"].as_f64()
                .or_else(|| first["geometry"]["coverage"].as_f64());
            json!({
                "evidence_status": "present",
                "status": if vigor.unwrap_or(0.0) < 0.5 { "weak" } else { "acceptable" },
                "label": label,
                "confidence": confidence,
                "vigor_index": vigor,
                "evidence_at": img["created_at"],
                "image_id": img["id"],
                "image_url": img["image_url"],
                "advice": analysis["metadata"]["advice"]
            })
        }
    }
}

fn dimension_from_latest_uav(history: &[Value]) -> Value {
    let latest = history.iter().max_by_key(|item| item["created_at"].as_str().unwrap_or("").to_string());
    let Some(item) = latest else {
        return json!({"evidence_status":"missing","status":"unknown","center_shift":null,"confidence":null});
    };
    let shift = item["center_shift"].as_f64();
    json!({
        "evidence_status": "present",
        "status": if shift.unwrap_or(0.0) > 1.5 { "watch" } else { "stable" },
        "center_shift": shift,
        "confidence": item["match_confidence"],
        "evidence_at": item["created_at"],
        "mission_id": item["mission_id"],
        "mission_name": item["mission_name"]
    })
}

fn disease_risk_level(label: &str) -> &'static str {
    if label.contains("severe") || label.contains("moderate") {
        "high"
    } else if label.contains("suspected") || label.contains("risk") {
        "medium"
    } else {
        "low"
    }
}

fn recommended_action(fruit: &Value, disease: &Value, growth: &Value, uav: &Value, missing: &[Value]) -> &'static str {
    if ["medium", "high"].contains(&disease["risk_level"].as_str().unwrap_or("")) {
        "inspect_disease"
    } else if fruit["status"].as_str() == Some("harvest_ready") {
        "harvest"
    } else if growth["status"].as_str() == Some("weak") || uav["status"].as_str() == Some("watch") {
        "recheck"
    } else if !missing.is_empty() {
        "collect_more_evidence"
    } else {
        "wait"
    }
}

fn summary_for_action(action: &str, missing: &[Value]) -> String {
    match action {
        "inspect_disease" => "Disease risk evidence is present; schedule field inspection before making a diagnosis.".to_string(),
        "harvest" => "Fruit evidence suggests harvest readiness; verify bunch condition before harvest.".to_string(),
        "recheck" => "Growth or UAV evidence needs follow-up monitoring.".to_string(),
        "collect_more_evidence" => format!("Assessment is partial; missing evidence: {}.", missing.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")),
        _ => "All available mock evidence is stable; continue routine monitoring.".to_string(),
    }
}

fn latest_timestamp(images: &[Value], history: &[Value]) -> Option<String> {
    images.iter()
        .filter_map(|v| v["created_at"].as_str())
        .chain(history.iter().filter_map(|v| v["created_at"].as_str()))
        .max()
        .map(|s| s.to_string())
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).ok().map(|dt| dt.with_timezone(&Utc))
}
