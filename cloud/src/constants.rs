pub(crate) const DEFAULT_BIND: &str = "0.0.0.0:9000";
pub(crate) const DEFAULT_CONFIG_PATH: &str = "config/sensors.toml";
pub(crate) const DEFAULT_ACK_MATCH_LEGACY: &str = "ack:success";
pub(crate) const DEFAULT_ACK_MISMATCH: &str = "ack:error";
pub(crate) const DEFAULT_ACK_UNKNOWN_SENSOR: &str = "ack:unknown_sensor";
pub(crate) const DEFAULT_ACK_REGISTER_OK: &str = "ack:register_ok";
pub(crate) const DEFAULT_ACK_TOKEN_INVALID: &str = "ack:token_invalid";
pub(crate) const DEFAULT_ACK_REGISTER_CONFLICT: &str = "ack:register_conflict";
pub(crate) const DEFAULT_ACK_CREDENTIAL_REVOKED: &str = "ack:credential_revoked";
pub(crate) const DEFAULT_ACK_UNREGISTERED: &str = "ack:unregistered";
pub(crate) const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub(crate) const DEFAULT_ONCE: bool = false;
pub(crate) const UDP_BUFFER_SIZE: usize = 65_535;
pub(crate) const DEFAULT_TOKEN_STORE_PATH: &str = "state/token_store.json";
pub(crate) const DEFAULT_REGISTRY_PATH: &str = "state/registry.json";
pub(crate) const DEFAULT_TELEMETRY_STORE_PATH: &str = "state/telemetry.jsonl";
pub(crate) const DEFAULT_IMAGE_STORE_PATH: &str = "state/image_uploads";
pub(crate) const DEFAULT_IMAGE_INDEX_PATH: &str = "state/image_index.jsonl";
pub(crate) const DEFAULT_IMAGE_DB_ERROR_STORE_PATH: &str = "state/image_upload_errors.jsonl";
pub(crate) const DEFAULT_AI_PREDICT_URL: &str = "http://ai-engine:8000/api/v1/predict";
pub(crate) const DEFAULT_OPENCLAW_URL: &str = "http://openclaw:3000";

pub(crate) fn default_bind() -> String {
    std::env::var("CLOUD_BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND.to_string())
}

pub(crate) fn default_ack_mismatch() -> String {
    DEFAULT_ACK_MISMATCH.to_string()
}

pub(crate) fn default_ack_unknown_sensor() -> String {
    DEFAULT_ACK_UNKNOWN_SENSOR.to_string()
}

pub(crate) fn default_token_store_path() -> String {
    std::env::var("TOKEN_STORE_PATH").unwrap_or_else(|_| DEFAULT_TOKEN_STORE_PATH.to_string())
}

pub(crate) fn default_registry_path() -> String {
    std::env::var("REGISTRY_PATH").unwrap_or_else(|_| DEFAULT_REGISTRY_PATH.to_string())
}

pub(crate) fn default_telemetry_store_path() -> String {
    std::env::var("TELEMETRY_STORE_PATH")
        .unwrap_or_else(|_| DEFAULT_TELEMETRY_STORE_PATH.to_string())
}

pub(crate) fn default_image_store_path() -> String {
    std::env::var("IMAGE_STORE_PATH").unwrap_or_else(|_| DEFAULT_IMAGE_STORE_PATH.to_string())
}

pub(crate) fn default_image_index_path() -> String {
    std::env::var("IMAGE_INDEX_PATH").unwrap_or_else(|_| DEFAULT_IMAGE_INDEX_PATH.to_string())
}

pub(crate) fn default_image_db_error_store_path() -> String {
    std::env::var("IMAGE_DB_ERROR_STORE_PATH")
        .unwrap_or_else(|_| DEFAULT_IMAGE_DB_ERROR_STORE_PATH.to_string())
}

pub(crate) fn default_ai_predict_url() -> String {
    std::env::var("AI_PREDICT_URL").unwrap_or_else(|_| DEFAULT_AI_PREDICT_URL.to_string())
}

pub(crate) fn default_openclaw_url() -> String {
    std::env::var("OPENCLAW_URL").unwrap_or_else(|_| DEFAULT_OPENCLAW_URL.to_string())
}
