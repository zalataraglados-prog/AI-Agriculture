use std::collections::HashMap;

use chrono::{DateTime, Utc};
use postgres::{Client, NoTls};
use serde::Serialize;
use serde_json::Value;

pub(crate) struct DbManager {
    client: Client,
}

#[derive(Debug, Clone)]
pub(crate) struct ImageUploadDbRecord {
    pub(crate) upload_id: String,
    pub(crate) device_id: String,
    pub(crate) captured_at: DateTime<Utc>,
    pub(crate) received_at: DateTime<Utc>,
    pub(crate) location: String,
    pub(crate) crop_type: String,
    pub(crate) farm_note: String,
    pub(crate) saved_path: String,
    pub(crate) sha256: String,
    pub(crate) image_type: String,
    pub(crate) file_size: i64,
    pub(crate) upload_status: String,
    pub(crate) error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ImageInferenceDbRecord {
    pub(crate) upload_id: String,
    pub(crate) predicted_class: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) model_version: Option<String>,
    pub(crate) topk_json: Value,
    pub(crate) metadata_json: Value,
    pub(crate) geometry_json: Option<Value>,
    pub(crate) latency_ms: Option<i32>,
    pub(crate) advice_code: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SensorTelemetryDbRecord {
    pub(crate) ts: DateTime<Utc>,
    pub(crate) device_id: String,
    pub(crate) sensor_id: String,
    pub(crate) fields_json: Value,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ImageUploadQueryFilter {
    pub(crate) start_time: Option<DateTime<Utc>>,
    pub(crate) end_time: Option<DateTime<Utc>>,
    pub(crate) device_id: Option<String>,
    pub(crate) crop_type: Option<String>,
    pub(crate) upload_status: Option<String>,
    pub(crate) predicted_class: Option<String>,
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImageUploadQueryRow {
    pub(crate) upload_id: String,
    pub(crate) device_id: String,
    pub(crate) captured_at: String,
    pub(crate) received_at: String,
    pub(crate) location: String,
    pub(crate) crop_type: String,
    pub(crate) farm_note: String,
    pub(crate) saved_path: String,
    pub(crate) sha256: String,
    pub(crate) image_type: String,
    pub(crate) file_size: i64,
    pub(crate) upload_status: String,
    pub(crate) predicted_class: Option<String>,
    pub(crate) disease_rate: Option<f64>,
    pub(crate) is_diseased: Option<bool>,
    pub(crate) model_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SensorTelemetryQueryFilter {
    pub(crate) start_time: Option<DateTime<Utc>>,
    pub(crate) end_time: Option<DateTime<Utc>>,
    pub(crate) device_id: Option<String>,
    pub(crate) sensor_id: Option<String>,
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SensorTelemetryQueryRow {
    pub(crate) ts: String,
    pub(crate) device_id: String,
    pub(crate) sensor_id: String,
    pub(crate) fields: HashMap<String, Value>,
}

impl DbManager {
    pub(crate) fn connect_and_migrate(database_url: &str) -> Result<Self, String> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|e| format!("failed to connect postgres: {e}"))?;

        client
            .batch_execute(include_str!(
                "../sql/migrations/0001_create_new_backend_tables.sql"
            ))
            .map_err(|e| {
                format!("failed to run migration 0001_create_new_backend_tables.sql: {e}")
            })?;
        client
            .batch_execute(include_str!(
                "../sql/migrations/0002_migrate_legacy_tables.sql"
            ))
            .map_err(|e| format!("failed to run migration 0002_migrate_legacy_tables.sql: {e}"))?;

        Ok(Self { client })
    }

    pub(crate) fn insert_image_upload(
        &mut self,
        record: &ImageUploadDbRecord,
    ) -> Result<(), String> {
        let stmt = self
            .client
            .prepare(
                "INSERT INTO image_uploads (
                    upload_id, device_id, captured_at, received_at, location, crop_type, farm_note,
                    saved_path, sha256, image_type, file_size, upload_status, error_message
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (upload_id) DO UPDATE SET
                    device_id = EXCLUDED.device_id,
                    captured_at = EXCLUDED.captured_at,
                    received_at = EXCLUDED.received_at,
                    location = EXCLUDED.location,
                    crop_type = EXCLUDED.crop_type,
                    farm_note = EXCLUDED.farm_note,
                    saved_path = EXCLUDED.saved_path,
                    sha256 = EXCLUDED.sha256,
                    image_type = EXCLUDED.image_type,
                    file_size = EXCLUDED.file_size,
                    upload_status = EXCLUDED.upload_status,
                    error_message = EXCLUDED.error_message",
            )
            .map_err(|e| format!("failed to prepare image upload insert: {e}"))?;
        self.client
            .execute(
                &stmt,
                &[
                    &record.upload_id,
                    &record.device_id,
                    &record.captured_at,
                    &record.received_at,
                    &record.location,
                    &record.crop_type,
                    &record.farm_note,
                    &record.saved_path,
                    &record.sha256,
                    &record.image_type,
                    &record.file_size,
                    &record.upload_status,
                    &record.error_message,
                ],
            )
            .map_err(|e| format!("failed to insert image upload record: {e}"))?;
        Ok(())
    }

    pub(crate) fn insert_sensor_telemetry(
        &mut self,
        record: &SensorTelemetryDbRecord,
    ) -> Result<(), String> {
        let stmt = self
            .client
            .prepare(
                "INSERT INTO sensor_telemetry (ts, device_id, sensor_id, fields_json)
                 VALUES ($1, $2, $3, $4)",
            )
            .map_err(|e| format!("failed to prepare sensor telemetry insert: {e}"))?;
        self.client
            .execute(
                &stmt,
                &[
                    &record.ts,
                    &record.device_id,
                    &record.sensor_id,
                    &record.fields_json,
                ],
            )
            .map_err(|e| format!("failed to insert sensor telemetry: {e}"))?;
        Ok(())
    }

    pub(crate) fn update_upload_status(
        &mut self,
        upload_id: &str,
        upload_status: &str,
        error_message: Option<String>,
    ) -> Result<(), String> {
        let stmt = self
            .client
            .prepare(
                "UPDATE image_uploads
                 SET upload_status = $2, error_message = $3
                 WHERE upload_id = $1",
            )
            .map_err(|e| format!("failed to prepare upload status update: {e}"))?;
        self.client
            .execute(&stmt, &[&upload_id, &upload_status, &error_message])
            .map_err(|e| format!("failed to update upload status: {e}"))?;
        Ok(())
    }

    pub(crate) fn insert_inference_and_mark_inferred(
        &mut self,
        record: &ImageInferenceDbRecord,
    ) -> Result<(), String> {
        let mut tx = self
            .client
            .transaction()
            .map_err(|e| format!("failed to start inference transaction: {e}"))?;

        let insert_stmt = tx
            .prepare(
                "INSERT INTO image_inference_results (
                    upload_id, predicted_class, confidence, model_version,
                    topk_json, metadata_json, geometry_json, latency_ms, advice_code
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9
                )
                ON CONFLICT (upload_id) DO UPDATE SET
                    predicted_class = EXCLUDED.predicted_class,
                    confidence = EXCLUDED.confidence,
                    model_version = EXCLUDED.model_version,
                    topk_json = EXCLUDED.topk_json,
                    metadata_json = EXCLUDED.metadata_json,
                    geometry_json = EXCLUDED.geometry_json,
                    latency_ms = EXCLUDED.latency_ms,
                    advice_code = EXCLUDED.advice_code",
            )
            .map_err(|e| format!("failed to prepare inference insert in tx: {e}"))?;
        tx.execute(
            &insert_stmt,
            &[
                &record.upload_id,
                &record.predicted_class,
                &record.confidence,
                &record.model_version,
                &record.topk_json,
                &record.metadata_json,
                &record.geometry_json,
                &record.latency_ms,
                &record.advice_code,
            ],
        )
        .map_err(|e| format!("failed to insert inference record: {e}"))?;

        let update_stmt = tx
            .prepare(
                "UPDATE image_uploads
                 SET upload_status = 'inferred', error_message = NULL
                 WHERE upload_id = $1",
            )
            .map_err(|e| format!("failed to prepare inferred status update in tx: {e}"))?;
        tx.execute(&update_stmt, &[&record.upload_id])
            .map_err(|e| format!("failed to update upload status to inferred: {e}"))?;

        tx.commit()
            .map_err(|e| format!("failed to commit inference transaction: {e}"))?;
        Ok(())
    }

    pub(crate) fn query_image_uploads(
        &mut self,
        filter: &ImageUploadQueryFilter,
    ) -> Result<Vec<ImageUploadQueryRow>, String> {
        let limit = filter.limit.max(1).min(1000) as i64;
        let stmt = self
            .client
            .prepare(
                "SELECT
                    iu.upload_id,
                    iu.device_id,
                    iu.captured_at,
                    iu.received_at,
                    iu.location,
                    iu.crop_type,
                    iu.farm_note,
                    iu.saved_path,
                    iu.sha256,
                    iu.image_type,
                    iu.file_size,
                    iu.upload_status,
                    ir.predicted_class,
                    COALESCE(
                        NULLIF(ir.metadata_json->>'disease_rate', '')::double precision,
                        CASE
                            WHEN ir.metadata_json ? 'healthy_prob'
                                THEN 1.0 - NULLIF(ir.metadata_json->>'healthy_prob', '')::double precision
                            ELSE NULL
                        END
                    ) AS disease_rate,
                    CASE
                        WHEN COALESCE(
                            NULLIF(ir.metadata_json->>'disease_rate', '')::double precision,
                            CASE
                                WHEN ir.metadata_json ? 'healthy_prob'
                                    THEN 1.0 - NULLIF(ir.metadata_json->>'healthy_prob', '')::double precision
                                ELSE NULL
                            END
                        ) IS NULL THEN NULL
                        WHEN COALESCE(
                            NULLIF(ir.metadata_json->>'disease_rate', '')::double precision,
                            CASE
                                WHEN ir.metadata_json ? 'healthy_prob'
                                    THEN 1.0 - NULLIF(ir.metadata_json->>'healthy_prob', '')::double precision
                                ELSE NULL
                            END
                        ) >= 0.5 THEN TRUE
                        ELSE FALSE
                    END AS is_diseased,
                    ir.model_version
                FROM image_uploads iu
                LEFT JOIN image_inference_results ir ON ir.upload_id = iu.upload_id
                WHERE
                    ($1::timestamptz IS NULL OR iu.captured_at >= $1) AND
                    ($2::timestamptz IS NULL OR iu.captured_at < $2) AND
                    ($3::text IS NULL OR iu.device_id = $3) AND
                    ($4::text IS NULL OR iu.crop_type = $4) AND
                    ($5::text IS NULL OR iu.upload_status = $5) AND
                    ($6::text IS NULL OR ir.predicted_class = $6)
                ORDER BY iu.captured_at DESC
                LIMIT $7",
            )
            .map_err(|e| format!("failed to prepare image upload query: {e}"))?;

        let rows = self
            .client
            .query(
                &stmt,
                &[
                    &filter.start_time,
                    &filter.end_time,
                    &filter.device_id,
                    &filter.crop_type,
                    &filter.upload_status,
                    &filter.predicted_class,
                    &limit,
                ],
            )
            .map_err(|e| format!("failed to query image uploads: {e}"))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let captured_at: DateTime<Utc> = row.get("captured_at");
            let received_at: DateTime<Utc> = row.get("received_at");
            out.push(ImageUploadQueryRow {
                upload_id: row.get("upload_id"),
                device_id: row.get("device_id"),
                captured_at: captured_at.to_rfc3339(),
                received_at: received_at.to_rfc3339(),
                location: row.get("location"),
                crop_type: row.get("crop_type"),
                farm_note: row.get("farm_note"),
                saved_path: row.get("saved_path"),
                sha256: row.get("sha256"),
                image_type: row.get("image_type"),
                file_size: row.get("file_size"),
                upload_status: row.get("upload_status"),
                predicted_class: row.get("predicted_class"),
                disease_rate: row.get("disease_rate"),
                is_diseased: row.get("is_diseased"),
                model_version: row.get("model_version"),
            });
        }

        Ok(out)
    }

    pub(crate) fn query_sensor_telemetry(
        &mut self,
        filter: &SensorTelemetryQueryFilter,
    ) -> Result<Vec<SensorTelemetryQueryRow>, String> {
        let limit = filter.limit.max(1).min(1000) as i64;
        let stmt = self
            .client
            .prepare(
                "SELECT
                    ts,
                    device_id,
                    sensor_id,
                    fields_json
                FROM sensor_telemetry
                WHERE
                    ($1::timestamptz IS NULL OR ts >= $1) AND
                    ($2::timestamptz IS NULL OR ts < $2) AND
                    ($3::text IS NULL OR device_id = $3) AND
                    ($4::text IS NULL OR sensor_id = $4)
                ORDER BY ts DESC
                LIMIT $5",
            )
            .map_err(|e| format!("failed to prepare telemetry query: {e}"))?;

        let rows = self
            .client
            .query(
                &stmt,
                &[
                    &filter.start_time,
                    &filter.end_time,
                    &filter.device_id,
                    &filter.sensor_id,
                    &limit,
                ],
            )
            .map_err(|e| format!("failed to query telemetry: {e}"))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let ts: DateTime<Utc> = row.get("ts");
            let fields_json: Value = row.get("fields_json");
            let fields = serde_json::from_value::<HashMap<String, Value>>(fields_json)
                .unwrap_or_default();
            out.push(SensorTelemetryQueryRow {
                ts: ts.to_rfc3339(),
                device_id: row.get("device_id"),
                sensor_id: row.get("sensor_id"),
                fields,
            });
        }

        Ok(out)
    }
}

