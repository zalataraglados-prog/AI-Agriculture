use chrono::Local;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, ErrorKind, Write};
use std::net::UdpSocket;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{DiagConfig, GatewayCommand, RunConfig};
use crate::constants::{
    DEFAULT_DEVICE_LOOP_SLEEP_MS, DEFAULT_PAYLOAD_SUCCESS, DEFAULT_TARGET, RESERVED_IMAGE_FEATURE,
    RESERVED_IMAGE_SENSOR_ID,
};
use crate::datasource::{DataSource, SerialEsp32DataSource};
use crate::persist::{
    ensure_state_dir, load_device_index, load_profile, reset_state, save_device_index,
    save_profile, DeviceIndexStore, GatewayProfile,
};
use crate::protocol::{
    build_image_channel_packet, build_register_packet, build_sensor_packet, RegisterPayload,
};
use crate::serial::{discover_on_port, list_serial_ports};

fn ts() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z").to_string()
}

fn env_non_empty(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

#[derive(Debug, Clone)]
struct DiscoveredDevice {
    port: String,
    baud: u32,
    sensors: Vec<String>,
    feature_mapping: BTreeMap<String, String>,
    device_id: String,
}

#[derive(Debug, Default, Clone)]
struct PortDiscoveryOutcome {
    port_opened: bool,
    managed_protocol_detected: bool,
    detected_baud: Option<u32>,
    sensors: Vec<String>,
    feature_mapping: BTreeMap<String, String>,
}

#[derive(Clone)]
struct SharedContext {
    run_cfg: RunConfig,
    state_dir: std::path::PathBuf,
    profile: Arc<Mutex<GatewayProfile>>,
    prompt_lock: Arc<Mutex<()>>,
}

pub fn run_command(command: GatewayCommand) -> Result<(), String> {
    match command {
        GatewayCommand::Run(cfg) => run_managed(cfg),
        GatewayCommand::Reset(cfg) => {
            reset_state(&cfg.state_dir)?;
            println!("[{}][gateway] Reset complete: {}", ts(), cfg.state_dir);
            Ok(())
        }
        GatewayCommand::Diag(cfg) => run_diag(cfg),
    }
}

fn run_managed(run_cfg: RunConfig) -> Result<(), String> {
    let state_dir = ensure_state_dir(&run_cfg.state_dir)?;
    let (mut profile, profile_is_new) = match load_profile(&state_dir)? {
        Some(existing) => (existing, false),
        None => {
            println!("[{}][gateway] First-time setup detected.", ts());
            let created = prompt_initial_profile(run_cfg.target_override.as_deref())?;
            save_profile(&state_dir, &created)?;
            (created, true)
        }
    };
    if let Some(target) = run_cfg.target_override.as_ref() {
        profile.cloud_target = target.clone();
        save_profile(&state_dir, &profile)?;
    } else if let Some(target) = env_non_empty("GATEWAY_CLOUD_TARGET") {
        profile.cloud_target = target;
        save_profile(&state_dir, &profile)?;
    }

    let prompt_lock = Arc::new(Mutex::new(()));
    if !profile_is_new {
        prompt_trial_field_info(&mut profile, &prompt_lock)?;
        save_profile(&state_dir, &profile)?;
    }

    let device_index = load_device_index(&state_dir)?;
    let shared = SharedContext {
        run_cfg: run_cfg.clone(),
        state_dir: state_dir.clone(),
        profile: Arc::new(Mutex::new(profile)),
        prompt_lock: prompt_lock.clone(),
    };
    let device_index = Arc::new(Mutex::new(device_index));

    println!(
        "[{}][gateway] Managed mode started. target is cached in state profile.",
        ts()
    );
    println!(
        "[{}][gateway] Recursively scanning serial ports and baud rates...",
        ts()
    );
    let mut running: HashMap<String, JoinHandle<()>> = HashMap::new();
    loop {
        reap_finished_sessions(&mut running);
        let ports = list_serial_ports()?;
        if ports.is_empty() {
            println!("[{}][gateway] No serial ports found, waiting...", ts());
        }
        for port in ports {
            if running.contains_key(&port) {
                continue;
            }
            let discovery = discover_device_for_port(&port, &shared)?;
            if !discovery.port_opened {
                continue;
            }
            if !discovery.managed_protocol_detected {
                continue;
            }
            if let Some(baud) = discovery.detected_baud {
                let (device_id, _) =
                    get_or_create_device_id(&port, &shared.state_dir, &device_index)?;
                let device = DiscoveredDevice {
                    port: port.clone(),
                    baud,
                    sensors: discovery.sensors,
                    feature_mapping: discovery.feature_mapping,
                    device_id,
                };
                println!(
                    "[{}][gateway] Device discovered: {} @ {} sensors={:?}",
                    ts(),
                    device.port,
                    device.baud,
                    device.sensors
                );
                let thread_port = device.port.clone();
                let thread_shared = shared.clone();
                let handle = thread::spawn(move || {
                    if let Err(err) = run_device_session_loop(device, thread_shared) {
                        eprintln!("[{}][gateway] session ended with error: {err}", ts());
                    }
                });
                running.insert(thread_port, handle);
            }
        }

        thread::sleep(run_cfg.scan_interval);
    }
}

fn reap_finished_sessions(running: &mut HashMap<String, JoinHandle<()>>) {
    let finished: Vec<String> = running
        .iter()
        .filter_map(|(port, handle)| {
            if handle.is_finished() {
                Some(port.clone())
            } else {
                None
            }
        })
        .collect();
    for port in finished {
        if let Some(handle) = running.remove(&port) {
            match handle.join() {
                Ok(()) => println!(
                    "[{}][gateway] Session finished, back to recursive search: {port}",
                    ts()
                ),
                Err(_) => eprintln!(
                    "[{}][gateway] Session panic, back to recursive search: {port}",
                    ts()
                ),
            }
        }
    }
}

fn discover_device_for_port(
    port: &str,
    shared: &SharedContext,
) -> Result<PortDiscoveryOutcome, String> {
    let mut outcome = PortDiscoveryOutcome::default();
    let mut first_opened_baud: Option<u32> = None;
    for &baud in &shared.run_cfg.baud_list {
        let result = match discover_on_port(port, baud, shared.run_cfg.scan_window) {
            Ok(v) => v,
            Err(_) => continue,
        };
        outcome.port_opened = true;
        if first_opened_baud.is_none() {
            first_opened_baud = Some(baud);
        }
        if !result.sample_lines.is_empty() {
            println!(
                "[{}][gateway] Discovery sample {port}@{baud}: {:?}",
                ts(),
                result.sample_lines
            );
        }
        if !result.managed_protocol_detected {
            continue;
        }
        outcome.managed_protocol_detected = true;
        println!(
            "[{}][gateway] Managed protocol detected on {port}@{baud}",
            ts()
        );
        outcome.detected_baud = Some(baud);
        if result.known_sensors.is_empty() {
            return Ok(outcome);
        }

        let sensors: Vec<String> = result.known_sensors.iter().cloned().collect();
        let mut feature_mapping = BTreeMap::new();
        for sensor in &sensors {
            feature_mapping.insert(sensor.clone(), sensor.clone());
        }

        outcome.sensors = sensors;
        outcome.feature_mapping = feature_mapping;
        return Ok(outcome);
    }
    if outcome.port_opened {
        outcome.managed_protocol_detected = false;
        outcome.detected_baud = first_opened_baud;
    }
    Ok(outcome)
}

fn get_or_create_device_id(
    port: &str,
    state_dir: &Path,
    device_index: &Arc<Mutex<DeviceIndexStore>>,
) -> Result<(String, bool), String> {
    let mut index_guard = device_index
        .lock()
        .map_err(|_| "Device index lock poisoned".to_string())?;
    if let Some(existing) = index_guard.port_to_device_id.get(port) {
        return Ok((existing.clone(), false));
    }
    let created = generate_device_id(port);
    index_guard
        .port_to_device_id
        .insert(port.to_string(), created.clone());
    save_device_index(state_dir, &index_guard)?;
    Ok((created, true))
}

fn run_device_session_loop(device: DiscoveredDevice, shared: SharedContext) -> Result<(), String> {
    loop {
        let mut source = match SerialEsp32DataSource::open(&device.port, device.baud) {
            Ok(s) => s,
            Err(err) => {
                eprintln!(
                    "[{}][gateway] Failed to open {}@{}: {err}. retrying...",
                    ts(),
                    device.port,
                    device.baud
                );
                thread::sleep(Duration::from_millis(DEFAULT_DEVICE_LOOP_SLEEP_MS));
                return Ok(());
            }
        };
        println!("[{}][gateway] Session online: {}", ts(), source.name());
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind local UDP socket: {e}"))?;
        socket
            .set_read_timeout(Some(shared.run_cfg.ack_timeout))
            .map_err(|e| format!("Failed to set ACK timeout: {e}"))?;
        let register_result = register_device(&socket, &device, &shared);
        if let Err(err) = register_result {
            eprintln!(
                "[{}][gateway] Registration failed for {}: {err}",
                ts(),
                device.device_id
            );
            return Ok(());
        }
        loop {
            let event = match source.next_event() {
                Ok(v) => v,
                Err(err) => {
                    eprintln!(
                        "[{}][gateway] Source read error on {}: {err}",
                        ts(),
                        source.name()
                    );
                    break;
                }
            };
            let target = {
                let guard = shared
                    .profile
                    .lock()
                    .map_err(|_| "Profile lock poisoned".to_string())?;
                guard.cloud_target.clone()
            };
            let payload = if event.sensor_id == RESERVED_IMAGE_SENSOR_ID
                || event.feature == RESERVED_IMAGE_FEATURE
            {
                build_image_channel_packet(&event.fields, &device.device_id)
            } else {
                build_sensor_packet(&event.sensor_id, &event.fields, &device.device_id)
            };
            socket
                .send_to(payload.as_bytes(), &target)
                .map_err(|e| format!("Failed to send payload to {target}: {e}"))?;
            let mut ack_buf = [0_u8; 1024];
            let (ack_size, ack_peer) = match socket.recv_from(&mut ack_buf) {
                Ok(v) => v,
                Err(err)
                    if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
                {
                    eprintln!(
                        "[{}][gateway] ACK timeout from {target}, keeping session alive for {}",
                        ts(),
                        device.device_id
                    );
                    continue;
                }
                Err(err) => {
                    eprintln!("[{}][gateway] ACK receive failed: {err}", ts());
                    continue;
                }
            };
            let ack = String::from_utf8_lossy(&ack_buf[..ack_size])
                .trim()
                .to_string();
            if ack == "ack:unregistered"
                || ack == "ack:token_invalid"
                || ack == "ack:credential_revoked"
            {
                println!(
                    "[{}][gateway] {} requires re-register (ack={}), re-registering...",
                    ts(),
                    device.device_id,
                    ack
                );
                register_device(&socket, &device, &shared)?;
                continue;
            }
            println!(
                "[{}][gateway] {} -> {} payload=\"{}\" ACK {} from {}",
                ts(),
                event.feature,
                target,
                payload,
                ack,
                ack_peer
            );
        }
        println!(
            "[{}][gateway] Session dropped for {}, back to discovery.",
            ts(),
            device.port
        );
        return Ok(());
    }
}

fn register_device(
    socket: &UdpSocket,
    device: &DiscoveredDevice,
    shared: &SharedContext,
) -> Result<(), String> {
    loop {
        let (mut target, location, crop_type, farm_note, maybe_token, maybe_device_key) = {
            let profile = shared
                .profile
                .lock()
                .map_err(|_| "Profile lock poisoned".to_string())?;
            (
                profile.cloud_target.clone(),
                profile.farm_location.clone(),
                profile.crop_type.clone(),
                profile.farm_note.clone(),
                profile.last_token.clone(),
                profile.device_key.clone(),
            )
        };
        if let Some(env_target) = env_non_empty("GATEWAY_CLOUD_TARGET") {
            target = env_target;
        }

        if target.trim().is_empty() {
            let prompted_target = prompt_line(
                "Cloud target ip:port:",
                Some(DEFAULT_TARGET),
                &shared.prompt_lock,
            )?;
            let mut profile = shared
                .profile
                .lock()
                .map_err(|_| "Profile lock poisoned".to_string())?;
            profile.cloud_target = prompted_target.clone();
            save_profile(&shared.state_dir, &profile)?;
            target = prompted_target;
        }

        let device_key = if let Some(value) = env_non_empty("GATEWAY_DEVICE_KEY") {
            Some(value)
        } else {
            maybe_device_key.filter(|v| !v.trim().is_empty())
        };

        let token = if device_key.is_none() {
            if let Some(value) = env_non_empty("GATEWAY_CLOUD_TOKEN") {
                Some(value)
            } else if let Some(value) = maybe_token.filter(|v| !v.trim().is_empty()) {
                Some(value)
            } else {
                let prompted = prompt_line("Cloud token (1h):", None, &shared.prompt_lock)?;
                let mut profile = shared
                    .profile
                    .lock()
                    .map_err(|_| "Profile lock poisoned".to_string())?;
                profile.last_token = Some(prompted.clone());
                save_profile(&shared.state_dir, &profile)?;
                Some(prompted)
            }
        } else {
            None
        };

        let payload = RegisterPayload {
            device_id: device.device_id.clone(),
            location,
            crop_type,
            farm_note,
            sensors: device.sensors.clone(),
            feature_mapping: device.feature_mapping.clone(),
            token,
            device_key: device_key.clone(),
        };
        let packet = build_register_packet(&payload)?;
        socket
            .send_to(packet.as_bytes(), &target)
            .map_err(|e| format!("Failed to send register packet: {e}"))?;
        let mut ack_buf = [0_u8; 512];
        let (ack_size, _) = match socket.recv_from(&mut ack_buf) {
            Ok(v) => v,
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                return Err("Register ACK timeout".to_string());
            }
            Err(err) => return Err(format!("Register ACK receive failed: {err}")),
        };
        let ack = String::from_utf8_lossy(&ack_buf[..ack_size])
            .trim()
            .to_string();
        if ack.starts_with("ack:register_ok") {
            if let Some(new_device_key) = extract_device_key_from_register_ack(&ack) {
                let mut profile = shared
                    .profile
                    .lock()
                    .map_err(|_| "Profile lock poisoned".to_string())?;
                profile.device_key = Some(new_device_key);
                save_profile(&shared.state_dir, &profile)?;
            }
            println!(
                "[{}][gateway] register ok: device_id={} sensors={:?}",
                ts(),
                device.device_id,
                device.sensors
            );
            return Ok(());
        }

        match ack.as_str() {
            "ack:token_invalid" => {
                if device_key.is_some() {
                    println!(
                        "[{}][gateway] device credential invalid, fallback to token bootstrap",
                        ts()
                    );
                    clear_cached_device_key(shared)?;
                    continue;
                }
                println!("[{}][gateway] token invalid, please input new token", ts());
                let new_token = prompt_line("Cloud token (1h):", None, &shared.prompt_lock)?;
                let mut profile = shared
                    .profile
                    .lock()
                    .map_err(|_| "Profile lock poisoned".to_string())?;
                profile.last_token = Some(new_token);
                save_profile(&shared.state_dir, &profile)?;
            }
            "ack:credential_revoked" => {
                println!("[{}][gateway] device credential revoked by cloud, switching to token bootstrap", ts());
                clear_cached_device_key(shared)?;
            }
            "ack:register_conflict" => {
                return Err(format!(
                    "Cloud reported register conflict for device_id {}",
                    device.device_id
                ));
            }
            other => {
                return Err(format!("Unexpected register ACK: {other}"));
            }
        }
    }
}

