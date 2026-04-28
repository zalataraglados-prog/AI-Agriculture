use std::io::ErrorKind;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::constants::{
    DEFAULT_ACK_CREDENTIAL_REVOKED, DEFAULT_ACK_REGISTER_CONFLICT, DEFAULT_ACK_REGISTER_OK,
    DEFAULT_ACK_TOKEN_INVALID, DEFAULT_ACK_UNREGISTERED, UDP_BUFFER_SIZE,
};
use crate::db::{DbManager, SensorTelemetryDbRecord};
use crate::model::{EvalResult, RegisterOutcome, RegisterRequest, RuntimeConfig};
use crate::payload::{evaluate_payload, parse_sensor_kv_payload};
use crate::registry::{CredentialValidation, DeviceRegistry};
use crate::telemetry::{append_record, typed_fields_for_record, TelemetryRecord};
use crate::time_util::now_rfc3339;
use crate::token::validate_current_hour_token;

const ACK_HEARTBEAT: &str = "ack:heartbeat";
const ACK_BUSY: &str = "ack:busy";
const HEARTBEAT_PREFIX: &str = "heartbeat:";
const WORKER_COUNT: usize = 4;
const WORK_QUEUE_CAPACITY: usize = 2048;
const HEARTBEAT_INTERVAL_SEC: u64 = 30;
const HEARTBEAT_TIMEOUT_SEC: u64 = 90;
const HEARTBEAT_SCAN_INTERVAL_SEC: u64 = 5;

#[derive(Debug)]
struct PacketTask {
    seq: u64,
    payload: String,
    peer: SocketAddr,
}

#[derive(Debug, Default)]
struct PresenceTracker {
    last_seen_epoch_sec: std::collections::HashMap<String, u64>,
    online: std::collections::HashMap<String, bool>,
}

impl PresenceTracker {
    fn mark_online(&mut self, device_id: &str, now_sec: u64) -> bool {
        self.last_seen_epoch_sec
            .insert(device_id.to_string(), now_sec);
        let prev = self
            .online
            .insert(device_id.to_string(), true)
            .unwrap_or(false);
        !prev
    }

    fn scan_offline(&mut self, now_sec: u64, timeout_sec: u64) -> Vec<String> {
        let mut changed = Vec::new();
        for (device_id, last_seen) in &self.last_seen_epoch_sec {
            let is_online = now_sec.saturating_sub(*last_seen) <= timeout_sec;
            let prev = self.online.get(device_id).copied().unwrap_or(false);
            if prev && !is_online {
                self.online.insert(device_id.clone(), false);
                changed.push(device_id.clone());
            }
        }
        changed
    }
}

