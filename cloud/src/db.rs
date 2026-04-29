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

    pub(crate) captured_at: DateTime<Utc>,

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

        let migration_dir = Self::resolve_migration_dir();
        let mut entries: Vec<_> = std::fs::read_dir(&migration_dir)
            .map_err(|e| format!("cannot read migration dir {}: {e}", migration_dir.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "sql").unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let sql = std::fs::read_to_string(&path)
                .map_err(|e| format!("failed to read {name}: {e}"))?;
            client.batch_execute(&sql)
                .map_err(|e| format!("failed to run migration {name}: {e}"))?;
            eprintln!("{} [db] migration ok: {}", crate::time_util::now_rfc3339(), name);
        }

        Ok(Self { client })
    }

    fn resolve_migration_dir() -> std::path::PathBuf {
        if let Ok(p) = std::env::var("CLOUD_MIGRATION_DIR") {
            return std::path::PathBuf::from(p);
        }
        std::path::PathBuf::from("sql/migrations")
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

                ON CONFLICT (captured_at, upload_id) DO UPDATE SET

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

        captured_at: DateTime<Utc>,

        upload_status: &str,

        error_message: Option<String>,

    ) -> Result<(), String> {

        let stmt = self

            .client

            .prepare(

                "UPDATE image_uploads

                 SET upload_status = $2, error_message = $3

                 WHERE upload_id = $1 AND captured_at = $4",

            )

            .map_err(|e| format!("failed to prepare upload status update: {e}"))?;

        self.client

            .execute(

                &stmt,

                &[&upload_id, &upload_status, &error_message, &captured_at],

            )

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

                    upload_id, captured_at, predicted_class, confidence, model_version,

                    topk_json, metadata_json, geometry_json, latency_ms, advice_code

                ) VALUES (

                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10

                )

                ON CONFLICT (upload_id, captured_at) DO UPDATE SET

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

                &record.captured_at,

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

                 WHERE upload_id = $1 AND captured_at = $2",

            )

            .map_err(|e| format!("failed to prepare inferred status update in tx: {e}"))?;

        tx.execute(&update_stmt, &[&record.upload_id, &record.captured_at])

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

                LEFT JOIN image_inference_results ir

                  ON ir.upload_id = iu.upload_id AND ir.captured_at = iu.captured_at

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



    pub(crate) fn get_saved_path_by_upload_id(

        &mut self,

        upload_id: &str,

    ) -> Result<Option<String>, String> {

        let stmt = self

            .client

            .prepare(

                "SELECT saved_path

                 FROM image_uploads

                 WHERE upload_id = $1

                 ORDER BY captured_at DESC

                 LIMIT 1",

            )

            .map_err(|e| format!("failed to prepare image path query: {e}"))?;

        let rows = self

            .client

            .query(&stmt, &[&upload_id])

            .map_err(|e| format!("failed to query image path: {e}"))?;

        Ok(rows.first().map(|row| row.get::<_, String>("saved_path")))

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

            let fields =

                serde_json::from_value::<HashMap<String, Value>>(fields_json).unwrap_or_default();

            out.push(SensorTelemetryQueryRow {

                ts: ts.to_rfc3339(),

                device_id: row.get("device_id"),

                sensor_id: row.get("sensor_id"),

                fields,

            });

        }



        Ok(out)
    }

    pub(crate) fn insert_plantation(&mut self, name: &str, crop_type: &str) -> Result<i32, String> {
        let row = self.client.query_one(
            "INSERT INTO plantations (name, crop_type) VALUES ($1, $2) RETURNING id",
            &[&name, &crop_type],
        ).map_err(|e| format!("insert_plantation error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn insert_uav_mission(&mut self, plantation_id: i32, mission_name: &str) -> Result<i32, String> {
        let row = self.client.query_one(
            "INSERT INTO uav_missions (plantation_id, mission_name) VALUES ($1, $2) RETURNING id",
            &[&plantation_id, &mission_name],
        ).map_err(|e| format!("insert_uav_mission error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn insert_uav_orthomosaic(&mut self, mission_id: i32, width: i32, height: i32) -> Result<i32, String> {
        let row = self.client.query_one(
            "INSERT INTO uav_orthomosaics (mission_id, width, height) VALUES ($1, $2, $3) RETURNING id",
            &[&mission_id, &width, &height],
        ).map_err(|e| format!("insert_uav_orthomosaic error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn insert_uav_tile(&mut self, ortho_id: i32, tile_x: i32, tile_y: i32) -> Result<i32, String> {
        let row = self.client.query_one(
            "INSERT INTO uav_tiles (orthomosaic_id, tile_x, tile_y) VALUES ($1, $2, $3) RETURNING id",
            &[&ortho_id, &tile_x, &tile_y],
        ).map_err(|e| format!("insert_uav_tile error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn insert_uav_detection(&mut self, mission_id: i32, ortho_id: i32, cx: f64, cy: f64, conf: f64) -> Result<i32, String> {
        let row = self.client.query_one(
            "INSERT INTO uav_tree_detections (mission_id, orthomosaic_id, crown_center_x, crown_center_y, confidence) VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[&mission_id, &ortho_id, &cx, &cy, &conf],
        ).map_err(|e| format!("insert_uav_detection error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn query_detections_by_orthomosaic(&mut self, ortho_id: i32) -> Result<Vec<serde_json::Value>, String> {
        let rows = self.client.query(
            "SELECT id, crown_center_x, crown_center_y, confidence, review_status FROM uav_tree_detections WHERE orthomosaic_id = $1",
            &[&ortho_id],
        ).map_err(|e| format!("query_detections error: {}", e))?;
        let mut out = Vec::new();
        for r in rows {
            let id: i32 = r.get("id");
            let cx: Option<f64> = r.get("crown_center_x");
            let cy: Option<f64> = r.get("crown_center_y");
            let conf: f64 = r.get("confidence");
            let status: String = r.get("review_status");
            out.push(serde_json::json!({
                "id": id,
                "crown_center_x": cx,
                "crown_center_y": cy,
                "confidence": conf,
                "review_status": status
            }));
        }
        Ok(out)
    }

    pub(crate) fn update_detection_status(&mut self, det_id: i32, status: &str) -> Result<(), String> {
        let affected = self.client.execute(
            "UPDATE uav_tree_detections SET review_status = $1 WHERE id = $2",
            &[&status, &det_id],
        ).map_err(|e| format!("update_detection_status error: {}", e))?;
        if affected == 0 {
            return Err("detection not found".to_string());
        }
        Ok(())
    }

    pub(crate) fn get_detection_by_id(&mut self, det_id: i32) -> Result<Option<serde_json::Value>, String> {
        let rows = self.client.query(
            "SELECT id, mission_id, orthomosaic_id, crown_center_x, crown_center_y, review_status, matched_tree_id FROM uav_tree_detections WHERE id = $1",
            &[&det_id],
        ).map_err(|e| format!("get_detection_by_id error: {}", e))?;
        if rows.is_empty() { return Ok(None); }
        let r = &rows[0];
        let mid: i32 = r.get("mission_id");
        let oid: Option<i32> = r.get("orthomosaic_id");
        let cx: Option<f64> = r.get("crown_center_x");
        let cy: Option<f64> = r.get("crown_center_y");
        let status: String = r.get("review_status");
        let matched: Option<i32> = r.get("matched_tree_id");
        Ok(Some(serde_json::json!({
            "mission_id": mid,
            "orthomosaic_id": oid,
            "crown_center_x": cx,
            "crown_center_y": cy,
            "review_status": status,
            "matched_tree_id": matched
        })))
    }

    pub(crate) fn insert_tree(&mut self, plantation_id: i32, species: &str, tree_code: &str, cx: Option<f64>, cy: Option<f64>, source_ortho: Option<i32>) -> Result<i32, String> {
        let manual_verified = true;
        let row = self.client.query_one(
            "INSERT INTO trees (plantation_id, species, tree_code, crown_center_x, crown_center_y, coordinate_x, coordinate_y, source_orthomosaic_id, barcode_value, manual_verified) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            &[&plantation_id, &species, &tree_code, &cx, &cy, &cx, &cy, &source_ortho, &tree_code, &manual_verified],
        ).map_err(|e| format!("insert_tree error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn get_tree_code_by_id(&mut self, tree_id: i32) -> Result<Option<String>, String> {
        let rows = self.client.query(
            "SELECT tree_code FROM trees WHERE id = $1",
            &[&tree_id],
        ).map_err(|e| format!("get_tree_code_by_id error: {}", e))?;
        if rows.is_empty() { return Ok(None); }
        let code: String = rows[0].get(0);
        Ok(Some(code))
    }

    pub(crate) fn get_tree_by_code(&mut self, tree_code: &str) -> Result<Option<serde_json::Value>, String> {
        let rows = self.client.query(
            "SELECT id, tree_code, species, current_status, crown_center_x, crown_center_y FROM trees WHERE tree_code = $1",
            &[&tree_code],
        ).map_err(|e| format!("get_tree_by_code error: {}", e))?;
        if rows.is_empty() { return Ok(None); }
        let r = &rows[0];
        let id: i32 = r.get("id");
        let code: String = r.get("tree_code");
        let species: String = r.get("species");
        let status: String = r.get("current_status");
        let cx: Option<f64> = r.get("crown_center_x");
        let cy: Option<f64> = r.get("crown_center_y");
        Ok(Some(serde_json::json!({
            "id": id,
            "tree_code": code,
            "species": species,
            "current_status": status,
            "crown_center_x": cx,
            "crown_center_y": cy
        })))
    }

    pub(crate) fn get_max_tree_seq(&mut self, prefix: &str) -> Result<i64, String> {
        let like_pattern = format!("{}%", prefix);
        let row = self.client.query_one(
            "SELECT COUNT(*) as cnt FROM trees WHERE tree_code LIKE $1",
            &[&like_pattern],
        ).map_err(|e| format!("get_max_tree_seq error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn link_detection_to_tree(&mut self, det_id: i32, tree_id: i32) -> Result<(), String> {
        self.client.execute(
            "UPDATE uav_tree_detections SET matched_tree_id = $1 WHERE id = $2",
            &[&tree_id, &det_id],
        ).map_err(|e| format!("link_detection_to_tree error: {}", e))?;
        Ok(())
    }

    pub(crate) fn get_mission_id_by_orthomosaic(&mut self, ortho_id: i32) -> Result<i32, String> {
        let row = self.client.query_one(
            "SELECT mission_id FROM uav_orthomosaics WHERE id = $1",
            &[&ortho_id],
        ).map_err(|e| format!("get_mission_id_by_orthomosaic error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn get_plantation_id_by_detection(&mut self, det_id: i32) -> Result<i32, String> {
        let row = self.client.query_one(
            "SELECT m.plantation_id FROM uav_tree_detections d JOIN uav_missions m ON d.mission_id = m.id WHERE d.id = $1",
            &[&det_id],
        ).map_err(|e| format!("get_plantation_id_by_detection error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn next_tree_code_seq(&mut self) -> Result<i64, String> {
        let row = self.client.query_one(
            "SELECT nextval('tree_code_seq')",
            &[],
        ).map_err(|e| format!("next_tree_code_seq error: {}", e))?;
        Ok(row.get(0))
    }

    pub(crate) fn confirm_detection_tx(&mut self, det_id: i32, plantation_id: i32, species: &str, tree_code: &str, cx: Option<f64>, cy: Option<f64>, source_ortho: Option<i32>) -> Result<i32, String> {
        let mut tx = self.client.transaction().map_err(|e| format!("transaction start error: {}", e))?;
        let manual_verified = true;
        
        let row = tx.query_one(
            "INSERT INTO trees (plantation_id, species, tree_code, crown_center_x, crown_center_y, coordinate_x, coordinate_y, source_orthomosaic_id, barcode_value, manual_verified) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            &[&plantation_id, &species, &tree_code, &cx, &cy, &cx, &cy, &source_ortho, &tree_code, &manual_verified],
        ).map_err(|e| format!("insert_tree error: {}", e))?;
        let tree_id: i32 = row.get(0);
        
        let affected = tx.execute(
            "UPDATE uav_tree_detections SET review_status = 'confirmed', matched_tree_id = $1 WHERE id = $2",
            &[&tree_id, &det_id],
        ).map_err(|e| format!("update_detection_status error: {}", e))?;
        
        if affected == 0 {
            return Err("detection not found".to_string());
        }
        
        tx.commit().map_err(|e| format!("transaction commit error: {}", e))?;
        Ok(tree_id)
    }
}