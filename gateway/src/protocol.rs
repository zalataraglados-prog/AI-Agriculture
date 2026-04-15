use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ParsedSensorEvent {
    pub sensor_id: String,
    pub feature: String,
    pub fields: BTreeMap<String, String>,
}

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
    // Placeholder channel for future image transport migration.
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

type TextParser = fn(&str) -> Option<ParsedSensorEvent>;

pub fn parse_known_event(raw_line: &str) -> Option<ParsedSensorEvent> {
    const PARSERS: [TextParser; 5] = [
        parse_image_channel_line,
        parse_mq7_line,
        parse_dht22_line,
        parse_adc_line,
        parse_pcf8591_line,
    ];

    for parser in PARSERS {
        if let Some(event) = parser(raw_line) {
            return Some(event);
        }
    }

    None
}

fn parse_image_channel_line(line: &str) -> Option<ParsedSensorEvent> {
    let feature = extract_feature(line)?;
    if feature != "image" && feature != "img" && feature != "frame" {
        return None;
    }

    let mut fields = parse_generic_fields(line);
    if fields.is_empty() {
        fields.insert("raw_text".to_string(), line.replace(',', " "));
    }

    Some(ParsedSensorEvent {
        sensor_id: "image".to_string(),
        feature: "image".to_string(),
        fields,
    })
}

fn parse_mq7_line(line: &str) -> Option<ParsedSensorEvent> {
    let mut raw: Option<u16> = None;
    let mut voltage: Option<f32> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("raw=") {
            raw = value.parse::<u16>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("voltage=") {
            let stripped = value.strip_suffix('V').unwrap_or(value);
            voltage = stripped.parse::<f32>().ok();
        }
    }

    let (Some(raw), Some(voltage)) = (raw, voltage) else {
        return None;
    };

    let mut fields = BTreeMap::new();
    fields.insert("raw".to_string(), raw.to_string());
    fields.insert("voltage".to_string(), format!("{:.3}", voltage));
    Some(ParsedSensorEvent {
        sensor_id: "mq7".to_string(),
        feature: "mq7".to_string(),
        fields,
    })
}

fn parse_dht22_line(line: &str) -> Option<ParsedSensorEvent> {
    let mut temp_c: Option<f32> = None;
    let mut hum: Option<f32> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("temp_c=") {
            temp_c = value.parse::<f32>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("hum=") {
            let stripped = value.strip_suffix('%').unwrap_or(value);
            hum = stripped.parse::<f32>().ok();
        }
    }

    let (Some(temp_c), Some(hum)) = (temp_c, hum) else {
        return None;
    };

    let mut fields = BTreeMap::new();
    fields.insert("temp_c".to_string(), format!("{:.1}", temp_c));
    fields.insert("hum".to_string(), format!("{:.1}", hum));
    Some(ParsedSensorEvent {
        sensor_id: "dht22".to_string(),
        feature: "dht22".to_string(),
        fields,
    })
}

fn parse_adc_line(line: &str) -> Option<ParsedSensorEvent> {
    let mut pin: Option<u8> = None;
    let mut raw: Option<u16> = None;
    let mut voltage: Option<f32> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("pin=") {
            pin = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("raw=") {
            raw = value.parse::<u16>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("voltage=") {
            let stripped = value.strip_suffix('V').unwrap_or(value);
            voltage = stripped.parse::<f32>().ok();
        }
    }

    let (Some(pin), Some(raw), Some(voltage)) = (pin, raw, voltage) else {
        return None;
    };

    let mut fields = BTreeMap::new();
    fields.insert("pin".to_string(), pin.to_string());
    fields.insert("raw".to_string(), raw.to_string());
    fields.insert("voltage".to_string(), format!("{:.3}", voltage));
    Some(ParsedSensorEvent {
        sensor_id: "adc".to_string(),
        feature: "adc".to_string(),
        fields,
    })
}

fn parse_pcf8591_line(line: &str) -> Option<ParsedSensorEvent> {
    let mut addr = "0x48".to_string();
    let mut ain0: Option<u8> = None;
    let mut ain1: Option<u8> = None;
    let mut ain2: Option<u8> = None;
    let mut ain3: Option<u8> = None;

    for token in line.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("addr=") {
            addr = value.to_string();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain0=") {
            ain0 = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain1=") {
            ain1 = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain2=") {
            ain2 = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain3=") {
            ain3 = value.parse::<u8>().ok();
        }
    }

    let (Some(ain0), Some(ain1), Some(ain2), Some(ain3)) = (ain0, ain1, ain2, ain3) else {
        return None;
    };

    let mut fields = BTreeMap::new();
    fields.insert("addr".to_string(), addr);
    fields.insert("ain0".to_string(), ain0.to_string());
    fields.insert("ain1".to_string(), ain1.to_string());
    fields.insert("ain2".to_string(), ain2.to_string());
    fields.insert("ain3".to_string(), ain3.to_string());
    Some(ParsedSensorEvent {
        sensor_id: "pcf8591".to_string(),
        feature: "pcf8591".to_string(),
        fields,
    })
}

fn parse_generic_fields(line: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();

    for token in line.replace(',', " ").split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };

        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }

        let normalized = normalize_value(value);
        if normalized.is_empty() {
            continue;
        }
        fields.insert(key, normalized);
    }

    fields
}

fn normalize_value(value: &str) -> String {
    let trimmed = value
        .trim()
        .trim_matches(|c: char| c == ',' || c == ';' || c == '"');
    let trimmed = trimmed.strip_suffix('V').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('%').unwrap_or(trimmed);
    trimmed.to_string()
}

fn extract_feature(line: &str) -> Option<String> {
    let first = line.split_whitespace().next()?;
    let mut buf = String::new();
    for ch in first.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            buf.push(ch.to_ascii_lowercase());
        } else if !buf.is_empty() {
            break;
        }
    }

    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

