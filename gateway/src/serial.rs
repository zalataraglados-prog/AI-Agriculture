use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, ErrorKind};
#[cfg(target_os = "linux")]
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use serialport::SerialPort;
use crate::constants::{RESERVED_IMAGE_FEATURE, RESERVED_IMAGE_SENSOR_ID};

#[derive(Debug)]
pub struct Mq7Reading {
    pub raw: u16,
    pub voltage: f32,
}

#[derive(Debug)]
pub struct Dht22Reading {
    pub temp_c: f32,
    pub hum: f32,
}

#[derive(Debug)]
pub struct AdcReading {
    pub pin: u8,
    pub raw: u16,
    pub voltage: f32,
}

#[derive(Debug)]
pub struct Pcf8591Reading {
    pub addr: String,
    pub ain0: u8,
    pub ain1: u8,
    pub ain2: u8,
    pub ain3: u8,
}

#[derive(Debug, Clone)]
pub struct SensorEvent {
    pub sensor_id: String,
    pub feature: String,
    pub fields: BTreeMap<String, String>,
    #[allow(dead_code)]
    pub raw_line: String,
}

#[derive(Debug, Default, Clone)]
pub struct DiscoveryResult {
    pub known_sensors: BTreeSet<String>,
    pub unknown_features: BTreeSet<String>,
    pub sample_lines: Vec<String>,
    pub managed_protocol_detected: bool,
}

pub struct SerialEsp32Source {
    reader: BufReader<Box<dyn SerialPort>>,
    port: String,
    baud: u32,
}

impl SerialEsp32Source {
    pub fn open(port: &str, baud: u32) -> Result<Self, String> {
        let serial = serialport::new(port, baud)
            .timeout(Duration::from_millis(1200))
            .open()
            .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

        Ok(Self {
            reader: BufReader::new(serial),
            port: port.to_string(),
            baud,
        })
    }

    pub fn describe(&self) -> String {
        format!("{}@{}", self.port, self.baud)
    }

    pub fn next_event(
        &mut self,
        feature_mapping: &BTreeMap<String, String>,
    ) -> Result<Option<SensorEvent>, String> {
        loop {
            let maybe_line = self.read_line()?;
            let line = match maybe_line {
                Some(v) => v,
                None => continue,
            };

            if let Some(event) = parse_known_event(&line) {
                return Ok(Some(event));
            }

            let Some(feature) = extract_feature(&line) else {
                continue;
            };

            let sensor_id = match feature_mapping.get(&feature) {
                Some(value) => value.clone(),
                None => return Ok(None),
            };

            let mut fields = parse_generic_fields(&line);
            if fields.is_empty() {
                fields.insert("raw_text".to_string(), line.replace(',', " "));
            }

            return Ok(Some(SensorEvent {
                sensor_id,
                feature,
                fields,
                raw_line: line,
            }));
        }
    }

    fn read_line(&mut self) -> Result<Option<String>, String> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed))
                }
            }
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                Ok(None)
            }
            Err(err) => Err(format!("Failed to read serial line: {err}")),
        }
    }
}

#[derive(Debug, Clone)]
struct NativePin {
    gpio: u32,
    pin_label: String,
    sensor_id: String,
    feature: String,
    value_path: String,
}

pub struct NativeSensorSource {
    pins: Vec<NativePin>,
    next_index: usize,
    last_round_at: Option<Instant>,
    round_interval: Duration,
}

