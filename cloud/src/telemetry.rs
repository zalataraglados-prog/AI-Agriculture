use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{FieldType, RuntimeConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TelemetryRecord {
    pub(crate) ts: String,
    pub(crate) device_id: String,
    pub(crate) sensor_id: String,
    pub(crate) fields: HashMap<String, Value>,
}

pub(crate) fn append_record(path: &str, record: &TelemetryRecord) -> Result<(), String> {
    ensure_parent_dir(path)?;

    let line = serde_json::to_string(record)
        .map_err(|e| format!("Failed to serialize telemetry record: {e}"))?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Failed to open telemetry store {}: {e}", path))?;

    file.write_all(line.as_bytes())
        .map_err(|e| format!("Failed to write telemetry record: {e}"))?;
    file.write_all(b"\n")
        .map_err(|e| format!("Failed to finalize telemetry record: {e}"))?;

    Ok(())
}

pub(crate) fn load_records(path: &str) -> Result<Vec<TelemetryRecord>, String> {
    if !Path::new(path).exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read telemetry store {}: {e}", path))?;

    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<TelemetryRecord>(trimmed) {
            out.push(record);
        }
    }

    Ok(out)
}

pub(crate) fn typed_fields_for_record(
    sensor_id: &str,
    raw_fields: &HashMap<String, String>,
    cfg: &RuntimeConfig,
) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    let type_map = cfg
        .sensor_rules
        .get(sensor_id)
        .map(|rule| &rule.field_types);

    for (key, raw) in raw_fields {
        if key == "device_id" || key == "token" {
            continue;
        }
        let value = type_map
            .and_then(|fields| fields.get(key))
            .map(|field_type| parse_typed_value(raw, *field_type))
            .unwrap_or_else(|| Value::String(raw.clone()));
        out.insert(key.clone(), value);
    }

    out
}

fn parse_typed_value(raw: &str, field_type: FieldType) -> Value {
    match field_type {
        FieldType::String => Value::String(raw.to_string()),
        FieldType::Bool => raw
            .parse::<bool>()
            .map(Value::Bool)
            .unwrap_or_else(|_| Value::String(raw.to_string())),
        FieldType::U8 => to_json_number(raw.parse::<u8>().map(|v| v as u64), raw),
        FieldType::U16 => to_json_number(raw.parse::<u16>().map(|v| v as u64), raw),
        FieldType::U32 => to_json_number(raw.parse::<u32>().map(|v| v as u64), raw),
        FieldType::I32 => to_json_number(raw.parse::<i32>().map(|v| v as i64), raw),
        FieldType::F32 => to_json_float(raw.parse::<f64>().ok(), raw),
        FieldType::F64 => to_json_float(raw.parse::<f64>().ok(), raw),
    }
}

fn to_json_number<T>(parsed: Result<T, std::num::ParseIntError>, raw: &str) -> Value
where
    serde_json::Number: From<T>,
{
    parsed
        .map(|v| Value::Number(serde_json::Number::from(v)))
        .unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn to_json_float(parsed: Option<f64>, raw: &str) -> Value {
    parsed
        .filter(|v| v.is_finite())
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .unwrap_or_else(|| Value::String(raw.to_string()))
}

fn ensure_parent_dir(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create telemetry dir {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}