pub(crate) fn run(cfg: &RuntimeConfig, db: Arc<Mutex<DbManager>>) -> Result<(), String> {
    let socket =
        UdpSocket::bind(&cfg.bind).map_err(|e| format!("Bind failed on {}: {e}", cfg.bind))?;
    socket
        .set_read_timeout(cfg.timeout)
        .map_err(|e| format!("Failed to set read timeout: {e}"))?;

    let registry = Arc::new(Mutex::new(DeviceRegistry::load(&cfg.registry_path)?));
    let presence = Arc::new(Mutex::new(PresenceTracker::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let received_count = Arc::new(AtomicU64::new(0));
    let success_count = Arc::new(AtomicU64::new(0));

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
        "{ts} [cloud] workers={WORKER_COUNT}, queue_capacity={WORK_QUEUE_CAPACITY}, heartbeat={}s timeout={}s",
        HEARTBEAT_INTERVAL_SEC, HEARTBEAT_TIMEOUT_SEC
    );
    println!(
        "{ts} [cloud] Mode: {}",
        if cfg.once {
            "exit after first successful match"
        } else {
            "continuous/limited receive"
        }
    );

    thread::scope(|scope| -> Result<(), String> {
        let (tx, rx) = sync_channel::<PacketTask>(WORK_QUEUE_CAPACITY);
        let rx = Arc::new(Mutex::new(rx));

        for worker_id in 0..WORKER_COUNT {
            let worker_rx: Arc<Mutex<Receiver<PacketTask>>> = rx.clone();
            let worker_socket = socket
                .try_clone()
                .map_err(|e| format!("Worker socket clone failed: {e}"))?;
            let worker_registry = registry.clone();
            let worker_db = db.clone();
            let worker_stop = stop.clone();
            let worker_success = success_count.clone();

            scope.spawn(move || {
                loop {
                    if worker_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let task = {
                        let guard = match worker_rx.lock() {
                            Ok(v) => v,
                            Err(_) => break,
                        };
                        match guard.recv() {
                            Ok(v) => v,
                            Err(_) => break,
                        }
                    };

                    if let Some(json_text) = task.payload.strip_prefix("register:") {
                        let ack = {
                            let mut guard = match worker_registry.lock() {
                                Ok(v) => v,
                                Err(_) => continue,
                            };
                            handle_register(json_text, cfg, &mut guard)
                        };
                        let _ = worker_socket.send_to(ack.as_bytes(), task.peer);
                        println!(
                            "{} [cloud] Packet #{} from {}: register request => ACK=\"{}\"",
                            now_rfc3339(),
                            task.seq,
                            task.peer,
                            redact_register_ack_for_log(&ack)
                        );
                        if ack == DEFAULT_ACK_REGISTER_OK {
                            worker_success.fetch_add(1, Ordering::Relaxed);
                            if cfg.once {
                                worker_stop.store(true, Ordering::Relaxed);
                            }
                        }
                        continue;
                    }

                    let result = {
                        let guard = match worker_registry.lock() {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        evaluate_data_packet(&task.payload, cfg, &guard)
                    };

                    if result.matched {
                        worker_success.fetch_add(1, Ordering::Relaxed);
                        if let Err(err) = persist_matched_telemetry(&task.payload, cfg, worker_db.clone())
                        {
                            eprintln!(
                                "{} [cloud] WARN: failed to persist telemetry: {err}",
                                now_rfc3339()
                            );
                        }
                        if cfg.once {
                            worker_stop.store(true, Ordering::Relaxed);
                        }
                    }

                    let _ = worker_socket.send_to(result.ack.as_bytes(), task.peer);
                    println!(
                        "{} [cloud] worker#{worker_id} packet #{} from {}: \"{}\" => {} ; ACK=\"{}\" ; {}",
                        now_rfc3339(),
                        task.seq,
                        task.peer,
                        task.payload,
                        if result.matched { "MATCH" } else { "MISMATCH" },
                        result.ack,
                        result.detail
                    );
                }
            });
        }

        {
            let tracker = presence.clone();
            let monitor_stop = stop.clone();
            scope.spawn(move || loop {
                if monitor_stop.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_secs(HEARTBEAT_SCAN_INTERVAL_SEC));
                let now_sec = now_epoch_sec();
                let changed = {
                    let mut guard = match tracker.lock() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    guard.scan_offline(now_sec, HEARTBEAT_TIMEOUT_SEC)
                };
                for device_id in changed {
                    println!(
                        "{} [cloud] presence: device_id={} => OFFLINE (heartbeat timeout {}s)",
                        now_rfc3339(),
                        device_id,
                        HEARTBEAT_TIMEOUT_SEC
                    );
                }
            });
        }

        let mut buf = [0_u8; UDP_BUFFER_SIZE];
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            let (size, peer) = match socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(err)
                    if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
                {
                    return Err("Receive timeout reached without enough packets".to_string());
                }
                Err(err) => return Err(format!("Receive failed: {err}")),
            };

            let seq = received_count.fetch_add(1, Ordering::Relaxed) + 1;
            let payload = String::from_utf8_lossy(&buf[..size]).trim().to_string();

            if let Some(device_id) = parse_heartbeat_device_id(&payload) {
                let now_sec = now_epoch_sec();
                let became_online = {
                    let mut guard = presence
                        .lock()
                        .map_err(|_| "presence lock poisoned".to_string())?;
                    guard.mark_online(&device_id, now_sec)
                };
                socket
                    .send_to(ACK_HEARTBEAT.as_bytes(), peer)
                    .map_err(|e| format!("ACK send failed to {peer}: {e}"))?;
                if became_online {
                    println!(
                        "{} [cloud] presence: device_id={} => ONLINE",
                        now_rfc3339(),
                        device_id
                    );
                }
                continue;
            }

            match tx.try_send(PacketTask { seq, payload, peer }) {
                Ok(_) => {}
                Err(TrySendError::Full(task)) => {
                    let _ = socket.send_to(ACK_BUSY.as_bytes(), task.peer);
                    eprintln!(
                        "{} [cloud] queue full: packet #{} from {} dropped with ACK=\"{}\"",
                        now_rfc3339(),
                        task.seq,
                        task.peer,
                        ACK_BUSY
                    );
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err("worker queue disconnected".to_string());
                }
            }

            if let Some(max) = cfg.max_packets {
                if seq >= max {
                    break;
                }
            }
        }

        stop.store(true, Ordering::Relaxed);
        drop(tx);
        Ok(())
    })?;

    let received = received_count.load(Ordering::Relaxed);
    let matched = success_count.load(Ordering::Relaxed);
    println!(
        "{} [cloud] Summary: received={received}, matched={matched}",
        now_rfc3339()
    );

    if matched == 0 {
        return Err("No matching packet received".to_string());
    }

    Ok(())
}

