use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde_json::{json, Value};
use tiny_http::{Method, Request, Response};

use crate::assessment::{build_blocks_report, build_plantation_dashboard, build_tree_assessment_by_code};
use crate::db::DbManager;

const MAX_TOOL_LIMIT: usize = 50;

pub(crate) fn handle_tool_request(
    mut request: Request,
    method: Method,
    path: &str,
    query: &str,
    db: Arc<Mutex<DbManager>>,
) {
    let result = match (method, path) {
        (Method::Get, "/api/v1/openclaw/tools/manifest") => Ok(tool_manifest()),
        (Method::Get, "/api/v1/openclaw/tools/tree-profile") => handle_tree_profile(query, db),
        (Method::Get, "/api/v1/openclaw/tools/tree-timeline") => handle_tree_timeline(query, db),
        (Method::Get, "/api/v1/openclaw/tools/plantation-report") => handle_plantation_report(query, db),
        (Method::Get, "/api/v1/openclaw/tools/missing-evidence") => handle_missing_evidence(query, db),
        (Method::Get, "/api/v1/openclaw/tools/patrol-report") => handle_patrol_report(query, db),
        (Method::Post, "/api/v1/openclaw/tools/explain-prediction") => {
            let mut body = Vec::new();
            if let Err(err) = request.as_reader().read_to_end(&mut body) {
                Err(tool_error(400, format!("failed to read request body: {err}")))
            } else {
                handle_explain_prediction(&body)
            }
        }
        (Method::Get, "/api/v1/openclaw/tools/explain-prediction") => {
            Err(tool_error(405, "use POST for explain-prediction".to_string()))
        }
        _ => Err(tool_error(404, "openclaw tool not found".to_string())),
    };

    match result {
        Ok(payload) => respond_json(request, 200, &payload),
        Err(err) => {
            let status = err["http_status"].as_u64().unwrap_or(500) as u16;
            respond_json(request, status, &err);
        }
    }
}

fn handle_tree_profile(query: &str, db: Arc<Mutex<DbManager>>) -> Result<Value, Value> {
    let params = parse_query_params(query);
    let tree_code = required_param(&params, "tree_code")?;
    let limit = limit_param(&params, 10);

    let result = db
        .lock()
        .map_err(|_| tool_error(500, "db lock failed".to_string()))
        .and_then(|mut g| {
            let Some(tree) = g.get_tree_by_code(&tree_code).map_err(internal_error)? else {
                return Err(tool_error(404, "tree not found".to_string()));
            };
            let assessment = build_tree_assessment_by_code(&mut g, &tree_code).map_err(internal_error)?;
            let timeline = g.get_tree_timeline(&tree_code).map_err(internal_error)?;
            Ok(json!({
                "status": "ok",
                "tool": "query_tree_profile",
                "tree": tree,
                "assessment": assessment,
                "timeline": take_limit(timeline, limit),
                "metadata": tool_metadata("cloud_db")
            }))
        });

    result
}

fn handle_tree_timeline(query: &str, db: Arc<Mutex<DbManager>>) -> Result<Value, Value> {
    let params = parse_query_params(query);
    let tree_code = required_param(&params, "tree_code")?;
    let limit = limit_param(&params, 20);

    db.lock()
        .map_err(|_| tool_error(500, "db lock failed".to_string()))
        .and_then(|mut g| {
            if g.get_tree_by_code(&tree_code).map_err(internal_error)?.is_none() {
                return Err(tool_error(404, "tree not found".to_string()));
            }
            let timeline = g.get_tree_timeline(&tree_code).map_err(internal_error)?;
            Ok(json!({
                "status": "ok",
                "tool": "query_tree_timeline",
                "tree_code": tree_code,
                "limit": limit,
                "timeline": take_limit(timeline, limit),
                "metadata": tool_metadata("cloud_db")
            }))
        })
}

fn handle_plantation_report(query: &str, db: Arc<Mutex<DbManager>>) -> Result<Value, Value> {
    let params = parse_query_params(query);
    let plantation_id = required_i32_param(&params, "plantation_id")?;
    let limit = limit_param(&params, 50);

    db.lock()
        .map_err(|_| tool_error(500, "db lock failed".to_string()))
        .and_then(|mut g| {
            let mut dashboard = build_plantation_dashboard(&mut g, plantation_id).map_err(internal_error)?;
            limit_array_field(&mut dashboard, "trees", limit);
            limit_array_field(&mut dashboard, "priority_trees", limit);
            let mut blocks = build_blocks_report(&mut g, plantation_id).map_err(internal_error)?;
            limit_block_trees(&mut blocks, limit);
            Ok(json!({
                "status": "ok",
                "tool": "query_plantation_report",
                "plantation_id": plantation_id,
                "dashboard": dashboard,
                "blocks_report": blocks,
                "metadata": tool_metadata("cloud_db")
            }))
        })
}

