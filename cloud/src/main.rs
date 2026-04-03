use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::time::Duration;

use serde::Deserialize;

const DEFAULT_BIND: &str = "0.0.0.0:9000";
const DEFAULT_CONFIG_PATH: &str = "config/sensors.toml";
const DEFAULT_ACK_MATCH_LEGACY: &str = "ack:success";
const DEFAULT_ACK_MISMATCH: &str = "ack:error";
const DEFAULT_ACK_UNKNOWN_SENSOR: &str = "ack:unknown_sensor";
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_ONCE: bool = false;

#[derive(Debug)]
struct CliConfig {
    bind_override: Option<String>,
    config_path: String,
    once: Option<bool>,
    max_packets: Option<u64>,
    timeout_override: Option<Option<Duration>>,
    ack_mismatch_override: Option<String>,
    ack_unknown_sensor_override: Option<String>,
    legacy_expected: Option<String>,
    legacy_ack_match: Option<String>,
}

#[derive(Debug)]
struct RuntimeConfig {
    bind: String,
    once: bool,
    max_packets: Option<u64>,
    timeout: Option<Duration>,
    ack_mismatch: String,
    ack_unknown_sensor: String,
    exact_rules: HashMap<String, String>,
    sensor_rules: HashMap<String, SensorRule>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    receiver: ReceiverFileConfig,
    #[serde(default)]
    exact_payloads: Vec<ExactPayloadRule>,
    #[serde(default)]
    sensors: Vec<SensorRuleFile>,
}

#[derive(Debug, Deserialize)]
struct ReceiverFileConfig {
    #[serde(default = "default_bind")]
    bind: String,
    once: Option<bool>,
    timeout_ms: Option<u64>,
    #[serde(default = "default_ack_mismatch")]
    ack_mismatch: String,
    #[serde(default = "default_ack_unknown_sensor")]
    ack_unknown_sensor: String,
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
struct ExactPayloadRule {
    payload: String,
    ack: String,
}

#[derive(Debug, Deserialize)]
struct SensorRuleFile {
    id: String,
    ack: Option<String>,
    #[serde(default)]
    required_fields: Vec<String>,
    #[serde(default)]
    field_types: HashMap<String, FieldType>,
}

#[derive(Debug, Clone)]
struct SensorRule {
    ack: String,
    required_fields: Vec<String>,
    field_types: HashMap<String, FieldType>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FieldType {
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
struct EvalResult {
    matched: bool,
    ack: String,
    detail: String,
}

fn default_bind() -> String {
    DEFAULT_BIND.to_string()
}

fn default_ack_mismatch() -> String {
    DEFAULT_ACK_MISMATCH.to_string()
}

fn default_ack_unknown_sensor() -> String {
    DEFAULT_ACK_UNKNOWN_SENSOR.to_string()
}

fn print_usage(binary: &str) {
    eprintln!(
        "Usage:
  {binary} [--config <path>] [--bind <ip:port>] [--once] [--max-packets <n>] [--timeout-ms <ms>]
          [--ack-mismatch <payload>] [--ack-unknown-sensor <payload>]
          [--expected <legacy-payload>] [--ack-match <legacy-ack>]

Defaults:
  --config {DEFAULT_CONFIG_PATH}
  --bind from config (fallback {DEFAULT_BIND})
  --timeout-ms from config (fallback {DEFAULT_TIMEOUT_MS})
  --once from config (fallback {DEFAULT_ONCE})
  --ack-mismatch from config (fallback {DEFAULT_ACK_MISMATCH})
  --ack-unknown-sensor from config (fallback {DEFAULT_ACK_UNKNOWN_SENSOR})

Config mode:
  Uses exact payload rules and sensor rules from TOML.
  Example sensor payload:
    mq7:raw=206,voltage=0.166

Legacy compatibility:
  --expected / --ack-match adds one temporary exact rule at runtime."
    );
}

fn parse_args() -> Result<CliConfig, String> {
    let mut bind_override: Option<String> = None;
    let mut config_path = DEFAULT_CONFIG_PATH.to_string();
    let mut once: Option<bool> = None;
    let mut max_packets: Option<u64> = None;
    let mut timeout_override: Option<Option<Duration>> = None;
    let mut ack_mismatch_override: Option<String> = None;
    let mut ack_unknown_sensor_override: Option<String> = None;
    let mut legacy_expected: Option<String> = None;
    let mut legacy_ack_match: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                bind_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --bind".to_string())?,
                );
            }
            "--config" => {
                config_path = args
                    .next()
                    .ok_or_else(|| "Missing value for --config".to_string())?;
            }
            "--once" => {
                once = Some(true);
            }
            "--max-packets" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "Missing value for --max-packets".to_string())?;
                let value = raw
                    .parse::<u64>()
                    .map_err(|_| "Invalid --max-packets, expected unsigned integer".to_string())?;
                if value == 0 {
                    return Err("--max-packets must be >= 1".to_string());
                }
                max_packets = Some(value);
                once = Some(false);
            }
            "--timeout-ms" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "Missing value for --timeout-ms".to_string())?;
                let ms = raw
                    .parse::<u64>()
                    .map_err(|_| "Invalid --timeout-ms, expected unsigned integer".to_string())?;
                timeout_override = Some(if ms == 0 {
                    None
                } else {
                    Some(Duration::from_millis(ms))
                });
            }
            "--ack-mismatch" => {
                ack_mismatch_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --ack-mismatch".to_string())?,
                );
            }
            "--ack-unknown-sensor" => {
                ack_unknown_sensor_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --ack-unknown-sensor".to_string())?,
                );
            }
            "--expected" => {
                legacy_expected = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --expected".to_string())?,
                );
            }
            "--ack-match" => {
                legacy_ack_match = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --ack-match".to_string())?,
                );
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    if legacy_ack_match.is_some() && legacy_expected.is_none() {
        return Err("--ack-match requires --expected".to_string());
    }

    Ok(CliConfig {
        bind_override,
        config_path,
        once,
        max_packets,
        timeout_override,
        ack_mismatch_override,
        ack_unknown_sensor_override,
        legacy_expected,
        legacy_ack_match,
    })
}

