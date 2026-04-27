use crate::models::{ImageIndexRecord, ProcessedData, ProcessedSensorData};
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct CustomGatewayProcessor;

impl CustomGatewayProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process(&self, raw_data: &[u8]) -> Option<ProcessedData> {
        let text = std::str::from_utf8(raw_data).ok()?;
        let v: Value = serde_json::from_str(text).ok()?;

        // 判断是图片数据还是传感器数据
        if v.get("file_path").is_some() {
            // --- 图片数据处理 ---
            let file_path = v["file_path"].as_str()?.to_string();

            // 获取 device_id，如果没有则给默认值
            let device_id = v.get("device_id")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN_IMG_DEV")
                .to_string();

            let capture_time_str = v.get("time")
                .or(v.get("capture_time"))
                .and_then(|v| v.as_str())?;
            let capture_time = capture_time_str.parse::<DateTime<Utc>>().ok()?;

            let object_stamp = v.get("object_stamp")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let region_code = v.get("region")
                .or(v.get("region_code"))
                .and_then(|v| v.as_str())
                .unwrap_or("MY")
                .to_string();

            // 构造 ImageIndexRecord (确保字段与 models.rs 一致)
            Some(ProcessedData::Image(ImageIndexRecord {
                file_path,
                capture_time,
                object_stamp,
                region_code, // ✅ 修复了之前的 region_cod e 空格错误
                device_id,   // ✅ 确保包含 device_id
            }))
        } else {
            // --- 传感器数据处理 ---
            let device_id = v.get("device_id").and_then(|v| v.as_str())?.to_string();
            let time_str = v.get("time").and_then(|v| v.as_str())?;
            let time = time_str.parse::<DateTime<Utc>>().ok()?;
            let value = v.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let status = v.get("status").and_then(|v| v.as_str()).unwrap_or("pending").to_string();

            let region_code = v.get("region")
                .or(v.get("region_code"))
                .and_then(|v| v.as_str())
                .unwrap_or("MY")
                .to_string();

            // 构造 ProcessedSensorData
            Some(ProcessedData::Sensor(ProcessedSensorData {
                time,
                device_id,
                value,
                status,
                region_code, // ✅ 修复了之前的 r egion_code 空格错误
            }))
        }
    }
}