use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::constants::DEFAULT_ACK_MATCH_LEGACY;
use crate::model::{CliConfig, ConfigFile, RuntimeConfig, SensorRule, TokenCliConfig};

pub(crate) fn parse_config_file(content: &str) -> Result<ConfigFile, String> {
    toml::from_str(content).map_err(|e| format!("Failed to parse TOML config: {e}"))
}

pub(crate) fn build_runtime_config(
    cli: &CliConfig,
    file_cfg: ConfigFile,
) -> Result<RuntimeConfig, String> {
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

    if let Some(legacy_expected) = cli.legacy_expected.as_ref() {
        let ack = cli
            .legacy_ack_match
            .clone()
            .unwrap_or_else(|| DEFAULT_ACK_MATCH_LEGACY.to_string());
        exact_rules.insert(legacy_expected.clone(), ack);
    }

    let bind = cli
        .bind_override
        .clone()
        .unwrap_or_else(|| file_cfg.receiver.bind.clone());

    let once = cli
        .once
        .unwrap_or_else(|| file_cfg.receiver.once.unwrap_or(false));

    let timeout = match cli.timeout_override {
        Some(override_value) => override_value,
        None => file_cfg.receiver.timeout_ms.map(Duration::from_millis),
    };

    let ack_mismatch = cli
        .ack_mismatch_override
        .clone()
        .unwrap_or_else(|| file_cfg.receiver.ack_mismatch.clone());

    let ack_unknown_sensor = cli
        .ack_unknown_sensor_override
        .clone()
        .unwrap_or_else(|| file_cfg.receiver.ack_unknown_sensor.clone());

    let token_store_path = cli
        .token_store_path_override
        .clone()
        .unwrap_or_else(|| file_cfg.receiver.token_store_path.clone());

    let registry_path = cli
        .registry_path_override
        .clone()
        .unwrap_or_else(|| file_cfg.receiver.registry_path.clone());
    let telemetry_store_path = file_cfg.receiver.telemetry_store_path.clone();

    ensure_parent_dir(&token_store_path)?;
    ensure_parent_dir(&registry_path)?;
    ensure_parent_dir(&telemetry_store_path)?;

    Ok(RuntimeConfig {
        bind,
        once,
        max_packets: cli.max_packets,
        timeout,
        ack_mismatch,
        ack_unknown_sensor,
        token_store_path,
        registry_path,
        telemetry_store_path,
        exact_rules,
        sensor_rules,
    })
}

pub(crate) fn load_runtime_config(cli: CliConfig) -> Result<RuntimeConfig, String> {
    let content = fs::read_to_string(&cli.config_path)
        .map_err(|e| format!("Failed to read config file {}: {e}", cli.config_path))?;
    let file_cfg = parse_config_file(&content)?;
    build_runtime_config(&cli, file_cfg)
}

pub(crate) fn resolve_token_store_path_for_token_command(
    cli: &TokenCliConfig,
) -> Result<String, String> {
    if let Some(path) = &cli.token_store_path_override {
        return Ok(path.clone());
    }

    let content = fs::read_to_string(&cli.config_path)
        .map_err(|e| format!("Failed to read config file {}: {e}", cli.config_path))?;
    let file_cfg = parse_config_file(&content)?;
    Ok(resolve_relative_path_from_config(
        &cli.config_path,
        &file_cfg.receiver.token_store_path,
    ))
}

fn resolve_relative_path_from_config(config_path: &str, raw_path: &str) -> String {
    let raw = Path::new(raw_path);
    if raw.is_absolute() {
        return raw_path.to_string();
    }

    let config = Path::new(config_path);
    let config_dir = config.parent().unwrap_or_else(|| Path::new("."));

    let base_dir: PathBuf = if config_dir
        .file_name()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("config"))
        .unwrap_or(false)
    {
        config_dir.parent().unwrap_or(config_dir).to_path_buf()
    } else {
        config_dir.to_path_buf()
    };

    base_dir.join(raw).to_string_lossy().to_string()
}

fn ensure_parent_dir(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent dir {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_relative_path_from_config;

    #[test]
    fn resolve_relative_path_from_config_dir_parent_when_config_folder() {
        let resolved = resolve_relative_path_from_config(
            "/opt/ai-agriculture/cloud/config/sensors.toml",
            "state/token_store.json",
        );
        assert_eq!(resolved, "/opt/ai-agriculture/cloud/state/token_store.json");
    }

    #[test]
    fn keep_absolute_path_unchanged() {
        let resolved = resolve_relative_path_from_config(
            "/opt/ai-agriculture/cloud/config/sensors.toml",
            "/opt/ai-agriculture/cloud/state/token_store.json",
        );
        assert_eq!(resolved, "/opt/ai-agriculture/cloud/state/token_store.json");
    }
}
