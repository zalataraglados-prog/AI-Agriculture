use chrono::Local;
use rand::seq::SliceRandom;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, BufRead, ErrorKind, Write};
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone)]
struct ImageUploadContext {
    image_dir: PathBuf,
    upload_url: String,
    upload_interval: Duration,
}

#[derive(Debug, Clone)]
struct ImageCyclePicker {
    files: Vec<PathBuf>,
    order: Vec<usize>,
    cursor: usize,
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
    let default_target = resolve_default_target();
    let (mut profile, profile_is_new) = match load_profile(&state_dir)? {
        Some(existing) => (existing, false),
        None => {
            println!("[{}][gateway] First-time setup detected.", ts());
            let created = prompt_initial_profile(
                run_cfg.target_override.as_deref(),
                default_target.as_str(),
            )?;
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
    let image_upload_ctx = build_image_upload_context(&run_cfg, &profile)?;
    if let Some(ctx) = image_upload_ctx.as_ref() {
        println!(
            "[{}][gateway] Image simulator enabled: dir={} interval_ms={} upload={}",
            ts(),
            ctx.image_dir.display(),
            ctx.upload_interval.as_millis(),
            ctx.upload_url
        );
    } else {
        println!(
            "[{}][gateway] Image simulator disabled (set --image-dir or GATEWAY_IMAGE_DIR to enable).",
            ts()
        );
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
        let ports = list_serial_ports(&shared.run_cfg.modbus)?;
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
                let thread_image_ctx = image_upload_ctx.clone();
                let handle = thread::spawn(move || {
                    if let Err(err) =
                        run_device_session_loop(device, thread_shared, thread_image_ctx)
                    {
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
        let result = match discover_on_port(
            port,
            baud,
            shared.run_cfg.scan_window,
            &shared.run_cfg.modbus,
        ) {
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

impl ImageCyclePicker {
    fn from_dir(root: &Path) -> Result<Self, String> {
        let mut files = Vec::new();
        collect_image_files(root, &mut files)?;
        if files.is_empty() {
            return Err(format!(
                "no .jpg/.jpeg/.png images found under {}",
                root.display()
            ));
        }
        let mut order: Vec<usize> = (0..files.len()).collect();
        order.shuffle(&mut rand::rng());
        Ok(Self {
            files,
            order,
            cursor: 0,
        })
    }

    fn next_path(&mut self) -> Option<&PathBuf> {
        if self.files.is_empty() {
            return None;
        }
        if self.cursor >= self.order.len() {
            self.cursor = 0;
            self.order.shuffle(&mut rand::rng());
        }
        let idx = *self.order.get(self.cursor)?;
        self.cursor += 1;
        self.files.get(idx)
    }
}

fn collect_image_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|e| format!("failed to read image dir {}: {e}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read dir entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_image_files(&path, out)?;
            continue;
        }
        if is_supported_image_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_supported_image_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|v| v.to_str()) else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png")
}

fn build_image_upload_context(
    run_cfg: &RunConfig,
    profile: &GatewayProfile,
) -> Result<Option<ImageUploadContext>, String> {
    let Some(image_dir_raw) = run_cfg.image_dir.as_ref() else {
        return Ok(None);
    };
    let image_dir = PathBuf::from(image_dir_raw);
    if !image_dir.exists() || !image_dir.is_dir() {
        return Err(format!(
            "image dir not found or not a directory: {}",
            image_dir.display()
        ));
    }

    let upload_url = match run_cfg.image_upload_url.as_ref() {
        Some(v) => v.clone(),
        None => derive_image_upload_url(&profile.cloud_target, run_cfg),
    };

    Ok(Some(ImageUploadContext {
        image_dir,
        upload_url,
        upload_interval: run_cfg.image_upload_interval,
    }))
}

fn derive_image_upload_url(udp_target: &str, run_cfg: &RunConfig) -> String {
    let host = udp_target
        .split(':')
        .next()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("127.0.0.1");
    format!(
        "{}://{}:{}{}",
        run_cfg.image_upload_scheme, host, run_cfg.image_upload_port, run_cfg.image_upload_path
    )
}

fn try_upload_cycle_image(
    ctx: &ImageUploadContext,
    picker: &mut ImageCyclePicker,
    device: &DiscoveredDevice,
    shared: &SharedContext,
) -> Result<(), String> {
    let Some(path) = picker.next_path().cloned() else {
        return Ok(());
    };
    let profile = shared
        .profile
        .lock()
        .map_err(|_| "Profile lock poisoned".to_string())?
        .clone();
    let ts_rfc3339 = Local::now().to_rfc3339();
    let bytes = fs::read(&path)
        .map_err(|e| format!("failed to read image file {}: {e}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("image.bin")
        .to_string();
    let mime = match path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
    {
        Some(ext) if ext == "png" => "image/png",
        Some(ext) if ext == "jpg" || ext == "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    };

    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(file_name.clone())
        .mime_str(mime)
        .map_err(|e| format!("failed to build multipart image part: {e}"))?;
    let form = reqwest::blocking::multipart::Form::new().part("file", part);

    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("failed to create image upload http client: {e}"))?
        .post(&ctx.upload_url)
        .query(&[
            ("device_id", device.device_id.as_str()),
            ("ts", ts_rfc3339.as_str()),
            ("location", profile.farm_location.as_str()),
            ("crop_type", profile.crop_type.as_str()),
            ("farm_note", profile.farm_note.as_str()),
        ])
        .multipart(form)
        .send()
        .map_err(|e| format!("failed to upload image {}: {e}", path.display()))?;
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|_| String::from("<read-body-failed>"));
    if !status.is_success() {
        return Err(format!(
            "image upload http {} for {} body={}",
            status,
            path.display(),
            truncate_for_log(&body)
        ));
    }
    println!(
        "[{}][gateway] image upload ok: device={} file={} -> {} body={}",
        ts(),
        device.device_id,
        path.display(),
        ctx.upload_url,
        truncate_for_log(&body)
    );
    Ok(())
}

fn truncate_for_log(raw: &str) -> String {
    const MAX_LEN: usize = 200;
    if raw.chars().count() <= MAX_LEN {
        return raw.to_string();
    }
    let short: String = raw.chars().take(MAX_LEN).collect();
    format!("{short}...(truncated)")
}

fn run_device_session_loop(
    device: DiscoveredDevice,
    shared: SharedContext,
    image_upload_ctx: Option<ImageUploadContext>,
) -> Result<(), String> {
    loop {
        let mut source =
            match SerialEsp32DataSource::open(&device.port, device.baud, &shared.run_cfg.modbus) {
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
        let mut image_picker = match image_upload_ctx.as_ref() {
            Some(ctx) => match ImageCyclePicker::from_dir(&ctx.image_dir) {
                Ok(v) => Some(v),
                Err(err) => {
                    eprintln!(
                        "[{}][gateway] image simulator disabled for {}: {err}",
                        ts(),
                        device.device_id
                    );
                    None
                }
            },
            None => None,
        };
        let mut next_image_due = Instant::now();
        loop {
            if let (Some(ctx), Some(picker)) = (image_upload_ctx.as_ref(), image_picker.as_mut()) {
                if Instant::now() >= next_image_due {
                    if let Err(err) = try_upload_cycle_image(ctx, picker, &device, &shared) {
                        eprintln!("[{}][gateway] image upload warning: {err}", ts());
                    }
                    next_image_due = Instant::now() + ctx.upload_interval;
                }
            }
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
            let target_default = resolve_default_target();
            let prompted_target = prompt_non_empty_line(
                "Cloud target ip:port:",
                if target_default.is_empty() {
                    None
                } else {
                    Some(target_default.as_str())
                },
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
    let ports = list_serial_ports(&cfg.modbus)?;
    if ports.is_empty() {
        println!("[{}][gateway][diag] no serial ports detected", ts());
        return Ok(());
    }
    for port in ports {
        println!("[{}][gateway][diag] port={port}", ts());
        for baud in &cfg.baud_list {
            match discover_on_port(&port, *baud, cfg.scan_window, &cfg.modbus) {
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

fn prompt_initial_profile(
    target_override: Option<&str>,
    default_target: &str,
) -> Result<GatewayProfile, String> {
    let target = match target_override {
        Some(value) => value.to_string(),
        None => prompt_non_empty_line(
            "Cloud target ip:port:",
            if default_target.is_empty() {
                None
            } else {
                Some(default_target)
            },
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

fn resolve_default_target() -> String {
    env_non_empty("GATEWAY_DEFAULT_TARGET").unwrap_or_else(|| DEFAULT_TARGET.to_string())
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

fn prompt_non_empty_line(
    message: &str,
    default: Option<&str>,
    prompt_lock: &Arc<Mutex<()>>,
) -> Result<String, String> {
    loop {
        let value = prompt_line(message, default, prompt_lock)?;
        if !value.trim().is_empty() {
            return Ok(value);
        }
        println!("[{}][gateway] value is required for {}", ts(), message);
    }
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

#[cfg(test)]
mod tests {
    use super::{
        derive_image_upload_url, try_upload_cycle_image, DiscoveredDevice, ImageCyclePicker,
        ImageUploadContext, SharedContext,
    };
    use crate::config::RunConfig;
    use crate::persist::GatewayProfile;
    use crate::serial::ModbusConfig;
    use std::collections::{BTreeMap, HashSet};
    use std::fs;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|v| v.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("gateway_image_picker_{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn derive_upload_url_from_udp_target() {
        let cfg = RunConfig {
            config_path: None,
            target_override: None,
            state_dir: "state".to_string(),
            scan_interval: Duration::from_secs(1),
            scan_window: Duration::from_millis(500),
            ack_timeout: Duration::from_millis(500),
            baud_list: vec![9600],
            image_dir: None,
            image_upload_url: None,
            image_upload_interval: Duration::from_secs(300),
            image_upload_scheme: "http".to_string(),
            image_upload_port: 8088,
            image_upload_path: "/api/v1/image/upload".to_string(),
            modbus: ModbusConfig::default(),
        };
        assert_eq!(
            derive_image_upload_url("8.134.32.223:9000", &cfg),
            "http://8.134.32.223:8088/api/v1/image/upload"
        );
        assert_eq!(
            derive_image_upload_url("10.72.40.186:7777", &cfg),
            "http://10.72.40.186:8088/api/v1/image/upload"
        );
    }

    #[test]
    fn cycle_picker_no_repeat_per_round() {
        let root = make_temp_dir();
        fs::write(root.join("a.jpg"), [1_u8, 2, 3]).expect("write a");
        fs::write(root.join("b.png"), [4_u8, 5, 6]).expect("write b");
        fs::write(root.join("c.jpeg"), [7_u8, 8, 9]).expect("write c");

        let mut picker = ImageCyclePicker::from_dir(&root).expect("create picker");
        let mut first_round = HashSet::new();
        for _ in 0..3 {
            let next = picker.next_path().expect("next path");
            first_round.insert(next.file_name().unwrap().to_string_lossy().to_string());
        }
        assert_eq!(first_round.len(), 3);

        let mut second_round = HashSet::new();
        for _ in 0..3 {
            let next = picker.next_path().expect("next path");
            second_round.insert(next.file_name().unwrap().to_string_lossy().to_string());
        }
        assert_eq!(second_round.len(), 3);

        fs::remove_dir_all(root).ok();
    }

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn image_upload_multipart_keeps_original_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).expect("read header line");
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("Content-Length") {
                        content_length = value.trim().parse::<usize>().expect("content length");
                    }
                }
            }
            let mut body = vec![0_u8; content_length];
            reader.read_exact(&mut body).expect("read body");
            tx.send(body).expect("send captured body");
            let mut writer = stream;
            let resp = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 20\r\n\r\n{\"status\":\"success\"}";
            writer.write_all(resp).expect("write response");
            writer.flush().expect("flush");
        });

        let root = make_temp_dir();
        let image_bytes: Vec<u8> = (0..=255).collect();
        let image_path = root.join("sample.jpg");
        fs::write(&image_path, &image_bytes).expect("write image");

        let mut picker = ImageCyclePicker::from_dir(&root).expect("create picker");
        let ctx = ImageUploadContext {
            image_dir: root.clone(),
            upload_url: format!("http://{}/api/v1/image/upload", addr),
            upload_interval: Duration::from_secs(300),
        };
        let device = DiscoveredDevice {
            port: "test-port".to_string(),
            baud: 9600,
            sensors: vec!["soil_modbus_02".to_string()],
            feature_mapping: BTreeMap::new(),
            device_id: "dev_test_image_integrity".to_string(),
        };
        let shared = SharedContext {
            run_cfg: RunConfig {
                config_path: None,
                target_override: None,
                state_dir: "state".to_string(),
                scan_interval: Duration::from_secs(1),
                scan_window: Duration::from_millis(500),
                ack_timeout: Duration::from_millis(500),
                baud_list: vec![9600],
                image_dir: Some(root.display().to_string()),
                image_upload_url: Some(ctx.upload_url.clone()),
                image_upload_interval: Duration::from_secs(300),
                image_upload_scheme: "http".to_string(),
                image_upload_port: 8088,
                image_upload_path: "/api/v1/image/upload".to_string(),
                modbus: ModbusConfig::default(),
            },
            state_dir: PathBuf::from("state"),
            profile: Arc::new(Mutex::new(GatewayProfile {
                cloud_target: "127.0.0.1:9000".to_string(),
                farm_location: "lab".to_string(),
                crop_type: "rice".to_string(),
                farm_note: "integrity_test".to_string(),
                last_token: None,
                device_key: None,
            })),
            prompt_lock: Arc::new(Mutex::new(())),
        };

        try_upload_cycle_image(&ctx, &mut picker, &device, &shared).expect("upload should succeed");
        let captured = rx
            .recv_timeout(Duration::from_secs(3))
            .expect("captured request body");
        assert!(
            contains_subslice(&captured, &image_bytes),
            "multipart body should contain original image bytes unchanged"
        );

        server.join().expect("server thread");
        fs::remove_dir_all(root).ok();
    }
}
