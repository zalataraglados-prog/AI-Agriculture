use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, BufRead, ErrorKind, Write};
use std::net::UdpSocket;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{DiagConfig, FlashConfig, GatewayCommand, RunConfig};
use crate::constants::{
    DEFAULT_DEVICE_LOOP_SLEEP_MS, DEFAULT_PAYLOAD_SUCCESS, DEFAULT_TARGET,
    RESERVED_IMAGE_FEATURE, RESERVED_IMAGE_SENSOR_ID,
};
use crate::datasource::{DataSource, NativeSensorDataSource, SerialEsp32DataSource};
use crate::persist::{
    ensure_state_dir, load_device_index, load_feature_map, load_profile, reset_state,
    save_device_index, save_feature_map, save_profile, DeviceIndexStore, FeatureMapStore,
    GatewayProfile,
};
use crate::protocol::{
    build_image_channel_packet, build_register_packet, build_sensor_packet, RegisterPayload,
};
use crate::serial::{discover_on_port, list_serial_ports};

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
    feature_map: Arc<Mutex<BTreeMap<String, String>>>,
    prompt_lock: Arc<Mutex<()>>,
}

pub fn run_command(command: GatewayCommand) -> Result<(), String> {
    match command {
        GatewayCommand::Run(cfg) => run_managed(cfg),
        GatewayCommand::Flash(cfg) => run_flash(cfg),
        GatewayCommand::Reset(cfg) => {
            reset_state(&cfg.state_dir)?;
            println!("[gateway] Reset complete: {}", cfg.state_dir);
            Ok(())
        }
        GatewayCommand::Diag(cfg) => run_diag(cfg),
    }
}

fn run_managed(run_cfg: RunConfig) -> Result<(), String> {
    let state_dir = ensure_state_dir(&run_cfg.state_dir)?;

    let mut profile = match load_profile(&state_dir)? {
        Some(existing) => existing,
        None => {
            println!("[gateway] First-time setup detected.");
            let created = prompt_initial_profile(run_cfg.target_override.as_deref())?;
            save_profile(&state_dir, &created)?;
            created
        }
    };

    if let Some(target) = run_cfg.target_override.as_ref() {
        profile.cloud_target = target.clone();
        save_profile(&state_dir, &profile)?;
    }

    let feature_store = load_feature_map(&state_dir)?;
    save_feature_map(&state_dir, &feature_store)?;
    let device_index = load_device_index(&state_dir)?;

    let shared = SharedContext {
        run_cfg: run_cfg.clone(),
        state_dir: state_dir.clone(),
        profile: Arc::new(Mutex::new(profile)),
        feature_map: Arc::new(Mutex::new(feature_store.mappings)),
        prompt_lock: Arc::new(Mutex::new(())),
    };
    let device_index = Arc::new(Mutex::new(device_index));

    println!("[gateway] Managed mode started. target is cached in state profile.");
    println!("[gateway] Recursively scanning serial ports and baud rates...");

    let mut running: HashMap<String, JoinHandle<()>> = HashMap::new();
    let mut flash_prompted_ports: BTreeSet<String> = BTreeSet::new();

    loop {
        reap_finished_sessions(&mut running);

        let ports = list_serial_ports()?;
        if ports.is_empty() {
            println!("[gateway] No serial ports found, waiting...");
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
                if !flash_prompted_ports.contains(&port) {
                    let flashed = maybe_flash_new_device(&port, &shared.prompt_lock)?;
                    if flashed {
                        flash_prompted_ports.insert(port.clone());
                    }
                }
                continue;
            }
            flash_prompted_ports.remove(&port);

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
                    "[gateway] Device discovered: {} @ {} sensors={:?}",
                    device.port, device.baud, device.sensors
                );
                let thread_port = device.port.clone();
                let thread_shared = shared.clone();
                let handle = thread::spawn(move || {
                    if let Err(err) = run_device_session_loop(device, thread_shared) {
                        eprintln!("[gateway] session ended with error: {err}");
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
                Ok(()) => println!("[gateway] Session finished, back to recursive search: {port}"),
                Err(_) => eprintln!("[gateway] Session panic, back to recursive search: {port}"),
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
            println!("[gateway] Discovery sample {port}@{baud}: {:?}", result.sample_lines);
        }

        if !result.managed_protocol_detected {
            continue;
        }

        outcome.managed_protocol_detected = true;
        println!("[gateway] Managed protocol detected on {port}@{baud}");
        outcome.detected_baud = Some(baud);

        if result.known_sensors.is_empty() && result.unknown_features.is_empty() {
            return Ok(outcome);
        }

        let mut feature_mapping_guard = shared
            .feature_map
            .lock()
            .map_err(|_| "Feature mapping lock poisoned".to_string())?;

        for feature in &result.unknown_features {
            if !feature_mapping_guard.contains_key(feature) {
                let mapped = prompt_unknown_feature_mapping(feature, &shared.prompt_lock)?;
                feature_mapping_guard.insert(feature.clone(), mapped);
            }
        }

        let mut sensors: BTreeSet<String> = result.known_sensors.iter().cloned().collect();
        for feature in &result.unknown_features {
            if let Some(sensor) = feature_mapping_guard.get(feature) {
                sensors.insert(sensor.clone());
            }
        }

        let feature_mapping_snapshot = feature_mapping_guard.clone();
        drop(feature_mapping_guard);

        save_feature_map(
            &shared.state_dir,
            &FeatureMapStore {
                mappings: feature_mapping_snapshot.clone(),
            },
        )?;

        outcome.detected_baud = Some(baud);
        outcome.sensors = sensors.into_iter().collect();
        outcome.feature_mapping = feature_mapping_snapshot;
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
        let mut source = match SerialEsp32DataSource::open(
            &device.port,
            device.baud,
            shared.feature_map.clone(),
        ) {
            Ok(s) => s,
            Err(err) => {
                eprintln!(
                    "[gateway] Failed to open {}@{}: {err}. retrying...",
                    device.port, device.baud
                );
                thread::sleep(Duration::from_millis(DEFAULT_DEVICE_LOOP_SLEEP_MS));
                return Ok(());
            }
        };

        println!("[gateway] Session online: {}", source.name());
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind local UDP socket: {e}"))?;
        socket
            .set_read_timeout(Some(shared.run_cfg.ack_timeout))
            .map_err(|e| format!("Failed to set ACK timeout: {e}"))?;

        let register_result = register_device(&socket, &device, &shared);
        if let Err(err) = register_result {
            eprintln!("[gateway] Registration failed for {}: {err}", device.device_id);
            return Ok(());
        }

        loop {
            let event = match source.next_event() {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("[gateway] Source read error on {}: {err}", source.name());
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
                        "[gateway] ACK timeout from {target}, keeping session alive for {}",
                        device.device_id
                    );
                    continue;
                }
                Err(err) => {
                    eprintln!("[gateway] ACK receive failed: {err}");
                    continue;
                }
            };

            let ack = String::from_utf8_lossy(&ack_buf[..ack_size]).trim().to_string();
            if ack == "ack:unregistered" || ack == "ack:token_invalid" {
                println!(
                    "[gateway] {} requires re-register (ack={}), re-registering...",
                    device.device_id, ack
                );
                register_device(&socket, &device, &shared)?;
                continue;
            }

            println!(
                "[gateway] {} -> {} payload=\"{}\" ACK {} from {}",
                event.feature, target, payload, ack, ack_peer
            );
        }

        println!("[gateway] Session dropped for {}, back to discovery.", device.port);
        return Ok(());
    }
}