fn clear_cached_device_key(shared: &SharedContext) -> Result<(), String> {
    let mut profile = shared
        .profile
        .lock()
        .map_err(|_| "Profile lock poisoned".to_string())?;
    profile.device_key = None;
    save_profile(&shared.state_dir, &profile)
}

fn extract_device_key_from_register_ack(ack: &str) -> Option<String> {
    let (_, suffix) = ack.split_once(';')?;
    for pair in suffix.split(';') {
        let (key, value) = pair.split_once('=')?;
        if key.trim() == "device_key" {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn run_diag(cfg: DiagConfig) -> Result<(), String> {
    ensure_state_dir(&cfg.state_dir)?;
    let ports = list_serial_ports()?;
    if ports.is_empty() {
        println!("[{}][gateway][diag] no serial ports detected", ts());
        return Ok(());
    }
    for port in ports {
        println!("[{}][gateway][diag] port={port}", ts());
        for baud in &cfg.baud_list {
            match discover_on_port(&port, *baud, cfg.scan_window) {
                Ok(found) => {
                    if found.known_sensors.is_empty() && found.unknown_features.is_empty() {
                        continue;
                    }
                    println!(
                        "  baud={} known={:?} unknown={:?} sample={:?}",
                        baud, found.known_sensors, found.unknown_features, found.sample_lines
                    );
                }
                Err(err) => {
                    println!("  baud={} open/read failed: {}", baud, err);
                }
            }
        }
    }
    Ok(())
}

fn prompt_initial_profile(target_override: Option<&str>) -> Result<GatewayProfile, String> {
    let target = match target_override {
        Some(value) => value.to_string(),
        None => prompt_line(
            "Cloud target ip:port:",
            Some(DEFAULT_TARGET),
            &Arc::new(Mutex::new(())),
        )?,
    };
    let location = prompt_line(
        "Farm location (self-describe):",
        Some("unknown_location"),
        &Arc::new(Mutex::new(())),
    )?;
    let crop_type = prompt_line(
        "Crop type:",
        Some("unknown_crop"),
        &Arc::new(Mutex::new(())),
    )?;
    let farm_note = prompt_line("Farm note (optional):", Some(""), &Arc::new(Mutex::new(())))?;
    let token = prompt_line("Cloud token (1h):", Some(""), &Arc::new(Mutex::new(())))?;
    Ok(GatewayProfile {
        cloud_target: target,
        farm_location: location,
        crop_type,
        farm_note,
        last_token: if token.is_empty() { None } else { Some(token) },
        device_key: None,
    })
}

fn prompt_trial_field_info(
    profile: &mut GatewayProfile,
    prompt_lock: &Arc<Mutex<()>>,
) -> Result<(), String> {
    profile.farm_location = prompt_line(
        "Farm location (self-describe):",
        Some(&profile.farm_location),
        prompt_lock,
    )?;
    profile.crop_type = prompt_line("Crop type:", Some(&profile.crop_type), prompt_lock)?;
    profile.farm_note = prompt_line(
        "Farm note (optional):",
        Some(&profile.farm_note),
        prompt_lock,
    )?;
    Ok(())
}

fn prompt_line(
    message: &str,
    default: Option<&str>,
    prompt_lock: &Arc<Mutex<()>>,
) -> Result<String, String> {
    let _guard = prompt_lock
        .lock()
        .map_err(|_| "Prompt lock poisoned".to_string())?;
    print!("{}", message);
    if let Some(default_value) = default {
        if !default_value.is_empty() {
            print!(" [{}]", default_value);
        }
    }
    print!(" ");
    io::stdout()
        .flush()
        .map_err(|e| format!("Failed to flush stdout: {e}"))?;
    let mut raw = Vec::new();
    io::stdin()
        .lock()
        .read_until(b'\n', &mut raw)
        .map_err(|e| format!("Failed to read stdin: {e}"))?;
    let decoded = decode_input_lossy(&raw);
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        return Ok(default.unwrap_or("").to_string());
    }
    Ok(trimmed.to_string())
}

fn decode_input_lossy(raw: &[u8]) -> String {
    if raw.is_empty() {
        return String::new();
    }
    if raw.len() >= 2 && raw.chunks_exact(2).all(|chunk| chunk[1] == 0) {
        let mut utf16 = Vec::with_capacity(raw.len() / 2);
        for chunk in raw.chunks_exact(2) {
            utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        let text = String::from_utf16_lossy(&utf16);
        return text.replace('\0', "");
    }
    String::from_utf8_lossy(raw).replace('\0', "")
}

fn generate_device_id(port: &str) -> String {
    let ts_val = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cleaned: String = port
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(24)
        .collect();
    format!("dev_{cleaned}_{ts_val}")
}

#[allow(dead_code)]
pub(crate) fn send_internal_success_probe(
    target: &str,
    ack_timeout: Duration,
) -> Result<(), String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind socket: {e}"))?;
    socket
        .set_read_timeout(Some(ack_timeout))
        .map_err(|e| format!("Failed to set timeout: {e}"))?;
    socket
        .send_to(DEFAULT_PAYLOAD_SUCCESS.as_bytes(), target)
        .map_err(|e| format!("Failed to send success probe: {e}"))?;
    let mut buf = [0_u8; 512];
    let (size, _) = socket.recv_from(&mut buf).map_err(|e| match e.kind() {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => {
            "Success probe timeout while waiting ACK".to_string()
        }
        _ => format!("Success probe failed: {e}"),
    })?;
    let ack = String::from_utf8_lossy(&buf[..size]).trim().to_string();
    println!("[{}][gateway][internal] success probe ACK: {ack}", ts());
    Ok(())
}