impl NativeSensorSource {
    pub fn new() -> Result<Self, String> {
        #[cfg(not(target_os = "linux"))]
        {
            return Err("Native direct sensor source is only supported on Linux".to_string());
        }

        #[cfg(target_os = "linux")]
        {
            let ph7_gpio = parse_gpio_env("GATEWAY_NATIVE_GPIO_PH7", 231)?;
            let pc11_gpio = parse_gpio_env("GATEWAY_NATIVE_GPIO_PC11", 75)?;
            let gpio_interval_ms = parse_u64_env("GATEWAY_NATIVE_GPIO_INTERVAL_MS", 800)?;
            if gpio_interval_ms == 0 {
                return Err("GATEWAY_NATIVE_GPIO_INTERVAL_MS must be >= 1".to_string());
            }

            let pins = vec![
                NativePin {
                    gpio: ph7_gpio,
                    pin_label: "PH7".to_string(),
                    sensor_id: "needle_ph7".to_string(),
                    feature: "needle_ph7".to_string(),
                    value_path: format!("/sys/class/gpio/gpio{ph7_gpio}/value"),
                },
                NativePin {
                    gpio: pc11_gpio,
                    pin_label: "PC11".to_string(),
                    sensor_id: "needle_pc11".to_string(),
                    feature: "needle_pc11".to_string(),
                    value_path: format!("/sys/class/gpio/gpio{pc11_gpio}/value"),
                },
            ];

            for pin in &pins {
                ensure_gpio_input(pin.gpio)?;
            }

            Ok(Self {
                pins,
                next_index: 0,
                last_round_at: None,
                round_interval: Duration::from_millis(gpio_interval_ms),
            })
        }
    }

    pub fn sensor_ids(&self) -> Vec<String> {
        self.pins.iter().map(|pin| pin.sensor_id.clone()).collect()
    }

    pub fn next_event(&mut self) -> Result<SensorEvent, String> {
        if self.pins.is_empty() {
            return Err("No native GPIO pins configured".to_string());
        }

        if self.next_index == 0 {
            if let Some(last) = self.last_round_at {
                let elapsed = last.elapsed();
                if elapsed < self.round_interval {
                    thread::sleep(self.round_interval - elapsed);
                }
            }
            self.last_round_at = Some(Instant::now());
        }

        let pin = &self.pins[self.next_index];
        self.next_index = (self.next_index + 1) % self.pins.len();

        let value_raw = fs::read_to_string(&pin.value_path)
            .map_err(|e| format!("Failed to read {}: {e}", pin.value_path))?;
        let value = value_raw.trim();
        if value != "0" && value != "1" {
            return Err(format!(
                "Unexpected GPIO value on {} (gpio{}): {}",
                pin.pin_label, pin.gpio, value
            ));
        }

        let mut fields = BTreeMap::new();
        fields.insert("pin".to_string(), pin.pin_label.clone());
        fields.insert("gpio".to_string(), pin.gpio.to_string());
        fields.insert("value".to_string(), value.to_string());
        fields.insert(
            "state".to_string(),
            if value == "1" {
                "active".to_string()
            } else {
                "inactive".to_string()
            },
        );

        Ok(SensorEvent {
            sensor_id: pin.sensor_id.clone(),
            feature: pin.feature.clone(),
            fields,
            raw_line: format!("{} gpio{} value={}", pin.pin_label, pin.gpio, value),
        })
    }
}

#[cfg(target_os = "linux")]
fn parse_gpio_env(name: &str, default: u32) -> Result<u32, String> {
    match env::var(name) {
        Ok(raw) => raw
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("Invalid {}='{}'", name, raw)),
        Err(_) => Ok(default),
    }
}

#[cfg(target_os = "linux")]
fn parse_u64_env(name: &str, default: u64) -> Result<u64, String> {
    match env::var(name) {
        Ok(raw) => raw
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("Invalid {}='{}'", name, raw)),
        Err(_) => Ok(default),
    }
}

