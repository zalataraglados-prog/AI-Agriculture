use std::env;
use std::fs;
use std::time::Duration;

use serde::Deserialize;

use crate::constants::{
    DEFAULT_ACK_TIMEOUT_MS, DEFAULT_BAUD_LIST, DEFAULT_SCAN_INTERVAL_MS, DEFAULT_SCAN_WINDOW_MS,
    DEFAULT_STATE_DIR, DEFAULT_TARGET,
};

#[derive(Debug, Clone)]
pub enum GatewayCommand {
    Run(RunConfig),
    Reset(ResetConfig),
    Diag(DiagConfig),
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub config_path: Option<String>,
    pub target_override: Option<String>,
    pub state_dir: String,
    pub scan_interval: Duration,
    pub scan_window: Duration,
    pub ack_timeout: Duration,
    pub baud_list: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct ResetConfig {
    pub state_dir: String,
}

#[derive(Debug, Clone)]
pub struct DiagConfig {
    pub state_dir: String,
    pub scan_window: Duration,
    pub baud_list: Vec<u32>,
}

pub fn print_usage(binary: &str) {
    eprintln!(
        "Usage:
  {binary} run [--config <path>] [--target <ip:port>] [--state-dir <dir>] [--scan-interval-ms <ms>]
               [--scan-window-ms <ms>] [--ack-timeout-ms <ms>] [--baud-list <csv>]
  {binary} diag [--state-dir <dir>] [--scan-window-ms <ms>] [--baud-list <csv>]
  {binary} reset [--state-dir <dir>]

Defaults:
  run --target uses cached value (fallback {DEFAULT_TARGET})
  --state-dir {DEFAULT_STATE_DIR}
  --scan-interval-ms {DEFAULT_SCAN_INTERVAL_MS}
  --scan-window-ms {DEFAULT_SCAN_WINDOW_MS}
  --ack-timeout-ms {DEFAULT_ACK_TIMEOUT_MS}
  --baud-list {}

Notes:
  1) 默认命令是 run（可省略 run）。
    2) --config 支持从 TOML 读取 run 配置；命令行参数优先级更高。
    3) success 固定报文能力保留为内部诊断，不再提供外部命令入口。",
        format_baud_list(&DEFAULT_BAUD_LIST),
    );
}

pub fn parse_args() -> Result<GatewayCommand, String> {
    let mut raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() {
        return Ok(GatewayCommand::Run(default_run_config()));
    }

    if raw[0] == "-h" || raw[0] == "--help" {
        let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
        print_usage(&binary);
        std::process::exit(0);
    }

    let explicit_subcommand = match raw[0].as_str() {
        "run" | "diag" | "reset" => Some(raw.remove(0)),
        _ => None,
    };

    match explicit_subcommand.as_deref() {
        Some("run") => parse_run_args(raw).map(GatewayCommand::Run),
        Some("diag") => parse_diag_args(raw).map(GatewayCommand::Diag),
        Some("reset") => parse_reset_args(raw).map(GatewayCommand::Reset),
        Some(other) => Err(format!("Unknown subcommand: {other}")),
        None => parse_run_args(raw).map(GatewayCommand::Run),
    }
}

fn parse_run_args(raw_args: Vec<String>) -> Result<RunConfig, String> {
    let mut cfg = default_run_config();

    let args = preprocess_run_args(raw_args, &mut cfg)?;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => {
                cfg.target_override = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --target".to_string())?,
                );
            }
            "--state-dir" => {
                cfg.state_dir = args
                    .next()
                    .ok_or_else(|| "Missing value for --state-dir".to_string())?;
            }
            "--scan-interval-ms" => {
                let value = parse_u64_arg(
                    args.next(),
                    "--scan-interval-ms",
                    "Invalid --scan-interval-ms, expected unsigned integer",
                )?;
                ensure_non_zero(value, "--scan-interval-ms")?;
                cfg.scan_interval = Duration::from_millis(value);
            }
            "--scan-window-ms" => {
                let value = parse_u64_arg(
                    args.next(),
                    "--scan-window-ms",
                    "Invalid --scan-window-ms, expected unsigned integer",
                )?;
                ensure_non_zero(value, "--scan-window-ms")?;
                cfg.scan_window = Duration::from_millis(value);
            }
            "--ack-timeout-ms" => {
                let value = parse_u64_arg(
                    args.next(),
                    "--ack-timeout-ms",
                    "Invalid --ack-timeout-ms, expected unsigned integer",
                )?;
                ensure_non_zero(value, "--ack-timeout-ms")?;
                cfg.ack_timeout = Duration::from_millis(value);
            }
            "--baud-list" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --baud-list".to_string())?;
                cfg.baud_list = parse_baud_csv(&value)?;
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument for run: {arg}")),
        }
    }

    Ok(cfg)
}

