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
    let metadata_json = first
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let geometry_json = first.get("geometry").cloned().filter(|v| !v.is_null());

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

fn trim_for_log(text: &str) -> String {
    const LIMIT: usize = 300;
    if text.len() <= LIMIT {
        text.to_string()
    } else {
        format!("{}...(truncated)", &text[..LIMIT])
    }
}