fn load_runtime_config(cli: CliConfig) -> Result<RuntimeConfig, String> {
    let content = fs::read_to_string(&cli.config_path)
        .map_err(|e| format!("Failed to read config file {}: {e}", cli.config_path))?;
    let file_cfg: ConfigFile =
        toml::from_str(&content).map_err(|e| format!("Failed to parse TOML config: {e}"))?;

    let mut exact_rules = HashMap::new();
    for rule in file_cfg.exact_payloads {
        if rule.payload.trim().is_empty() {
            return Err("Config error: exact payload must not be empty".to_string());
        }
        if exact_rules
            .insert(rule.payload.clone(), rule.ack.clone())
            .is_some()
        {
            return Err(format!(
                "Config error: duplicate exact payload rule: {}",
                rule.payload
            ));
        }
    }

    let mut sensor_rules = HashMap::new();
    for rule in file_cfg.sensors {
        if rule.id.trim().is_empty() {
            return Err("Config error: sensor id must not be empty".to_string());
        }

        let ack = rule.ack.unwrap_or_else(|| format!("ack:{}", rule.id));
        let compiled = SensorRule {
            ack,
            required_fields: rule.required_fields,
            field_types: rule.field_types,
        };

        if sensor_rules.insert(rule.id.clone(), compiled).is_some() {
            return Err(format!("Config error: duplicate sensor id: {}", rule.id));
        }
    }

    if let Some(legacy_expected) = cli.legacy_expected {
        let ack = cli
            .legacy_ack_match
            .unwrap_or_else(|| DEFAULT_ACK_MATCH_LEGACY.to_string());
        exact_rules.insert(legacy_expected, ack);
    }

    let bind = cli
        .bind_override
        .unwrap_or_else(|| file_cfg.receiver.bind.clone());

    let once = cli
        .once
        .unwrap_or_else(|| file_cfg.receiver.once.unwrap_or(DEFAULT_ONCE));

    let timeout = match cli.timeout_override {
        Some(override_value) => override_value,
        None => file_cfg.receiver.timeout_ms.map(Duration::from_millis),
    };

    let ack_mismatch = cli
        .ack_mismatch_override
        .unwrap_or_else(|| file_cfg.receiver.ack_mismatch.clone());

    let ack_unknown_sensor = cli
        .ack_unknown_sensor_override
        .unwrap_or_else(|| file_cfg.receiver.ack_unknown_sensor.clone());

    Ok(RuntimeConfig {
        bind,
        once,
        max_packets: cli.max_packets,
        timeout,
        ack_mismatch,
        ack_unknown_sensor,
        exact_rules,
        sensor_rules,
    })
}