#[cfg(target_os = "linux")]
fn ensure_gpio_input(gpio: u32) -> Result<(), String> {
    let gpio_dir = format!("/sys/class/gpio/gpio{gpio}");
    if fs::metadata(&gpio_dir).is_err() {
        let mut export = OpenOptions::new()
            .write(true)
            .open("/sys/class/gpio/export")
            .map_err(|e| format!("Failed to open /sys/class/gpio/export: {e}"))?;
        export
            .write_all(gpio.to_string().as_bytes())
            .map_err(|e| format!("Failed to export gpio{gpio}: {e}"))?;
    }

    let mut direction = OpenOptions::new()
        .write(true)
        .open(format!("{gpio_dir}/direction"))
        .map_err(|e| format!("Failed to open gpio{gpio} direction: {e}"))?;
    direction
        .write_all(b"in")
        .map_err(|e| format!("Failed to set gpio{gpio} direction to input: {e}"))?;
    Ok(())
}

pub fn list_serial_ports() -> Result<Vec<String>, String> {
    let mut ports = serialport::available_ports()
        .map_err(|e| format!("Failed to enumerate serial ports: {e}"))?;
    let mut names: Vec<String> = ports.drain(..).map(|p| p.port_name).collect();

    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let Some(name) = entry.file_name().to_str().map(|v| v.to_string()) else {
                    continue;
                };
                if name.starts_with("ttyS")
                    || name.starts_with("ttyUSB")
                    || name.starts_with("ttyACM")
                {
                    names.push(format!("/dev/{name}"));
                }
            }
        }
    }

    names.sort();
    names.dedup();

    if let Ok(extra) = env::var("GATEWAY_EXTRA_PORTS") {
        for part in extra.split(',') {
            let port = part.trim();
            if port.is_empty() {
                continue;
            }
            names.push(port.to_string());
        }
        names.sort();
        names.dedup();
    }

    Ok(names)
}

pub fn discover_on_port(port: &str, baud: u32, window: Duration) -> Result<DiscoveryResult, String> {
    let serial = serialport::new(port, baud)
        .timeout(Duration::from_millis(600))
        .open()
        .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

    let mut reader = BufReader::new(serial);
    let deadline = Instant::now() + window;
    let mut found = DiscoveryResult::default();

    while Instant::now() < deadline {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => continue,
            Ok(_) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                if found.sample_lines.len() < 5 {
                    found.sample_lines.push(trimmed.clone());
                }
                if is_managed_protocol_line(&trimmed) {
                    found.managed_protocol_detected = true;
                    continue;
                }

                if let Some(event) = parse_known_event(&trimmed) {
                    found.known_sensors.insert(event.sensor_id);
                    continue;
                }

                if let Some(feature) = extract_feature(&trimmed) {
                    found.unknown_features.insert(feature);
                }
            }
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                continue;
            }
            Err(err) => return Err(format!("Failed to read from {port}@{baud}: {err}")),
        }
    }

    Ok(found)
}

fn parse_image_channel_line(line: &str) -> Option<SensorEvent> {
    let feature = extract_feature(line)?;
    if feature != "image" && feature != "img" && feature != "frame" {
        return None;
    }

    let mut fields = parse_generic_fields(line);
    if fields.is_empty() {
        fields.insert("raw_text".to_string(), line.replace(',', " "));
    }

    Some(SensorEvent {
        sensor_id: RESERVED_IMAGE_SENSOR_ID.to_string(),
        feature: RESERVED_IMAGE_FEATURE.to_string(),
        fields,
        raw_line: line.to_string(),
    })
}

