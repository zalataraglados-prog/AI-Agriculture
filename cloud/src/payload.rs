use std::collections::HashMap;

use crate::model::{EvalResult, FieldType, RuntimeConfig};

pub(crate) fn parse_sensor_kv_payload(payload: &str) -> Result<(String, HashMap<String, String>), String> {
    let (sensor_id, kv_text) = payload
        .split_once(':')
        .ok_or_else(|| "missing ':' separator".to_string())?;

    let sensor_id = sensor_id.trim();
    if sensor_id.is_empty() {
        return Err("sensor id is empty".to_string());
    }

    let mut fields = HashMap::new();
    for pair in kv_text.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("invalid field format: {pair}"))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return Err("field key is empty".to_string());
        }
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(format!("duplicate field: {key}"));
        }
    }

    if fields.is_empty() {
        return Err("no fields found".to_string());
    }

    Ok((sensor_id.to_string(), fields))
}

fn validate_field_type(value: &str, field_type: FieldType) -> bool {
    match field_type {
        FieldType::String => true,
        FieldType::Bool => value.parse::<bool>().is_ok(),
        FieldType::U8 => value.parse::<u8>().is_ok(),
        FieldType::U16 => value.parse::<u16>().is_ok(),
        FieldType::U32 => value.parse::<u32>().is_ok(),
        FieldType::I32 => value.parse::<i32>().is_ok(),
        FieldType::F32 => value.parse::<f32>().map(|v| v.is_finite()).unwrap_or(false),
        FieldType::F64 => value.parse::<f64>().map(|v| v.is_finite()).unwrap_or(false),
    }
}

pub(crate) fn evaluate_payload(payload: &str, cfg: &RuntimeConfig) -> EvalResult {
    if let Some(ack) = cfg.exact_rules.get(payload) {
        return EvalResult {
            matched: true,
            ack: ack.clone(),
            detail: "matched exact payload rule".to_string(),
        };
    }

    let (sensor_id, fields) = match parse_sensor_kv_payload(payload) {
        Ok(v) => v,
        Err(err) => {
            return EvalResult {
                matched: false,
                ack: cfg.ack_mismatch.clone(),
                detail: format!("invalid payload format: {err}"),
            };
        }
    };

    let rule = match cfg.sensor_rules.get(&sensor_id) {
        Some(rule) => rule,
        None => {
            return EvalResult {
                matched: false,
                ack: cfg.ack_unknown_sensor.clone(),
                detail: format!("unknown sensor id: {sensor_id}"),
            };
        }
    };

    for required in &rule.required_fields {
        if !fields.contains_key(required) {
            return EvalResult {
                matched: false,
                ack: cfg.ack_mismatch.clone(),
                detail: format!("sensor {sensor_id} missing required field: {required}"),
            };
        }
    }

    for (field, field_type) in &rule.field_types {
        if let Some(value) = fields.get(field) {
            if !validate_field_type(value, *field_type) {
                return EvalResult {
                    matched: false,
                    ack: cfg.ack_mismatch.clone(),
                    detail: format!(
                        "sensor {sensor_id} field type mismatch: {field}={value} does not match {field_type:?}"
                    ),
                };
            }
        }
    }

    EvalResult {
        matched: true,
        ack: rule.ack.clone(),
        detail: format!("matched sensor rule: {sensor_id}"),
    }
}