fn handle_missing_evidence(query: &str, db: Arc<Mutex<DbManager>>) -> Result<Value, Value> {
    let params = parse_query_params(query);
    let tree_code = required_param(&params, "tree_code")?;

    db.lock()
        .map_err(|_| tool_error(500, "db lock failed".to_string()))
        .and_then(|mut g| {
            let Some(assessment) = build_tree_assessment_by_code(&mut g, &tree_code).map_err(internal_error)? else {
                return Err(tool_error(404, "tree not found".to_string()));
            };
            let missing = assessment["missing_evidence"].as_array().cloned().unwrap_or_default();
            let recommendations = missing
                .iter()
                .filter_map(|item| item.as_str())
                .map(evidence_recommendation)
                .collect::<Vec<_>>();
            Ok(json!({
                "status": "ok",
                "tool": "query_missing_evidence",
                "tree_code": tree_code,
                "missing_evidence": missing,
                "recommendations": recommendations,
                "assessment_summary": assessment["summary"],
                "metadata": tool_metadata("cloud_db")
            }))
        })
}

fn handle_patrol_report(query: &str, db: Arc<Mutex<DbManager>>) -> Result<Value, Value> {
    let params = parse_query_params(query);
    let plantation_id = required_i32_param(&params, "plantation_id")?;
    let limit = limit_param(&params, 50);

    db.lock()
        .map_err(|_| tool_error(500, "db lock failed".to_string()))
        .and_then(|mut g| {
            let dashboard = build_plantation_dashboard(&mut g, plantation_id).map_err(internal_error)?;
            let mut items = dashboard["trees"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(patrol_item_from_tree)
                .collect::<Vec<_>>();
            items.sort_by_key(|item| priority_rank(item["priority"].as_str().unwrap_or("low")));
            Ok(json!({
                "status": "ok",
                "tool": "generate_patrol_report",
                "plantation_id": plantation_id,
                "generated_at": Utc::now().to_rfc3339(),
                "limit": limit,
                "items": take_limit(items, limit),
                "metadata": tool_metadata("cloud_db")
            }))
        })
}

fn handle_explain_prediction(body: &[u8]) -> Result<Value, Value> {
    let parsed: Value = serde_json::from_slice(body)
        .map_err(|err| tool_error(400, format!("invalid json body: {err}")))?;
    let prediction = parsed
        .get("result_json")
        .or_else(|| parsed.get("prediction"))
        .unwrap_or(&parsed);

    let label = first_label(prediction).unwrap_or_else(|| "unknown".to_string());
    let image_role = prediction["metadata"]["image_role"]
        .as_str()
        .or_else(|| parsed["image_role"].as_str())
        .unwrap_or("unknown");
    let confidence = first_confidence(prediction);
    let explanation = explanation_for(image_role, &label, confidence);

    Ok(json!({
        "status": "ok",
        "tool": "explain_prediction",
        "explanation": explanation,
        "metadata": {
            "read_only": true,
            "deterministic": true,
            "mock_safe": true,
            "generated_at": Utc::now().to_rfc3339()
        }
    }))
}

fn tool_manifest() -> Value {
    json!({
        "status": "ok",
        "base_path": "/api/v1/openclaw/tools",
        "read_only": true,
        "max_limit": MAX_TOOL_LIMIT,
        "tools": [
            {"name":"query_tree_profile","method":"GET","path":"/tree-profile","params":["tree_code","limit?"],"description":"Fetch tree profile, assessment, and recent timeline evidence."},
            {"name":"query_tree_timeline","method":"GET","path":"/tree-timeline","params":["tree_code","limit?"],"description":"Fetch coordinate history timeline for one tree."},
            {"name":"query_plantation_report","method":"GET","path":"/plantation-report","params":["plantation_id","limit?"],"description":"Fetch plantation dashboard and block report summaries."},
            {"name":"query_missing_evidence","method":"GET","path":"/missing-evidence","params":["tree_code"],"description":"List missing evidence dimensions and collection recommendations."},
            {"name":"explain_prediction","method":"POST","path":"/explain-prediction","params":["result_json"],"description":"Deterministically explain a mock prediction without claiming diagnosis."},
            {"name":"generate_patrol_report","method":"GET","path":"/patrol-report","params":["plantation_id","limit?"],"description":"List priority trees for field patrol."}
        ],
        "safety": {
            "writes_database": false,
            "runs_models": false,
            "diagnosis_policy": "suspected_not_confirmed",
            "source_of_truth": "cloud structured APIs and database"
        }
    })
}

fn parse_query_params(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

fn required_param(params: &HashMap<String, String>, key: &str) -> Result<String, Value> {
    params
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| tool_error(400, format!("{key} is required")))
}

fn required_i32_param(params: &HashMap<String, String>, key: &str) -> Result<i32, Value> {
    let raw = required_param(params, key)?;
    raw.parse::<i32>()
        .map_err(|_| tool_error(400, format!("{key} must be an integer")))
}

fn limit_param(params: &HashMap<String, String>, default_value: usize) -> usize {
    params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default_value)
        .clamp(1, MAX_TOOL_LIMIT)
}

