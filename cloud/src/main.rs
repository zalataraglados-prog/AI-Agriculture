mod cli;
mod config;
mod constants;
mod http_server;
mod model;
mod payload;
mod registry;
mod server;
mod telemetry;
mod time_util;
mod token;

use std::env;

use cli::{parse_args, print_usage};
use config::{load_runtime_config, resolve_token_store_path_for_token_command};
use model::CliCommand;
use server::run;
use time_util::now_rfc3339;
use token::current_hour_token;

fn main() {
    let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
    let command = match parse_args() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("{} [cloud] Argument error: {err}\n", now_rfc3339());
            print_usage(&binary);
            std::process::exit(2);
        }
    };

    match command {
        CliCommand::Run(cli) => {
            let cfg = match load_runtime_config(cli) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("{} [cloud] Config error: {err}", now_rfc3339());
                    std::process::exit(2);
                }
            };

            // 启动 HTTP 后台和前端仪表盘服务 (端口 8088)
            http_server::start_http_server("0.0.0.0:8088", cfg.telemetry_store_path.clone());

            if let Err(err) = run(&cfg) {
                eprintln!("{} [cloud] ERROR: {err}", now_rfc3339());
                std::process::exit(1);
            }
        }
        CliCommand::Token(token_cli) => {
            let token_store_path = match resolve_token_store_path_for_token_command(&token_cli) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("{} [cloud] Config error: {err}", now_rfc3339());
                    std::process::exit(2);
                }
            };

            match current_hour_token(&token_store_path) {
                Ok(token) => {
                    println!("{token}");
                }
                Err(err) => {
                    eprintln!("{} [cloud] ERROR: {err}", now_rfc3339());
                    std::process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{build_runtime_config, parse_config_file};
    use crate::constants::{DEFAULT_ACK_MISMATCH, DEFAULT_ACK_UNKNOWN_SENSOR};
    use crate::model::{CliConfig, FieldType, RuntimeConfig, SensorRule};
    use crate::payload::{evaluate_payload, parse_sensor_kv_payload};
    use std::collections::HashMap;
    use std::time::Duration;

    fn default_cli() -> CliConfig {
        CliConfig {
            bind_override: None,
            config_path: "config/sensors.toml".to_string(),
            once: None,
            max_packets: None,
            timeout_override: None,
            ack_mismatch_override: None,
            ack_unknown_sensor_override: None,
            legacy_expected: None,
            legacy_ack_match: None,
            token_store_path_override: None,
            registry_path_override: None,
        }
    }

    fn test_runtime() -> RuntimeConfig {
        let mut exact_rules = HashMap::new();
        exact_rules.insert("success".to_string(), "ack:success".to_string());

        let mut mq7_types = HashMap::new();
        mq7_types.insert("raw".to_string(), FieldType::U16);
        mq7_types.insert("voltage".to_string(), FieldType::F32);
        let mut soil_modbus_types = HashMap::new();
        soil_modbus_types.insert("vwc".to_string(), FieldType::F32);
        soil_modbus_types.insert("temp_c".to_string(), FieldType::F32);
        soil_modbus_types.insert("ec".to_string(), FieldType::U32);
        soil_modbus_types.insert("protocol".to_string(), FieldType::String);
        soil_modbus_types.insert("slave_id".to_string(), FieldType::U16);

        let mut sensor_rules = HashMap::new();
        sensor_rules.insert(
            "mq7".to_string(),
            SensorRule {
                ack: "ack:mq7".to_string(),
                required_fields: vec!["raw".to_string(), "voltage".to_string()],
                field_types: mq7_types,
            },
        );
        sensor_rules.insert(
            "soil_modbus_02".to_string(),
            SensorRule {
                ack: "ack:soil_modbus_02".to_string(),
                required_fields: vec!["vwc".to_string(), "temp_c".to_string(), "ec".to_string()],
                field_types: soil_modbus_types,
            },
        );

        RuntimeConfig {
            bind: "0.0.0.0:9000".to_string(),
            once: false,
            max_packets: None,
            timeout: None,
            ack_mismatch: DEFAULT_ACK_MISMATCH.to_string(),
            ack_unknown_sensor: DEFAULT_ACK_UNKNOWN_SENSOR.to_string(),
            token_store_path: "state/token_store.test.json".to_string(),
            registry_path: "state/registry.test.json".to_string(),
            telemetry_store_path: "state/telemetry.test.jsonl".to_string(),
            exact_rules,
            sensor_rules,
        }
    }

    #[test]
    fn parse_sensor_payload_ok() {
        let (sensor, fields) = parse_sensor_kv_payload("mq7:device_id=dev01,raw=206,voltage=0.166")
            .expect("should parse");
        assert_eq!(sensor, "mq7");
        assert_eq!(fields.get("raw"), Some(&"206".to_string()));
        assert_eq!(fields.get("voltage"), Some(&"0.166".to_string()));
        assert_eq!(fields.get("device_id"), Some(&"dev01".to_string()));
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
        let result = evaluate_payload("mq7:device_id=dev01,raw=206,voltage=0.166", &cfg);
        assert!(result.matched);
        assert_eq!(result.ack, "ack:mq7");
    }

    #[test]
    fn evaluate_sensor_type_mismatch() {
        let cfg = test_runtime();
        let result = evaluate_payload("mq7:device_id=dev01,raw=abc,voltage=0.166", &cfg);
        assert!(!result.matched);
        assert_eq!(result.ack, "ack:error");
    }

    #[test]
    fn evaluate_soil_modbus_optional_fields_supported() {
        let cfg = test_runtime();
        let result = evaluate_payload(
            "soil_modbus_02:device_id=dev01,vwc=26.9,temp_c=24.8,ec=432,protocol=modbus.rtu.v1,slave_id=2",
            &cfg,
        );
        assert!(result.matched);
        assert_eq!(result.ack, "ack:soil_modbus_02");
    }

    #[test]
    fn evaluate_soil_modbus_missing_optional_fields_supported() {
        let cfg = test_runtime();
        let result = evaluate_payload(
            "soil_modbus_02:device_id=dev01,vwc=26.9,temp_c=24.8,ec=432",
            &cfg,
        );
        assert!(result.matched);
        assert_eq!(result.ack, "ack:soil_modbus_02");
    }

    #[test]
    fn build_runtime_config_from_toml_ok() {
        let content = r#"
[receiver]
bind = "127.0.0.1:9001"
once = true
timeout_ms = 5000
ack_mismatch = "ack:bad"
ack_unknown_sensor = "ack:unknown"
token_store_path = "state/token.json"
registry_path = "state/registry.json"
telemetry_store_path = "state/telemetry.jsonl"

[[exact_payloads]]
payload = "success"
ack = "ack:success"

[[sensors]]
id = "mq7"
required_fields = ["raw", "voltage"]

[sensors.field_types]
raw = "u16"
voltage = "f32"
"#;

        let file_cfg = parse_config_file(content).expect("valid toml");
        let cfg = build_runtime_config(&default_cli(), file_cfg).expect("build runtime config");

        assert_eq!(cfg.bind, "127.0.0.1:9001");
        assert!(cfg.once);
        assert_eq!(cfg.timeout, Some(Duration::from_millis(5000)));
        assert_eq!(cfg.ack_mismatch, "ack:bad");
        assert_eq!(cfg.ack_unknown_sensor, "ack:unknown");
        assert_eq!(cfg.token_store_path, "state/token.json");
        assert_eq!(cfg.registry_path, "state/registry.json");
        assert_eq!(cfg.telemetry_store_path, "state/telemetry.jsonl");
        assert_eq!(
            cfg.exact_rules.get("success"),
            Some(&"ack:success".to_string())
        );
        assert!(cfg.sensor_rules.contains_key("mq7"));
        assert_eq!(
            cfg.sensor_rules.get("mq7").map(|r| r.ack.as_str()),
            Some("ack:mq7")
        );
    }

    #[test]
    fn build_runtime_config_duplicate_sensor_id_error() {
        let content = r#"
[[sensors]]
id = "mq7"

[[sensors]]
id = "mq7"
"#;

        let file_cfg = parse_config_file(content).expect("valid toml");
        let err = build_runtime_config(&default_cli(), file_cfg).expect_err("should fail");
        assert!(err.contains("duplicate sensor id"));
    }

    #[test]
    fn build_runtime_config_legacy_rule_injection() {
        let content = r#"
[[exact_payloads]]
payload = "success"
ack = "ack:success"
"#;

        let mut cli = default_cli();
        cli.legacy_expected = Some("success".to_string());
        cli.legacy_ack_match = Some("ack:legacy".to_string());

        let file_cfg = parse_config_file(content).expect("valid toml");
        let cfg = build_runtime_config(&cli, file_cfg).expect("build runtime config");
        assert_eq!(
            cfg.exact_rules.get("success"),
            Some(&"ack:legacy".to_string())
        );
    }
}
