CREATE TABLE IF NOT EXISTS image_uploads (
    upload_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    location TEXT NOT NULL DEFAULT '',
    crop_type TEXT NOT NULL DEFAULT '',
    farm_note TEXT NOT NULL DEFAULT '',
    saved_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    image_type TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    upload_status TEXT NOT NULL CHECK (upload_status IN ('stored', 'inferred', 'failed')),
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS image_inference_results (
    id BIGSERIAL PRIMARY KEY,
    upload_id TEXT NOT NULL REFERENCES image_uploads(upload_id) ON DELETE CASCADE,
    predicted_class TEXT,
    confidence DOUBLE PRECISION,
    model_version TEXT,
    topk_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    geometry_json JSONB,
    latency_ms INTEGER,
    advice_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (upload_id)
);

CREATE TABLE IF NOT EXISTS sensor_telemetry (
    id BIGSERIAL PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL,
    device_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    fields_json JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_image_uploads_device_captured
    ON image_uploads (device_id, captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_image_uploads_sha256
    ON image_uploads (sha256);
CREATE INDEX IF NOT EXISTS idx_image_uploads_status
    ON image_uploads (upload_status);

CREATE INDEX IF NOT EXISTS idx_image_infer_upload
    ON image_inference_results (upload_id);
CREATE INDEX IF NOT EXISTS idx_image_infer_class_conf
    ON image_inference_results (predicted_class, confidence DESC);

CREATE INDEX IF NOT EXISTS idx_sensor_tel_device_ts
    ON sensor_telemetry (device_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_sensor_tel_sensor_ts
    ON sensor_telemetry (sensor_id, ts DESC);
