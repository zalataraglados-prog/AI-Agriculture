use std::env;
use std::time::Duration;

use crate::constants::{
    DEFAULT_ACK_MISMATCH, DEFAULT_ACK_UNKNOWN_SENSOR, DEFAULT_BIND, DEFAULT_CONFIG_PATH,
    DEFAULT_ONCE, DEFAULT_TIMEOUT_MS,
};
use crate::model::CliConfig;

pub(crate) fn print_usage(binary: &str) {
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

Flag note:
  --once conflicts with --max-packets

Legacy compatibility:
  --expected / --ack-match adds one temporary exact rule at runtime."
    );
}

pub(crate) fn parse_args() -> Result<CliConfig, String> {
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
                if max_packets.is_some() {
                    return Err("--once conflicts with --max-packets".to_string());
                }
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
                if once == Some(true) {
                    return Err("--max-packets conflicts with --once".to_string());
                }
                max_packets = Some(value);
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
