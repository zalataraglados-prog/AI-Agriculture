use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedSensorData {
    pub time: DateTime<Utc>,
    pub device_id: String,
    pub value: f64,
    pub status: String,
    pub region_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageIndexRecord {
    pub file_path: String,
    pub capture_time: DateTime<Utc>,
    pub object_stamp: String,
    pub region_code: String,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub enum ProcessedData {
    Sensor(ProcessedSensorData),
    Image(ImageIndexRecord),
}

#[derive(Debug, Deserialize, Clone)]
pub struct DataQueryRequest {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub device_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SensorDataChunk {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub data: Vec<ProcessedSensorData>,
}

#[derive(Debug, Serialize)]
pub struct ImageDataChunk {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub data: Vec<ImageIndexRecord>,
}