fn parse_known_event(line: &str) -> Option<SensorEvent> {
    if let Some(event) = parse_image_channel_line(line) {
        return Some(event);
    }

    if let Some(reading) = parse_dht22_line(line) {
        let mut fields = BTreeMap::new();
        fields.insert("temp_c".to_string(), format!("{:.1}", reading.temp_c));
        fields.insert("hum".to_string(), format!("{:.1}", reading.hum));
        return Some(SensorEvent {
            sensor_id: "dht22".to_string(),
            feature: "dht22".to_string(),
            fields,
            raw_line: line.to_string(),
        });
    }

    if let Some(reading) = parse_adc_line(line) {
        let mut fields = BTreeMap::new();
        fields.insert("pin".to_string(), reading.pin.to_string());
        fields.insert("raw".to_string(), reading.raw.to_string());
        fields.insert("voltage".to_string(), format!("{:.3}", reading.voltage));
        return Some(SensorEvent {
            sensor_id: "adc".to_string(),
            feature: "adc".to_string(),
            fields,
            raw_line: line.to_string(),
        });
    }

    if let Some(reading) = parse_pcf8591_line(line) {
        let mut fields = BTreeMap::new();
        fields.insert("addr".to_string(), reading.addr);
        fields.insert("ain0".to_string(), reading.ain0.to_string());
        fields.insert("ain1".to_string(), reading.ain1.to_string());
        fields.insert("ain2".to_string(), reading.ain2.to_string());
        fields.insert("ain3".to_string(), reading.ain3.to_string());
        return Some(SensorEvent {
            sensor_id: "pcf8591".to_string(),
            feature: "pcf8591".to_string(),
            fields,
            raw_line: line.to_string(),
        });
    }

    None
}

fn parse_generic_fields(line: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();

    for token in line.replace(',', " ").split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };

        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }

        let normalized = normalize_value(value);
        if normalized.is_empty() {
            continue;
        }
        fields.insert(key, normalized);
    }

    fields
}

fn normalize_value(value: &str) -> String {
    let trimmed = value
        .trim()
        .trim_matches(|c: char| c == ',' || c == ';' || c == '"');
    let trimmed = trimmed.strip_suffix('V').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('%').unwrap_or(trimmed);
    trimmed.to_string()
}

fn extract_feature(line: &str) -> Option<String> {
    let first = line.split_whitespace().next()?;
    let mut buf = String::new();
    for ch in first.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            buf.push(ch.to_ascii_lowercase());
        } else if !buf.is_empty() {
            break;
        }
    }

    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

fn is_managed_protocol_line(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    upper.starts_with("AIAG ")
        || upper.starts_with("AIAG:")
        || upper.contains("AIAG HELLO")
        || upper.contains("AIAG CAPS")
        || upper.contains("AIAG RUN")
}

pub fn parse_mq7_line(line: &str) -> Option<Mq7Reading> {
    let mut raw: Option<u16> = None;
    let mut voltage: Option<f32> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("raw=") {
            raw = value.parse::<u16>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("voltage=") {
            let stripped = value.strip_suffix('V').unwrap_or(value);
            voltage = stripped.parse::<f32>().ok();
        }
    }

    match (raw, voltage) {
        (Some(raw), Some(voltage)) => Some(Mq7Reading { raw, voltage }),
        _ => None,
    }
}

pub fn parse_dht22_line(line: &str) -> Option<Dht22Reading> {
    let mut temp_c: Option<f32> = None;
    let mut hum: Option<f32> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("temp_c=") {
            temp_c = value.parse::<f32>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("hum=") {
            let stripped = value.strip_suffix('%').unwrap_or(value);
            hum = stripped.parse::<f32>().ok();
        }
    }

    match (temp_c, hum) {
        (Some(temp_c), Some(hum)) => Some(Dht22Reading { temp_c, hum }),
        _ => None,
    }
}

pub fn parse_adc_line(line: &str) -> Option<AdcReading> {
    let mut pin: Option<u8> = None;
    let mut raw: Option<u16> = None;
    let mut voltage: Option<f32> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("pin=") {
            pin = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("raw=") {
            raw = value.parse::<u16>().ok();
            continue;
        }
        if let Some(value) = token.strip_prefix("voltage=") {
            let stripped = value.strip_suffix('V').unwrap_or(value);
            voltage = stripped.parse::<f32>().ok();
        }
    }

    match (pin, raw, voltage) {
        (Some(pin), Some(raw), Some(voltage)) => Some(AdcReading { pin, raw, voltage }),
        _ => None,
    }
}

