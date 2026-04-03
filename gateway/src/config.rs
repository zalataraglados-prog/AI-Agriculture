use std::env;
use std::time::Duration;

use crate::constants::{
    DEFAULT_ACK_TIMEOUT_MS, DEFAULT_EXPECTED_ACK, DEFAULT_INTERVAL_MS, DEFAULT_PAYLOAD_SUCCESS,
    DEFAULT_SERIAL_BAUD, DEFAULT_TARGET,
};

#[derive(Debug, Clone, Copy)]
pub enum PayloadMode {
    FixedSuccess,
    SerialMq7,
}

#[derive(Debug)]
pub struct Config {
    pub target: String,
    pub count: Option<u64>,
    pub interval: Duration,
    pub wait_ack: bool,
    pub ack_timeout: Duration,
    pub expected_ack: String,
    pub payload_mode: PayloadMode,
    pub serial_port: Option<String>,
    pub serial_baud: u32,
}

pub fn print_usage(binary: &str) {
    eprintln!(
        "Usage:
  {binary} [--target <ip:port>] [--count <n>] [--interval-ms <ms>] [--no-wait-ack]
          [--ack-timeout-ms <ms>] [--expected-ack <payload>]
          [--serial-port </dev/ttyUSB0>] [--serial-baud <baud>]

Defaults:
  --target {DEFAULT_TARGET}
  --interval-ms {DEFAULT_INTERVAL_MS}
  --count not set (send forever)
  --ack-timeout-ms {DEFAULT_ACK_TIMEOUT_MS}
  --expected-ack {DEFAULT_EXPECTED_ACK}
  --serial-baud {DEFAULT_SERIAL_BAUD}

Payload mode:
  1) default (no --serial-port): fixed payload \"{DEFAULT_PAYLOAD_SUCCESS}\"
  2) with --serial-port: parse serial line \"MQ7 raw=<n> voltage=<v>V\"
     and send payload \"mq7:raw=<n>,voltage=<v>\""
    );
}

pub fn parse_args() -> Result<Config, String> {
    let mut target = DEFAULT_TARGET.to_string();
    let mut count: Option<u64> = None;
    let mut interval_ms = DEFAULT_INTERVAL_MS;
    let mut wait_ack = true;
    let mut ack_timeout_ms = DEFAULT_ACK_TIMEOUT_MS;
    let mut expected_ack = DEFAULT_EXPECTED_ACK.to_string();
    let mut serial_port: Option<String> = None;
    let mut serial_baud = DEFAULT_SERIAL_BAUD;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --target".to_string())?;
                target = value;
            }
            "--count" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --count".to_string())?;
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "Invalid --count, expected unsigned integer".to_string())?;
                count = Some(parsed);
            }
            "--interval-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --interval-ms".to_string())?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| "Invalid --interval-ms, expected unsigned integer".to_string())?;
            }
            "--no-wait-ack" => {
                wait_ack = false;
            }
            "--ack-timeout-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --ack-timeout-ms".to_string())?;
                ack_timeout_ms = value.parse::<u64>().map_err(|_| {
                    "Invalid --ack-timeout-ms, expected unsigned integer".to_string()
                })?;
            }
            "--expected-ack" => {
                expected_ack = args
                    .next()
                    .ok_or_else(|| "Missing value for --expected-ack".to_string())?;
            }
            "--serial-port" => {
                serial_port = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --serial-port".to_string())?,
                );
            }
            "--serial-baud" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --serial-baud".to_string())?;
                serial_baud = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid --serial-baud, expected unsigned integer".to_string())?;
                if serial_baud == 0 {
                    return Err("--serial-baud must be >= 1".to_string());
                }
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    if count == Some(0) {
        return Err("--count must be >= 1 when provided".to_string());
    }
    if interval_ms == 0 {
        return Err("--interval-ms must be >= 1".to_string());
    }
    if ack_timeout_ms == 0 {
        return Err("--ack-timeout-ms must be >= 1".to_string());
    }

    let payload_mode = if serial_port.is_some() {
        PayloadMode::SerialMq7
    } else {
        PayloadMode::FixedSuccess
    };

    Ok(Config {
        target,
        count,
        interval: Duration::from_millis(interval_ms),
        wait_ack,
        ack_timeout: Duration::from_millis(ack_timeout_ms),
        expected_ack,
        payload_mode,
        serial_port,
        serial_baud,
    })
}