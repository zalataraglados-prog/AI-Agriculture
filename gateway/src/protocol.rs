use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RegisterPayload {
    pub device_id: String,
    pub location: String,
    pub crop_type: String,
    pub farm_note: String,
    pub sensors: Vec<String>,
    pub feature_mapping: BTreeMap<String, String>,
    pub token: String,
}

pub fn build_register_packet(payload: &RegisterPayload) -> Result<String, String> {
    let json = serde_json::to_string(payload)
        .map_err(|e| format!("Failed to serialize register payload: {e}"))?;
    Ok(format!("register:{json}"))
}

pub fn build_sensor_packet(
    sensor_id: &str,
    fields: &BTreeMap<String, String>,
    device_id: &str,
) -> String {
    let mut pairs = Vec::new();
    pairs.push(format!("device_id={device_id}"));

    for (key, value) in fields {
        if key == "device_id" {
            continue;
        }
        pairs.push(format!("{key}={value}"));
    }

    format!("{sensor_id}:{}", pairs.join(","))
}

pub fn build_image_channel_packet(
    fields: &BTreeMap<String, String>,
    device_id: &str,
) -> String {
    let mut pairs = Vec::new();
    pairs.push(format!("device_id={device_id}"));
    pairs.push("channel=image".to_string());
    pairs.push("status=reserved".to_string());

    for (key, value) in fields {
        if key == "device_id" {
            continue;
        }
        pairs.push(format!("{key}={value}"));
    }

    format!("image:{}", pairs.join(","))
}
