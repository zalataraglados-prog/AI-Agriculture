use std::path::Path;
use std::time::Instant;

use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct AiInferenceOutput {
    pub(crate) predicted_class: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) model_version: Option<String>,
    pub(crate) topk_json: Value,
    pub(crate) metadata_json: Value,
    pub(crate) geometry_json: Option<Value>,
    pub(crate) latency_ms: Option<i32>,
    pub(crate) advice_code: Option<String>,
}

pub(crate) fn infer_image_from_file(
    predict_url: &str,
    image_path: &str,
    image_type: &str,
) -> Result<AiInferenceOutput, String> {
    let bytes = std::fs::read(image_path)
        .map_err(|e| format!("failed to read image file {}: {e}", image_path))?;
    let filename = Path::new(image_path)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("upload.bin")
        .to_string();
    let content_type = match image_type {
        "png" => "image/png",
        _ => "image/jpeg",
    };

    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(filename)
        .mime_str(content_type)
        .map_err(|e| format!("failed to build multipart file part: {e}"))?;
    let form = reqwest::blocking::multipart::Form::new().part("file", part);

    let started = Instant::now();
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("failed to create AI HTTP client: {e}"))?
        .post(predict_url)
        .multipart(form)
        .send()
        .map_err(|e| format!("failed to call AI predict API {}: {e}", predict_url))?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|e| format!("failed to read AI response body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "AI predict API returned {} with body: {}",
            status,
            trim_for_log(&text)
        ));
    }

    parse_ai_response(&text, started.elapsed().as_millis() as i32)
}

fn parse_ai_response(text: &str, elapsed_ms: i32) -> Result<AiInferenceOutput, String> {
    let json: Value = serde_json::from_str(text).map_err(|e| {
        format!(
            "failed to parse AI JSON response: {e}; body={}",
            trim_for_log(text)
        )
    })?;

    if json
        .get("status")
        .and_then(|v| v.as_str())
        .map(|v| v == "error")
        .unwrap_or(false)
    {
        let message = json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown AI error");
        return Err(format!("AI inference status=error: {message}"));
    }

    let first = json
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| "AI response missing results[0]".to_string())?;

    let predicted_class = first
        .get("predicted_class")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let confidence = first.get("confidence").and_then(|v| v.as_f64());
    let model_version = first
        .get("model_version")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let topk_json = first
        .get("topk")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let mut metadata_json = first
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if !metadata_json.is_object() {
        metadata_json = Value::Object(serde_json::Map::new());
    }
    let geometry_json = first.get("geometry").cloned().filter(|v| !v.is_null());

    enrich_disease_metrics(&predicted_class, confidence, &topk_json, &mut metadata_json);

    let advice_code = metadata_json
        .get("advice_code")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    Ok(AiInferenceOutput {
        predicted_class,
        confidence,
        model_version,
        topk_json,
        metadata_json,
        geometry_json,
        latency_ms: Some(elapsed_ms),
        advice_code,
    })
}

fn enrich_disease_metrics(
    predicted_class: &Option<String>,
    confidence: Option<f64>,
    topk_json: &Value,
    metadata_json: &mut Value,
) {
    let Some(obj) = metadata_json.as_object_mut() else {
        return;
    };

    let healthy_prob_existing = obj.get("healthy_prob").and_then(|v| v.as_f64());
    let healthy_prob_topk = extract_healthy_prob_from_topk(topk_json);
    let healthy_prob = healthy_prob_existing.or(healthy_prob_topk).or_else(|| {
        match (predicted_class.as_deref(), confidence) {
            (Some("HealthyLeaf"), Some(c)) => Some(c.clamp(0.0, 1.0)),
            _ => None,
        }
    });

    if let Some(v) = healthy_prob {
        obj.insert("healthy_prob".to_string(), Value::from(v.clamp(0.0, 1.0)));
    }

    let disease_rate_existing = obj.get("disease_rate").and_then(|v| v.as_f64());
    let disease_rate = disease_rate_existing
        .or_else(|| healthy_prob.map(|p| (1.0 - p).clamp(0.0, 1.0)))
        .or_else(|| match (predicted_class.as_deref(), confidence) {
            (Some("HealthyLeaf"), Some(c)) => Some((1.0 - c).clamp(0.0, 1.0)),
            (Some(_), Some(c)) => Some(c.clamp(0.0, 1.0)),
            _ => None,
        });
    if let Some(v) = disease_rate {
        obj.insert("disease_rate".to_string(), Value::from(v.clamp(0.0, 1.0)));
    }
}

fn extract_healthy_prob_from_topk(topk_json: &Value) -> Option<f64> {
    topk_json
        .as_array()
        .and_then(|items| {
            items.iter().find_map(|item| {
                let label = item.get("label").and_then(|v| v.as_str())?;
                if label != "HealthyLeaf" {
                    return None;
                }
                item.get("score").and_then(|v| v.as_f64())
            })
        })
        .map(|v| v.clamp(0.0, 1.0))
}

fn trim_for_log(text: &str) -> String {
    const LIMIT: usize = 300;
    if text.len() <= LIMIT {
        text.to_string()
    } else {
        format!("{}...(truncated)", &text[..LIMIT])
    }
}
