use std::env;
use std::time::Duration;

use crate::constants::{
    DEFAULT_ACK_MISMATCH, DEFAULT_ACK_UNKNOWN_SENSOR, DEFAULT_BIND, DEFAULT_CONFIG_PATH,
    DEFAULT_ONCE, DEFAULT_REGISTRY_PATH, DEFAULT_TIMEOUT_MS, DEFAULT_TOKEN_STORE_PATH,
};
use crate::model::{CliCommand, CliConfig, TokenCliConfig};

pub(crate) fn print_usage(binary: &str) {
    eprintln!(
        "Usage:
  {binary} run [--config <path>] [--bind <ip:port>] [--once] [--max-packets <n>] [--timeout-ms <ms>]
               [--ack-mismatch <payload>] [--ack-unknown-sensor <payload>]
               [--token-store <path>] [--registry <path>] [--database-url <dsn>]
               [--expected <legacy-payload>] [--ack-match <legacy-ack>]
      {binary} token [--config <path>] [--token-store <path>]

Defaults:
  --config {DEFAULT_CONFIG_PATH}
  --bind from config (fallback {DEFAULT_BIND})
  --timeout-ms from config (fallback {DEFAULT_TIMEOUT_MS})
  --once from config (fallback {DEFAULT_ONCE})
  --ack-mismatch from config (fallback {DEFAULT_ACK_MISMATCH})
  --ack-unknown-sensor from config (fallback {DEFAULT_ACK_UNKNOWN_SENSOR})
  --token-store from config (fallback {DEFAULT_TOKEN_STORE_PATH})
  --registry from config (fallback {DEFAULT_REGISTRY_PATH})
  --database-url from CLI/env/config (required)

Notes:
  1) 默认子命令是 run（可省略 run）。
  2) 注册报文格式: register:{{json}}
  3) 实时数据必须携带 device_id=<id>，未注册返回 ack:unregistered。"
    );
}

pub(crate) fn parse_args() -> Result<CliCommand, String> {
    let mut raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() {
        return Ok(CliCommand::Run(default_run_cli()));
    }

    if raw[0] == "-h" || raw[0] == "--help" {
        let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
        print_usage(&binary);
        std::process::exit(0);
    }

    let sub = match raw[0].as_str() {
        "run" | "token" => Some(raw.remove(0)),
        _ => None,
    };

    match sub.as_deref() {
        Some("run") => parse_run_args(raw).map(CliCommand::Run),
        Some("token") => parse_token_args(raw).map(CliCommand::Token),
        Some(other) => Err(format!("Unknown subcommand: {other}")),
        None => parse_run_args(raw).map(CliCommand::Run),
    }
}

fn parse_run_args(raw_args: Vec<String>) -> Result<CliConfig, String> {
    let mut cfg = default_run_cli();

    let mut args = raw_args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                cfg.bind_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --bind".to_string())?,
                );
            }
            "--config" => {
                cfg.config_path = args
                    .next()
                    .ok_or_else(|| "Missing value for --config".to_string())?;
            }
            "--once" => {
                if cfg.max_packets.is_some() {
                    return Err("--once conflicts with --max-packets".to_string());
                }
                cfg.once = Some(true);
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
                if cfg.once == Some(true) {
                    return Err("--max-packets conflicts with --once".to_string());
                }
                cfg.max_packets = Some(value);
            }
            "--timeout-ms" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "Missing value for --timeout-ms".to_string())?;
                let ms = raw
                    .parse::<u64>()
                    .map_err(|_| "Invalid --timeout-ms, expected unsigned integer".to_string())?;
                cfg.timeout_override = Some(if ms == 0 {
                    None
                } else {
                    Some(Duration::from_millis(ms))
                });
            }
            "--ack-mismatch" => {
                cfg.ack_mismatch_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --ack-mismatch".to_string())?,
                );
            }
            "--ack-unknown-sensor" => {
                cfg.ack_unknown_sensor_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --ack-unknown-sensor".to_string())?,
                );
            }
            "--expected" => {
                cfg.legacy_expected = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --expected".to_string())?,
                );
            }
            "--ack-match" => {
                cfg.legacy_ack_match = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --ack-match".to_string())?,
                );
            }
            "--token-store" => {
                cfg.token_store_path_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --token-store".to_string())?,
                );
            }
            "--registry" => {
                cfg.registry_path_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --registry".to_string())?,
                );
            }
            "--database-url" => {
                cfg.database_url_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --database-url".to_string())?,
                );
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument for run: {arg}")),
        }
    }

    if cfg.legacy_ack_match.is_some() && cfg.legacy_expected.is_none() {
        return Err("--ack-match requires --expected".to_string());
    }

    Ok(cfg)
}

fn parse_token_args(raw_args: Vec<String>) -> Result<TokenCliConfig, String> {
    let mut config_path = DEFAULT_CONFIG_PATH.to_string();
    let mut token_store_path_override: Option<String> = None;

    let mut args = raw_args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = args
                    .next()
                    .ok_or_else(|| "Missing value for --config".to_string())?;
            }
            "--token-store" => {
                token_store_path_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --token-store".to_string())?,
                );
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument for token: {arg}")),
        }
    }

    Ok(TokenCliConfig {
        config_path,
        token_store_path_override,
    })
}

fn default_run_cli() -> CliConfig {
    CliConfig {
        bind_override: None,
        config_path: DEFAULT_CONFIG_PATH.to_string(),
        once: None,
        max_packets: None,
        timeout_override: None,
        ack_mismatch_override: None,
        ack_unknown_sensor_override: None,
        legacy_expected: None,
        legacy_ack_match: None,
        token_store_path_override: None,
        registry_path_override: None,
        database_url_override: None,
    }
}