pub fn parse_pcf8591_line(line: &str) -> Option<Pcf8591Reading> {
    let mut addr = "0x48".to_string();
    let mut ain0: Option<u8> = None;
    let mut ain1: Option<u8> = None;
    let mut ain2: Option<u8> = None;
    let mut ain3: Option<u8> = None;

    for token in line.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("addr=") {
            addr = value.to_string();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain0=") {
            ain0 = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain1=") {
            ain1 = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain2=") {
            ain2 = value.parse::<u8>().ok();
            continue;
        }
        if let Some(value) = lower.strip_prefix("ain3=") {
            ain3 = value.parse::<u8>().ok();
        }
    }

    match (ain0, ain1, ain2, ain3) {
        (Some(ain0), Some(ain1), Some(ain2), Some(ain3)) => Some(Pcf8591Reading {
            addr,
            ain0,
            ain1,
            ain2,
            ain3,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_adc_line, parse_dht22_line, parse_mq7_line, parse_pcf8591_line};

    #[test]
    fn parse_valid_line() {
        let reading = parse_mq7_line("MQ7 raw=206 voltage=0.166V").expect("should parse");
        assert_eq!(reading.raw, 206);
        assert!((reading.voltage - 0.166_f32).abs() < 1e-6);
    }

    #[test]
    fn parse_invalid_line_returns_none() {
        assert!(parse_mq7_line("random noise").is_none());
        assert!(parse_mq7_line("MQ7 raw=abc voltage=0.166V").is_none());
        assert!(parse_mq7_line("MQ7 raw=200").is_none());
    }

    #[test]
    fn parse_valid_dht22_line() {
        let reading = parse_dht22_line("DHT22 temp_c=31.5 hum=67.0").expect("should parse");
        assert!((reading.temp_c - 31.5_f32).abs() < 1e-6);
        assert!((reading.hum - 67.0_f32).abs() < 1e-6);
    }

    #[test]
    fn parse_invalid_dht22_line_returns_none() {
        assert!(parse_dht22_line("hello world").is_none());
        assert!(parse_dht22_line("DHT22 temp_c=abc hum=55.1").is_none());
        assert!(parse_dht22_line("DHT22 temp_c=25.2").is_none());
    }

    #[test]
    fn parse_valid_adc_line() {
        let reading = parse_adc_line("ADC pin=34 raw=523 voltage=0.421V").expect("should parse");
        assert_eq!(reading.pin, 34);
        assert_eq!(reading.raw, 523);
        assert!((reading.voltage - 0.421_f32).abs() < 1e-6);
    }

    #[test]
    fn parse_invalid_adc_line_returns_none() {
        assert!(parse_adc_line("ADC raw=500 voltage=0.400V").is_none());
        assert!(parse_adc_line("ADC pin=34 raw=abc voltage=0.400V").is_none());
        assert!(parse_adc_line("ADC pin=34 raw=500").is_none());
    }

    #[test]
    fn parse_valid_pcf8591_line() {
        let reading = parse_pcf8591_line("PCF8591 addr=0x48 AIN0=172 AIN1=255 AIN2=90 AIN3=129")
            .expect("should parse");
        assert_eq!(reading.addr, "0x48");
        assert_eq!(reading.ain0, 172);
        assert_eq!(reading.ain1, 255);
        assert_eq!(reading.ain2, 90);
        assert_eq!(reading.ain3, 129);
    }

    #[test]
    fn parse_pcf8591_line_without_addr_uses_default() {
        let reading = parse_pcf8591_line("AIN0=172 AIN1=255 AIN2=90 AIN3=129").expect("should parse");
        assert_eq!(reading.addr, "0x48");
    }

    #[test]
    fn parse_invalid_pcf8591_line_returns_none() {
        assert!(parse_pcf8591_line("PCF8591 addr=0x48 AIN0=172 AIN1=255").is_none());
        assert!(
            parse_pcf8591_line("PCF8591 addr=0x48 AIN0=abc AIN1=255 AIN2=90 AIN3=129").is_none()
        );
    }
}

