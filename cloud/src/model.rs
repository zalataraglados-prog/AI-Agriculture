use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use crate::constants::{
    default_ack_mismatch, default_ack_unknown_sensor, default_bind, DEFAULT_ONCE,
    DEFAULT_TIMEOUT_MS,
};

#[derive(Debug, Clone)]
pub(crate) struct CliConfig {
    pub(crate) bind_override: Option<String>,
    pub(crate) config_path: String,
    pub(crate) once: Option<bool>,
    pub(crate) max_packets: Option<u64>,
    pub(crate) timeout_override: Option<Option<Duration>>,
    pub(crate) ack_mismatch_override: Option<String>,
    pub(crate) ack_unknown_sensor_override: Option<String>,
    pub(crate) legacy_expected: Option<String>,
    pub(crate) legacy_ack_match: Option<String>,
}

#[derive(Debug)]
pub(crate) struct RuntimeConfig {
    pub(crate) bind: String,
    pub(crate) once: bool,
    pub(crate) max_packets: Option<u64>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) ack_mismatch: String,
    pub(crate) ack_unknown_sensor: String,
    pub(crate) exact_rules: HashMap<String, String>,
    pub(crate) sensor_rules: HashMap<String, SensorRule>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigFile {
    #[serde(default)]
    pub(crate) receiver: ReceiverFileConfig,
    #[serde(default)]
    pub(crate) exact_payloads: Vec<ExactPayloadRule>,
    #[serde(default)]
    pub(crate) sensors: Vec<SensorRuleFile>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReceiverFileConfig {
    #[serde(default = "default_bind")]
    pub(crate) bind: String,
    pub(crate) once: Option<bool>,
    pub(crate) timeout_ms: Option<u64>,
    #[serde(default = "default_ack_mismatch")]
    pub(crate) ack_mismatch: String,
    #[serde(default = "default_ack_unknown_sensor")]
    pub(crate) ack_unknown_sensor: String,
}

impl Default for ReceiverFileConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            once: Some(DEFAULT_ONCE),
            timeout_ms: Some(DEFAULT_TIMEOUT_MS),
            ack_mismatch: default_ack_mismatch(),
            ack_unknown_sensor: default_ack_unknown_sensor(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExactPayloadRule {
    pub(crate) payload: String,
    pub(crate) ack: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SensorRuleFile {
    pub(crate) id: String,
    pub(crate) ack: Option<String>,
    #[serde(default)]
    pub(crate) required_fields: Vec<String>,
    #[serde(default)]
    pub(crate) field_types: HashMap<String, FieldType>,
}

#[derive(Debug, Clone)]
pub(crate) struct SensorRule {
    pub(crate) ack: String,
    pub(crate) required_fields: Vec<String>,
    pub(crate) field_types: HashMap<String, FieldType>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FieldType {
    String,
    Bool,
    U8,
    U16,
    U32,
    I32,
    F32,
    F64,
}

#[derive(Debug)]
pub(crate) struct EvalResult {
    pub(crate) matched: bool,
    pub(crate) ack: String,
    pub(crate) detail: String,
}