fn register_device(
    socket: &UdpSocket,
    device: &DiscoveredDevice,
    shared: &SharedContext,
) -> Result<(), String> {
    loop {
        let (target, location, crop_type, farm_note, maybe_token) = {
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
            )
        };

        let token = if let Some(value) = maybe_token.filter(|v| !v.trim().is_empty()) {
            value
        } else {
            let prompted = prompt_line("Cloud token (1h):", None, &shared.prompt_lock)?;
            let mut profile = shared
                .profile
                .lock()
                .map_err(|_| "Profile lock poisoned".to_string())?;
            profile.last_token = Some(prompted.clone());
            save_profile(&shared.state_dir, &profile)?;
            prompted
        };

        let payload = RegisterPayload {
            device_id: device.device_id.clone(),
            location,
            crop_type,
            farm_note,
            sensors: device.sensors.clone(),
            feature_mapping: device.feature_mapping.clone(),
            token,
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

        let ack = String::from_utf8_lossy(&ack_buf[..ack_size]).trim().to_string();
        match ack.as_str() {
            "ack:register_ok" => {
                println!(
                    "[gateway] register ok: device_id={} sensors={:?}",
                    device.device_id, device.sensors
                );
                return Ok(());
            }
            "ack:token_invalid" => {
                println!("[gateway] token invalid, please input new token");
                let new_token = prompt_line("Cloud token (1h):", None, &shared.prompt_lock)?;
                let mut profile = shared
                    .profile
                    .lock()
                    .map_err(|_| "Profile lock poisoned".to_string())?;
                profile.last_token = Some(new_token);
                save_profile(&shared.state_dir, &profile)?;
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

fn run_flash(cfg: FlashConfig) -> Result<(), String> {
    let port = cfg
        .port
        .ok_or_else(|| "flash requires --port (for example /dev/ttyUSB0)".to_string())?;

    let rust_flash_script = cfg
        .fallback_script
        .clone()
        .unwrap_or_else(|| "scripts/flash_esp32_rust.sh".to_string());

    if Path::new(&rust_flash_script).exists() {
        println!("[gateway] Flash via Rust script: {rust_flash_script}");
        let status = Command::new(&rust_flash_script)
            .args([&port, &cfg.baud.to_string()])
            .status()
            .map_err(|e| format!("Failed to run rust flash script {rust_flash_script}: {e}"))?;

        if status.success() {
            println!("[gateway] Flash success via Rust script");
            return Ok(());
        }

        return Err(format!("Rust flash script exited with status {status}"));
    }

    let firmware = if let Some(path) = cfg.firmware_path.clone() {
        path
    } else {
        return Err(format!(
            "Rust flash script not found: {}. Provide script or pass --firmware <path>.",
            rust_flash_script
        ));
    };

    if !Path::new(&firmware).exists() {
        return Err(format!(
            "Firmware not found: {}. Pass valid --firmware path.",
            firmware
        ));
    }

    println!("[gateway] Flash via built-in esptool path...");
    let status = Command::new("esptool.py")
        .args([
            "--port",
            &port,
            "--baud",
            &cfg.baud.to_string(),
            "write_flash",
            "0x0",
            &firmware,
        ])
        .status();

    match status {
        Ok(st) if st.success() => {
            println!("[gateway] Flash success via esptool.py");
            return Ok(());
        }
        Ok(st) => {
            eprintln!("[gateway] esptool.py exited with status {st}");
        }
        Err(err) => {
            eprintln!("[gateway] esptool.py failed to launch: {err}");
        }
    }

    Err("Built-in binary flash failed".to_string())
}

fn run_diag(cfg: DiagConfig) -> Result<(), String> {
    let state_dir = ensure_state_dir(&cfg.state_dir)?;
    let feature_map = load_feature_map(&state_dir)?;
    let _native_placeholder = NativeSensorDataSource::new();

    println!("[gateway][diag] feature mapping loaded: {:?}", feature_map.mappings);

    let ports = list_serial_ports()?;
    if ports.is_empty() {
        println!("[gateway][diag] no serial ports detected");
        return Ok(());
    }

    for port in ports {
        println!("[gateway][diag] port={port}");
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
        None => prompt_line("Cloud target ip:port:", Some(DEFAULT_TARGET), &Arc::new(Mutex::new(())))?,
    };

    let location = prompt_line(
        "Farm location (self-describe):",
        Some("unknown_location"),
        &Arc::new(Mutex::new(())),
    )?;
    let crop_type = prompt_line("Crop type:", Some("unknown_crop"), &Arc::new(Mutex::new(())))?;
    let farm_note = prompt_line("Farm note (optional):", Some(""), &Arc::new(Mutex::new(())))?;
    let token = prompt_line("Cloud token (1h):", Some(""), &Arc::new(Mutex::new(())))?;

    Ok(GatewayProfile {
        cloud_target: target,
        farm_location: location,
        crop_type,
        farm_note,
        last_token: if token.is_empty() { None } else { Some(token) },
    })
}

fn prompt_unknown_feature_mapping(feature: &str, prompt_lock: &Arc<Mutex<()>>) -> Result<String, String> {
    prompt_line(
        &format!(
            "Unknown feature '{feature}' detected. Enter sensor id to map (e.g. mq7/dht22/adc/pcf8591):"
        ),
        Some(feature),
        prompt_lock,
    )
}

fn maybe_flash_new_device(port: &str, prompt_lock: &Arc<Mutex<()>>) -> Result<bool, String> {
    let _guard = prompt_lock
        .lock()
        .map_err(|_| "Prompt lock poisoned".to_string())?;
    println!("[gateway] New device detected on {port}, auto flashing Rust firmware...");
    let cfg = FlashConfig {
        port: Some(port.to_string()),
        baud: 921_600,
        firmware_path: None,
        fallback_script: Some("scripts/flash_esp32_rust.sh".to_string()),
    };

    match run_flash(cfg) {
        Ok(()) => {
            println!("[gateway] Flash completed on {port}");
            Ok(true)
        }
        Err(err) => {
            eprintln!("[gateway] Flash failed on {port}: {err}");
            println!("[gateway] Continue without flashing on {port}");
            Ok(false)
        }
    }
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
    let ts = SystemTime::now()
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
    format!("dev_{cleaned}_{ts}")
}

#[allow(dead_code)]
pub(crate) fn send_internal_success_probe(target: &str, ack_timeout: Duration) -> Result<(), String> {
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
    println!("[gateway][internal] success probe ACK: {ack}");
    Ok(())
}