fn parse_sensor_kv_payload(payload: &str) -> Result<(String, HashMap<String, String>), String> {
    let (sensor_id, kv_text) = payload
        .split_once(':')
        .ok_or_else(|| "missing ':' separator".to_string())?;

    let sensor_id = sensor_id.trim();
    if sensor_id.is_empty() {
        return Err("sensor id is empty".to_string());
    }

    let mut fields = HashMap::new();
    for pair in kv_text.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("invalid field format: {pair}"))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return Err("field key is empty".to_string());
        }
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(format!("duplicate field: {key}"));
        }
    }

    if fields.is_empty() {
        return Err("no fields found".to_string());
    }

    Ok((sensor_id.to_string(), fields))
}

fn validate_field_type(value: &str, field_type: FieldType) -> bool {
    match field_type {
        FieldType::String => true,
        FieldType::Bool => value.parse::<bool>().is_ok(),
        FieldType::U8 => value.parse::<u8>().is_ok(),
        FieldType::U16 => value.parse::<u16>().is_ok(),
        FieldType::U32 => value.parse::<u32>().is_ok(),
        FieldType::I32 => value.parse::<i32>().is_ok(),
        FieldType::F32 => value.parse::<f32>().map(|v| v.is_finite()).unwrap_or(false),
        FieldType::F64 => value.parse::<f64>().map(|v| v.is_finite()).unwrap_or(false),
    }
}

fn evaluate_payload(payload: &str, cfg: &RuntimeConfig) -> EvalResult {
    if let Some(ack) = cfg.exact_rules.get(payload) {
        return EvalResult {
            matched: true,
            ack: ack.clone(),
            detail: "matched exact payload rule".to_string(),
        };
    }

    let (sensor_id, fields) = match parse_sensor_kv_payload(payload) {
        Ok(v) => v,
        Err(err) => {
            return EvalResult {
                matched: false,
                ack: cfg.ack_mismatch.clone(),
                detail: format!("invalid payload format: {err}"),
            };
        }
    };

    let rule = match cfg.sensor_rules.get(&sensor_id) {
        Some(rule) => rule,
        None => {
            return EvalResult {
                matched: false,
                ack: cfg.ack_unknown_sensor.clone(),
                detail: format!("unknown sensor id: {sensor_id}"),
            };
        }
    };

    for required in &rule.required_fields {
        if !fields.contains_key(required) {
            return EvalResult {
                matched: false,
                ack: cfg.ack_mismatch.clone(),
                detail: format!("sensor {sensor_id} missing required field: {required}"),
            };
        }
    }

    for (field, field_type) in &rule.field_types {
        if let Some(value) = fields.get(field) {
            if !validate_field_type(value, *field_type) {
                return EvalResult {
                    matched: false,
                    ack: cfg.ack_mismatch.clone(),
                    detail: format!(
                        "sensor {sensor_id} field type mismatch: {field}={value} does not match {field_type:?}"
                    ),
                };
            }
        }
    }

    EvalResult {
        matched: true,
        ack: rule.ack.clone(),
        detail: format!("matched sensor rule: {sensor_id}"),
    }
}

