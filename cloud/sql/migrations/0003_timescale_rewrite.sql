CREATE EXTENSION IF NOT EXISTS timescaledb;

ALTER TABLE image_inference_results
    ADD COLUMN IF NOT EXISTS captured_at TIMESTAMPTZ;

UPDATE image_inference_results ir
SET captured_at = iu.captured_at
FROM image_uploads iu
WHERE ir.upload_id = iu.upload_id
  AND ir.captured_at IS NULL;

ALTER TABLE image_inference_results
    ALTER COLUMN captured_at SET NOT NULL;

ALTER TABLE image_inference_results
    DROP CONSTRAINT IF EXISTS image_inference_results_upload_id_key;

ALTER TABLE image_inference_results
    DROP CONSTRAINT IF EXISTS image_inference_results_upload_id_fkey;

ALTER TABLE image_inference_results
    DROP CONSTRAINT IF EXISTS image_inference_results_upload_fk;

ALTER TABLE image_inference_results
    DROP CONSTRAINT IF EXISTS image_inference_results_upload_id_captured_at_key;

ALTER TABLE image_uploads
    DROP CONSTRAINT IF EXISTS image_uploads_pkey;

ALTER TABLE image_uploads
    DROP CONSTRAINT IF EXISTS image_uploads_upload_id_captured_at_key;

ALTER TABLE image_uploads
    ADD CONSTRAINT image_uploads_pkey PRIMARY KEY (captured_at, upload_id);

ALTER TABLE image_uploads
    ADD CONSTRAINT image_uploads_upload_id_captured_at_key
    UNIQUE (upload_id, captured_at);

ALTER TABLE image_inference_results
    ADD CONSTRAINT image_inference_results_upload_id_captured_at_key
    UNIQUE (upload_id, captured_at);

ALTER TABLE image_inference_results
    ADD CONSTRAINT image_inference_results_upload_fk
    FOREIGN KEY (upload_id, captured_at)
    REFERENCES image_uploads(upload_id, captured_at)
    ON DELETE CASCADE;

SELECT create_hypertable(
    'sensor_telemetry',
    'ts',
    if_not_exists => TRUE,
    migrate_data => TRUE,
    chunk_time_interval => INTERVAL '2 hours'
);

SELECT create_hypertable(
    'image_uploads',
    'captured_at',
    if_not_exists => TRUE,
    migrate_data => TRUE,
    chunk_time_interval => INTERVAL '2 hours'
);

CREATE INDEX IF NOT EXISTS idx_sensor_tel_device_ts
    ON sensor_telemetry (device_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_sensor_tel_sensor_ts
    ON sensor_telemetry (sensor_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_image_uploads_device_captured
    ON image_uploads (device_id, captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_image_uploads_status_captured
    ON image_uploads (upload_status, captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_image_uploads_sha256
    ON image_uploads (sha256);

CREATE INDEX IF NOT EXISTS idx_image_infer_upload_captured
    ON image_inference_results (upload_id, captured_at);
CREATE INDEX IF NOT EXISTS idx_image_infer_class_conf
    ON image_inference_results (predicted_class, confidence DESC);