fn now_epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_heartbeat_device_id(payload: &str) -> Option<String> {
    let body = payload.strip_prefix(HEARTBEAT_PREFIX)?;
    for part in body.split(',') {
        let (k, v) = part.split_once('=')?;
        if k.trim() == "device_id" {
            let id = v.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn redact_register_ack_for_log(ack: &str) -> String {
    if ack.starts_with("ack:register_ok;device_key=") {
        "ack:register_ok;device_key=***".to_string()
    } else {
        ack.to_string()
    }
}
//Mark 01
fn handle_register(json_text: &str, cfg: &RuntimeConfig, registry: &mut DeviceRegistry) -> String {
    let request = match serde_json::from_str::<RegisterRequest>(json_text) {
        Ok(v) => v,
        Err(_) => return cfg.ack_mismatch.clone(),
    };
    let allowed_sensor_ids = cfg.sensor_rules.keys().cloned().collect();

    if let Some(device_key) = request
        .device_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let device_id = request.device_id.clone();
        match registry.validate_device_credential(&device_id, device_key) {
            CredentialValidation::Valid => {
                return match registry.register_device_with_credential(request, &allowed_sensor_ids)
                {
                    Ok(RegisterOutcome::Ok) => DEFAULT_ACK_REGISTER_OK.to_string(),
                    Ok(RegisterOutcome::Conflict) => DEFAULT_ACK_REGISTER_CONFLICT.to_string(),
                    Err(_) => cfg.ack_mismatch.clone(),
                };
            }
            CredentialValidation::Revoked => return DEFAULT_ACK_CREDENTIAL_REVOKED.to_string(),
            CredentialValidation::Invalid => return DEFAULT_ACK_TOKEN_INVALID.to_string(),
        }
    }

    let candidate_token = request.token.as_deref().map(str::trim).unwrap_or("");
    if candidate_token.is_empty() {
        return DEFAULT_ACK_TOKEN_INVALID.to_string();
    }

    let token_ok = match validate_current_hour_token(&cfg.token_store_path, candidate_token) {
        Ok(v) => v,
        Err(_) => return cfg.ack_mismatch.clone(),
    };

    if !token_ok {
        return DEFAULT_ACK_TOKEN_INVALID.to_string();
    }

    match registry.register_device_with_token(request, &allowed_sensor_ids) {
        Ok((RegisterOutcome::Ok, issued_key)) => {
            if let Some(device_key) = issued_key {
                format!("{DEFAULT_ACK_REGISTER_OK};device_key={device_key}")
            } else {
                DEFAULT_ACK_REGISTER_OK.to_string()
            }
        }
        Ok((RegisterOutcome::Conflict, _)) => DEFAULT_ACK_REGISTER_CONFLICT.to_string(),
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

fn persist_matched_telemetry(
    payload: &str,
    cfg: &RuntimeConfig,
    db: Arc<Mutex<DbManager>>,
) -> Result<(), String> {
    let (sensor_id, raw_fields) = parse_sensor_kv_payload(payload)?;
    let device_id = raw_fields
        .get("device_id")
        .cloned()
        .ok_or_else(|| "missing device_id for telemetry persistence".to_string())?;

    let typed_fields = typed_fields_for_record(&sensor_id, &raw_fields, cfg);
    let record = TelemetryRecord {
        ts: now_rfc3339(),
        device_id: device_id.clone(),
        sensor_id: sensor_id.clone(),
        fields: typed_fields,
    };

    let ts = chrono::DateTime::parse_from_rfc3339(&record.ts)
        .map(|v| v.with_timezone(&chrono::Utc))
        .map_err(|e| format!("invalid telemetry timestamp format: {e}"))?;
    let db_row = SensorTelemetryDbRecord {
        ts,
        device_id,
        sensor_id,
        fields_json: serde_json::to_value(&record.fields)
            .map_err(|e| format!("failed to serialize telemetry fields for db: {e}"))?,
    };

    db.lock()
        .map_err(|_| "db lock poisoned".to_string())
        .and_then(|mut guard| guard.insert_sensor_telemetry(&db_row))?;

    append_record(&cfg.telemetry_store_path, &record)
}