fn take_limit(items: Vec<Value>, limit: usize) -> Vec<Value> {
    items.into_iter().take(limit).collect()
}

fn limit_array_field(value: &mut Value, field: &str, limit: usize) {
    if let Some(items) = value[field].as_array_mut() {
        items.truncate(limit);
    }
}

fn limit_block_trees(value: &mut Value, limit: usize) {
    for block in value["blocks"].as_array_mut().into_iter().flatten() {
        if let Some(items) = block["trees"].as_array_mut() {
            items.truncate(limit);
        }
    }
}

fn evidence_recommendation(name: &str) -> Value {
    match name {
        "fruit" => json!({"evidence":"fruit","action":"capture_fruit_image","image_role":"fruit"}),
        "disease" => json!({"evidence":"disease","action":"capture_trunk_base_image","image_role":"trunk_base","risk_language":"suspected_not_confirmed"}),
        "growth" => json!({"evidence":"growth","action":"capture_crown_image","image_role":"crown"}),
        "uav" => json!({"evidence":"uav","action":"run_or_match_uav_mission","image_role":"uav"}),
        _ => json!({"evidence":name,"action":"collect_more_evidence"}),
    }
}

fn patrol_item_from_tree(tree: Value) -> Option<Value> {
    let action = tree["recommended_action"].as_str().unwrap_or("wait");
    if action == "wait" || action == "inactive" {
        return None;
    }
    let priority = match action {
        "inspect_disease" => "high",
        "harvest" => "high",
        "recheck" => "medium",
        "collect_more_evidence" => "medium",
        _ => "low",
    };
    Some(json!({
        "tree_code": tree["tree_code"],
        "block_id": tree["block_id"],
        "priority": priority,
        "recommended_action": action,
        "reason": tree["summary"],
        "valid_until": tree["valid_until"]
    }))
}

fn priority_rank(priority: &str) -> u8 {
    match priority {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    }
}

fn first_label(value: &Value) -> Option<String> {
    value["results"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["label"].as_str())
        .or_else(|| value["label"].as_str())
        .map(|s| s.to_string())
}

fn first_confidence(value: &Value) -> Option<f64> {
    value["results"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["confidence"].as_f64())
        .or_else(|| value["confidence"].as_f64())
}

fn explanation_for(image_role: &str, label: &str, confidence: Option<f64>) -> Value {
    let conf_text = confidence
        .map(|v| format!("{:.2}", v))
        .unwrap_or_else(|| "unknown".to_string());
    match image_role {
        "trunk_base" => json!({
            "summary": format!("The trunk-base mock result is '{label}' with confidence {conf_text}. Treat this as a suspected risk signal, not a confirmed diagnosis."),
            "risk_language": "suspected_not_confirmed",
            "next_action": "schedule_field_inspection",
            "evidence": [{"image_role":"trunk_base","label":label,"confidence":confidence}]
        }),
        "fruit" => json!({
            "summary": format!("The fruit mock result is '{label}' with confidence {conf_text}. Use it as harvest-readiness evidence only after field verification."),
            "risk_language": "not_applicable",
            "next_action": if label == "ripe" || label == "overripe" { "verify_and_harvest" } else { "monitor" },
            "evidence": [{"image_role":"fruit","label":label,"confidence":confidence}]
        }),
        "crown" => json!({
            "summary": format!("The crown mock result is '{label}' with confidence {conf_text}. Use it as growth/vigor evidence, not as a full tree diagnosis."),
            "risk_language": "not_applicable",
            "next_action": "monitor_growth",
            "evidence": [{"image_role":"crown","label":label,"confidence":confidence}]
        }),
        _ => json!({
            "summary": format!("The mock result is '{label}' with confidence {conf_text}. Interpret it as structured evidence for the tree record."),
            "risk_language": "suspected_not_confirmed",
            "next_action": "review_evidence",
            "evidence": [{"image_role":image_role,"label":label,"confidence":confidence}]
        }),
    }
}

fn tool_metadata(source: &str) -> Value {
    json!({
        "read_only": true,
        "source": source,
        "mock_assessment": true,
        "generated_at": Utc::now().to_rfc3339()
    })
}

fn internal_error(err: String) -> Value {
    tool_error(500, err)
}

fn tool_error(status: u16, message: String) -> Value {
    json!({
        "status": "error",
        "http_status": status,
        "message": message
    })
}

fn respond_json(request: Request, status: u16, body: &Value) {
    let response = Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    let _ = request.respond(response);
}