fn parse_diag_args(raw_args: Vec<String>) -> Result<DiagConfig, String> {
    let mut cfg = DiagConfig {
        state_dir: DEFAULT_STATE_DIR.to_string(),
        scan_window: Duration::from_millis(DEFAULT_SCAN_WINDOW_MS),
        baud_list: DEFAULT_BAUD_LIST.to_vec(),
    };

    let mut args = raw_args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state-dir" => {
                cfg.state_dir = args
                    .next()
                    .ok_or_else(|| "Missing value for --state-dir".to_string())?;
            }
            "--scan-window-ms" => {
                let value = parse_u64_arg(
                    args.next(),
                    "--scan-window-ms",
                    "Invalid --scan-window-ms, expected unsigned integer",
                )?;
                ensure_non_zero(value, "--scan-window-ms")?;
                cfg.scan_window = Duration::from_millis(value);
            }
            "--baud-list" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --baud-list".to_string())?;
                cfg.baud_list = parse_baud_csv(&value)?;
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument for diag: {arg}")),
        }
    }

    Ok(cfg)
}

fn parse_reset_args(raw_args: Vec<String>) -> Result<ResetConfig, String> {
    let mut cfg = ResetConfig {
        state_dir: DEFAULT_STATE_DIR.to_string(),
    };

    let mut args = raw_args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state-dir" => {
                cfg.state_dir = args
                    .next()
                    .ok_or_else(|| "Missing value for --state-dir".to_string())?;
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument for reset: {arg}")),
        }
    }

    Ok(cfg)
}

#[derive(Debug, Deserialize, Default)]
struct GatewayToml {
    run: Option<RunToml>,
}

#[derive(Debug, Deserialize, Default)]
struct RunToml {
    target: Option<String>,
    state_dir: Option<String>,
    scan_interval_ms: Option<u64>,
    scan_window_ms: Option<u64>,
    ack_timeout_ms: Option<u64>,
    baud_list: Option<Vec<u32>>,
}

fn preprocess_run_args(raw_args: Vec<String>, cfg: &mut RunConfig) -> Result<Vec<String>, String> {
    let mut filtered = Vec::new();
    let mut idx = 0;

    while idx < raw_args.len() {
        if raw_args[idx] == "--config" {
            let path = raw_args
                .get(idx + 1)
                .ok_or_else(|| "Missing value for --config".to_string())?
                .clone();
            apply_run_config_toml(&path, cfg)?;
            cfg.config_path = Some(path);
            idx += 2;
            continue;
        }

        filtered.push(raw_args[idx].clone());
        idx += 1;
    }

    Ok(filtered)
}

fn apply_run_config_toml(path: &str, cfg: &mut RunConfig) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read run config file {path}: {e}"))?;
    let parsed: GatewayToml = toml::from_str(&text)
        .map_err(|e| format!("Failed to parse TOML config {path}: {e}"))?;
    let Some(run) = parsed.run else {
        return Ok(());
    };

    if let Some(target) = run.target {
        cfg.target_override = Some(target);
    }
    if let Some(state_dir) = run.state_dir {
        cfg.state_dir = state_dir;
    }
    if let Some(ms) = run.scan_interval_ms {
        ensure_non_zero(ms, "run.scan_interval_ms")?;
        cfg.scan_interval = Duration::from_millis(ms);
    }
    if let Some(ms) = run.scan_window_ms {
        ensure_non_zero(ms, "run.scan_window_ms")?;
        cfg.scan_window = Duration::from_millis(ms);
    }
    if let Some(ms) = run.ack_timeout_ms {
        ensure_non_zero(ms, "run.ack_timeout_ms")?;
        cfg.ack_timeout = Duration::from_millis(ms);
    }
    if let Some(baud_list) = run.baud_list {
        validate_baud_list(&baud_list)?;
        cfg.baud_list = baud_list;
    }

    Ok(())
}

fn default_run_config() -> RunConfig {
    RunConfig {
        config_path: None,
        target_override: None,
        state_dir: DEFAULT_STATE_DIR.to_string(),
        scan_interval: Duration::from_millis(DEFAULT_SCAN_INTERVAL_MS),
        scan_window: Duration::from_millis(DEFAULT_SCAN_WINDOW_MS),
        ack_timeout: Duration::from_millis(DEFAULT_ACK_TIMEOUT_MS),
        baud_list: DEFAULT_BAUD_LIST.to_vec(),
    }
}

fn parse_baud_csv(value: &str) -> Result<Vec<u32>, String> {
    let mut baud_list = Vec::new();
    for item in value.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let baud = trimmed
            .parse::<u32>()
            .map_err(|_| format!("Invalid baud value in --baud-list: {trimmed}"))?;
        if baud == 0 {
            return Err("--baud-list cannot contain 0".to_string());
        }
        baud_list.push(baud);
    }

    validate_baud_list(&baud_list)?;
    Ok(baud_list)
}

fn validate_baud_list(baud_list: &[u32]) -> Result<(), String> {
    if baud_list.is_empty() {
        return Err("--baud-list must contain at least one baud".to_string());
    }
    if baud_list.iter().any(|v| *v == 0) {
        return Err("--baud-list cannot contain 0".to_string());
    }
    Ok(())
}

fn format_baud_list(list: &[u32]) -> String {
    list.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_u64_arg(raw: Option<String>, name: &str, invalid_msg: &str) -> Result<u64, String> {
    let value = raw.ok_or_else(|| format!("Missing value for {name}"))?;
    value.parse::<u64>().map_err(|_| invalid_msg.to_string())
}


fn ensure_non_zero(value: u64, flag: &str) -> Result<(), String> {
    if value == 0 {
        return Err(format!("{flag} must be >= 1"));
    }
    Ok(())
}