fn run(cfg: &RuntimeConfig) -> Result<(), String> {
    let socket =
        UdpSocket::bind(&cfg.bind).map_err(|e| format!("Bind failed on {}: {e}", cfg.bind))?;
    socket
        .set_read_timeout(cfg.timeout)
        .map_err(|e| format!("Failed to set read timeout: {e}"))?;

    println!("[cloud] Listening on {}", cfg.bind);
    println!(
        "[cloud] Loaded rules: exact={}, sensors={}",
        cfg.exact_rules.len(),
        cfg.sensor_rules.len()
    );
    println!(
        "[cloud] ACK defaults: mismatch=\"{}\", unknown_sensor=\"{}\"",
        cfg.ack_mismatch, cfg.ack_unknown_sensor
    );
    println!(
        "[cloud] Mode: {}",
        if cfg.once {
            "exit after first successful match"
        } else {
            "continuous/limited receive"
        }
    );

    let mut buf = [0_u8; 2048];
    let mut received_count: u64 = 0;
    let mut success_count: u64 = 0;

    loop {
        let (size, peer) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                return Err("Receive timeout reached without enough packets".to_string());
            }
            Err(err) => return Err(format!("Receive failed: {err}")),
        };

        received_count += 1;
        let payload = String::from_utf8_lossy(&buf[..size]).trim().to_string();
        let result = evaluate_payload(&payload, cfg);

        if result.matched {
            success_count += 1;
        }

        socket
            .send_to(result.ack.as_bytes(), peer)
            .map_err(|e| format!("ACK send failed to {peer}: {e}"))?;

        println!(
            "[cloud] Packet #{received_count} from {peer}: \"{payload}\" => {} ; ACK=\"{}\" ; {}",
            if result.matched { "MATCH" } else { "MISMATCH" },
            result.ack,
            result.detail
        );

        if cfg.once && result.matched {
            break;
        }

        if let Some(max) = cfg.max_packets {
            if received_count >= max {
                break;
            }
        }
    }

    println!("[cloud] Summary: received={received_count}, matched={success_count}");

    if success_count == 0 {
        return Err("No matching packet received".to_string());
    }

    Ok(())
}

fn main() {
    let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
    let cli = match parse_args() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Argument error: {err}\n");
            print_usage(&binary);
            std::process::exit(2);
        }
    };

    let cfg = match load_runtime_config(cli) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Config error: {err}");
            std::process::exit(2);
        }
    };

    if let Err(err) = run(&cfg) {
        eprintln!("[cloud] ERROR: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_payload, parse_sensor_kv_payload, FieldType, RuntimeConfig, SensorRule,
        DEFAULT_ACK_MISMATCH, DEFAULT_ACK_UNKNOWN_SENSOR,
    };
    use std::collections::HashMap;

    fn test_runtime() -> RuntimeConfig {
        let mut exact_rules = HashMap::new();
        exact_rules.insert("success".to_string(), "ack:success".to_string());

        let mut mq7_types = HashMap::new();
        mq7_types.insert("raw".to_string(), FieldType::U16);
        mq7_types.insert("voltage".to_string(), FieldType::F32);

        let mut sensor_rules = HashMap::new();
        sensor_rules.insert(
            "mq7".to_string(),
            SensorRule {
                ack: "ack:mq7".to_string(),
                required_fields: vec!["raw".to_string(), "voltage".to_string()],
                field_types: mq7_types,
            },
        );

        RuntimeConfig {
            bind: "0.0.0.0:9000".to_string(),
            once: false,
            max_packets: None,
            timeout: None,
            ack_mismatch: DEFAULT_ACK_MISMATCH.to_string(),
            ack_unknown_sensor: DEFAULT_ACK_UNKNOWN_SENSOR.to_string(),
            exact_rules,
            sensor_rules,
        }
    }

    #[test]
    fn parse_sensor_payload_ok() {
        let (sensor, fields) =
            parse_sensor_kv_payload("mq7:raw=206,voltage=0.166").expect("should parse");
        assert_eq!(sensor, "mq7");
        assert_eq!(fields.get("raw"), Some(&"206".to_string()));
        assert_eq!(fields.get("voltage"), Some(&"0.166".to_string()));
    }

    #[test]
    fn parse_sensor_payload_err() {
        assert!(parse_sensor_kv_payload("mq7").is_err());
        assert!(parse_sensor_kv_payload("mq7:bad").is_err());
    }

    #[test]
    fn evaluate_exact_success() {
        let cfg = test_runtime();
        let result = evaluate_payload("success", &cfg);
        assert!(result.matched);
        assert_eq!(result.ack, "ack:success");
    }

    #[test]
    fn evaluate_sensor_success() {
        let cfg = test_runtime();
        let result = evaluate_payload("mq7:raw=206,voltage=0.166", &cfg);
        assert!(result.matched);
        assert_eq!(result.ack, "ack:mq7");
    }

    #[test]
    fn evaluate_sensor_type_mismatch() {
        let cfg = test_runtime();
        let result = evaluate_payload("mq7:raw=abc,voltage=0.166", &cfg);
        assert!(!result.matched);
        assert_eq!(result.ack, "ack:error");
    }
}
