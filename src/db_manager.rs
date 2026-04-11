use crate::models::{
    DataQueryRequest, ImageDataChunk, ImageIndexRecord, ProcessedData,
    ProcessedSensorData, SensorDataChunk
};
use chrono::{Duration, Utc};
use postgres::{Client, NoTls};
use std::time::{Duration as StdDuration, Instant};

pub struct DbManager {
    client: Client,
    sensor_buffer: Vec<ProcessedSensorData>,
    image_buffer: Vec<ImageIndexRecord>,
    batch_size: usize,
    last_flush_time: Instant,
    flush_interval: StdDuration,
}

impl DbManager {
    pub fn new(dsn: &str) -> Result<Self, postgres::Error> {
        let client = Client::connect(dsn, NoTls)?;
        Ok(Self {
            client,
            sensor_buffer: Vec::new(),
            image_buffer: Vec::new(),
            batch_size: 100,
            last_flush_time: Instant::now(),
            flush_interval: StdDuration::from_secs(5),
        })
    }

    pub fn add_data(&mut self, data: ProcessedData) {
        match data {
            ProcessedData::Sensor(s) => self.sensor_buffer.push(s),
            ProcessedData::Image(i) => self.image_buffer.push(i),
        }
        self.try_flush();
    }

    fn try_flush(&mut self) {
        let now = Instant::now();
        if self.sensor_buffer.len() >= self.batch_size
            || self.image_buffer.len() >= self.batch_size
            || (now.duration_since(self.last_flush_time) > self.flush_interval
            && (!self.sensor_buffer.is_empty() || !self.image_buffer.is_empty()))
        {
            if let Err(e) = self.flush_all() {
                eprintln!("[ERROR] 批量写入失败：{}", e);
            }
        }
    }

    pub fn flush_all(&mut self) -> Result<(), postgres::Error> {
        if self.sensor_buffer.is_empty() && self.image_buffer.is_empty() {
            return Ok(());
        }

        let mut tx = self.client.transaction()?;

        if !self.sensor_buffer.is_empty() {
            let stmt = tx.prepare(
                "INSERT INTO sensor_data (time, device_id, value, status, region_code)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (device_id, time) DO NOTHING"
            )?;
            for r in &self.sensor_buffer {
                tx.execute(&stmt, &[&r.time.naive_utc(), &r.device_id, &r.value, &r.status, &r.region_code])?;
            }
            println!("[INFO] 成功批量写入 {} 条传感器数据", self.sensor_buffer.len());
        }

        if !self.image_buffer.is_empty() {
            let stmt = tx.prepare(
                "INSERT INTO image_index (file_path, capture_time, object_stamp, region_code, device_id)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (file_path, capture_time) DO NOTHING"
            )?;
            for r in &self.image_buffer {
                tx.execute(&stmt, &[&r.file_path, &r.capture_time.naive_utc(), &r.object_stamp, &r.region_code, &r.device_id])?;
            }
            println!("[INFO] 成功批量写入 {} 条图片索引", self.image_buffer.len());
        }

        tx.commit()?;
        self.sensor_buffer.clear();
        self.image_buffer.clear();
        self.last_flush_time = Instant::now();
        Ok(())
    }

    pub fn get_sensor_chunks(&mut self, req: &DataQueryRequest) -> Result<Vec<SensorDataChunk>, postgres::Error> {
        let mut chunks = Vec::new();
        let mut current_start = req.start_time;

        while current_start < req.end_time {
            let current_end = std::cmp::min(current_start + Duration::hours(2), req.end_time);

            let query = if let Some(dev_id) = &req.device_id {
                "SELECT time, device_id, value, status, region_code FROM sensor_data
                 WHERE time >= $1 AND time < $2 AND device_id = $3
                 ORDER BY time ASC"
            } else {
                "SELECT time, device_id, value, status, region_code FROM sensor_data
                 WHERE time >= $1 AND time < $2
                 ORDER BY time ASC"
            };

            let rows = if let Some(dev_id) = &req.device_id {
                self.client.query(query, &[&current_start.naive_utc(), &current_end.naive_utc(), dev_id])?
            } else {
                self.client.query(query, &[&current_start.naive_utc(), &current_end.naive_utc()])?
            };

            let data: Vec<ProcessedSensorData> = rows.iter().map(|row| {
                // ✅ 修复：处理可能的 NULL 值，如果为 NULL 则给默认值
                let device_id: Option<String> = row.get("device_id");

                ProcessedSensorData {
                    time: row.get::<_, chrono::NaiveDateTime>("time").and_local_timezone(Utc).unwrap(),
                    device_id: device_id.unwrap_or_else(|| "UNKNOWN_DEV".to_string()),
                    value: row.get("value"),
                    status: row.get("status"),
                    region_code: row.get("region_code"),
                }
            }).collect();

            if !data.is_empty() {
                chunks.push(SensorDataChunk {
                    window_start: current_start,
                    window_end: current_end,
                    data,
                });
            }

            current_start = current_end;
        }
        Ok(chunks)
    }

    pub fn get_image_chunks(&mut self, req: &DataQueryRequest) -> Result<Vec<ImageDataChunk>, postgres::Error> {
        let mut chunks = Vec::new();
        let mut current_start = req.start_time;

        while current_start < req.end_time {
            let current_end = std::cmp::min(current_start + Duration::hours(2), req.end_time);

            let query = if let Some(dev_id) = &req.device_id {
                "SELECT file_path, capture_time, object_stamp, region_code, device_id FROM image_index
                 WHERE capture_time >= $1 AND capture_time < $2 AND device_id = $3
                 ORDER BY capture_time ASC"
            } else {
                "SELECT file_path, capture_time, object_stamp, region_code, device_id FROM image_index
                 WHERE capture_time >= $1 AND capture_time < $2
                 ORDER BY capture_time ASC"
            };

            let rows = if let Some(dev_id) = &req.device_id {
                self.client.query(query, &[&current_start.naive_utc(), &current_end.naive_utc(), dev_id])?
            } else {
                self.client.query(query, &[&current_start.naive_utc(), &current_end.naive_utc()])?
            };

            let data: Vec<ImageIndexRecord> = rows.iter().map(|row| {
                // ✅ 修复：处理可能的 NULL 值，如果为 NULL 则给默认值
                let device_id: Option<String> = row.get("device_id");

                ImageIndexRecord {
                    file_path: row.get("file_path"),
                    capture_time: row.get::<_, chrono::NaiveDateTime>("capture_time").and_local_timezone(Utc).unwrap(),
                    object_stamp: row.get("object_stamp"),
                    region_code: row.get("region_code"),
                    device_id: device_id.unwrap_or_else(|| "UNKNOWN_IMG".to_string()),
                }
            }).collect();

            if !data.is_empty() {
                chunks.push(ImageDataChunk {
                    window_start: current_start,
                    window_end: current_end,
                    data,
                });
            }

            current_start = current_end;
        }
        Ok(chunks)
    }

    pub fn close(mut self) {
        let _ = self.flush_all();
    }
}