use std::io::ErrorKind;
use std::net::UdpSocket;

use crate::constants::{
    DEFAULT_ACK_REGISTER_CONFLICT, DEFAULT_ACK_REGISTER_OK, DEFAULT_ACK_TOKEN_INVALID,
    DEFAULT_ACK_UNREGISTERED, UDP_BUFFER_SIZE,
};
use crate::model::{EvalResult, RegisterOutcome, RegisterRequest, RuntimeConfig};
use crate::payload::{evaluate_payload, parse_sensor_kv_payload};
use crate::registry::DeviceRegistry;
use crate::telemetry::{append_record, typed_fields_for_record, TelemetryRecord};
use crate::time_util::now_rfc3339;
use crate::token::validate_current_hour_token;

pub(crate) fn run(cfg: &RuntimeConfig) -> Result<(), String> {
    let socket =
        UdpSocket::bind(&cfg.bind).map_err(|e| format!("Bind failed on {}: {e}", cfg.bind))?;
    //socket
    //   .set_read_timeout(cfg.timeout)
    //   .map_err(|e| format!("Failed to set read timeout: {e}"))?;

    let mut registry = DeviceRegistry::load(&cfg.registry_path)?;

    let ts = now_rfc3339();
    println!("{ts} [cloud] Listening on {}", cfg.bind);
    println!(
        "{ts} [cloud] Loaded rules: exact={}, sensors={}",
        cfg.exact_rules.len(),
        cfg.sensor_rules.len()
    );
    println!(
        "{ts} [cloud] ACK defaults: mismatch=\"{}\", unknown_sensor=\"{}\"",
        cfg.ack_mismatch, cfg.ack_unknown_sensor
    );
    println!("{ts} [cloud] registry={}", cfg.registry_path);
    println!("{ts} [cloud] token_store={}", cfg.token_store_path);
    println!("{ts} [cloud] telemetry_store={}", cfg.telemetry_store_path);
    println!(
        "{ts} [cloud] Mode: {}",
        if cfg.once {
            "exit after first successful match"
        } else {
            "continuous/limited receive"
        }
    );

    let mut buf = [0_u8; UDP_BUFFER_SIZE];
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

        if let Some(json_text) = payload.strip_prefix("register:") {
            let ack = handle_register(json_text, cfg, &mut registry);
            socket
                .send_to(ack.as_bytes(), peer)
                .map_err(|e| format!("ACK send failed to {peer}: {e}"))?;

            println!(
                "{} [cloud] Packet #{received_count} from {peer}: register request => ACK=\"{}\"",
                now_rfc3339(),
                ack
            );

            if ack == DEFAULT_ACK_REGISTER_OK {
                success_count += 1;
            }

            if cfg.once && ack == DEFAULT_ACK_REGISTER_OK {
                break;
            }
            if let Some(max) = cfg.max_packets {
                if received_count >= max {
                    break;
                }
            }
            continue;
        }

        let result = evaluate_data_packet(&payload, cfg, &registry);

        if result.matched {
            success_count += 1;
            if let Err(err) = persist_matched_telemetry(&payload, cfg) {
                eprintln!(
                    "{} [cloud] WARN: failed to persist telemetry: {err}",
                    now_rfc3339()
                );
            }
        }

        socket
            .send_to(result.ack.as_bytes(), peer)
            .map_err(|e| format!("ACK send failed to {peer}: {e}"))?;

        println!(
            "{} [cloud] Packet #{received_count} from {peer}: \"{payload}\" => {} ; ACK=\"{}\" ; {}",
            now_rfc3339(),
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

    println!(
        "{} [cloud] Summary: received={received_count}, matched={success_count}",
        now_rfc3339()
    );

    if success_count == 0 {
        return Err("No matching packet received".to_string());
    }

    Ok(())
}
//Mark 01
fn handle_register(json_text: &str, cfg: &RuntimeConfig, registry: &mut DeviceRegistry) -> String {
    let request = match serde_json::from_str::<RegisterRequest>(json_text) {
        Ok(v) => v,
        Err(_) => return cfg.ack_mismatch.clone(),
    };

    let token_ok = match validate_current_hour_token(&cfg.token_store_path, &request.token) {
        Ok(v) => v,
        Err(_) => return cfg.ack_mismatch.clone(),
    };

    if !token_ok {
        return DEFAULT_ACK_TOKEN_INVALID.to_string();
    }

    let allowed_sensor_ids = cfg.sensor_rules.keys().cloned().collect();
    match registry.register_device(request, &allowed_sensor_ids) {
        Ok(RegisterOutcome::Ok) => DEFAULT_ACK_REGISTER_OK.to_string(),
        Ok(RegisterOutcome::Conflict) => DEFAULT_ACK_REGISTER_CONFLICT.to_string(),
        Err(_) => cfg.ack_mismatch.clone(),
    }
}

fn evaluate_data_packet(
    payload: &str,
    cfg: &RuntimeConfig,
    registry: &DeviceRegistry,
) -> EvalResult {
    if let Ok((sensor_id, fields)) = parse_sensor_kv_payload(payload) {
        let device_id = match fields.get("device_id") {
            Some(value) if !value.trim().is_empty() => value,
            _ => {
                return EvalResult {
                    matched: false,
                    ack: DEFAULT_ACK_UNREGISTERED.to_string(),
                    detail: "missing required field device_id".to_string(),
                };
            }
        };

        if !registry.is_registered(device_id) {
            return EvalResult {
                matched: false,
                ack: DEFAULT_ACK_UNREGISTERED.to_string(),
                detail: format!("device_id is not registered: {device_id}"),
            };
        }

        if !registry.is_sensor_allowed_for_device(device_id, &sensor_id) {
            return EvalResult {
                matched: false,
                ack: DEFAULT_ACK_UNREGISTERED.to_string(),
                detail: format!(
                    "sensor_id {sensor_id} is not registered for device_id {device_id}"
                ),
            };
        }
    }

    evaluate_payload(payload, cfg)
}

fn persist_matched_telemetry(payload: &str, cfg: &RuntimeConfig) -> Result<(), String> {
    let (sensor_id, raw_fields) = parse_sensor_kv_payload(payload)?;
    let device_id = raw_fields
        .get("device_id")
        .cloned()
        .ok_or_else(|| "missing device_id for telemetry persistence".to_string())?;

    let typed_fields = typed_fields_for_record(&sensor_id, &raw_fields, cfg);
    let record = TelemetryRecord {
        ts: now_rfc3339(),
        device_id,
        sensor_id,
        fields: typed_fields,
    };

    append_record(&cfg.telemetry_store_path, &record)
}
