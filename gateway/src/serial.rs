use std::collections::{BTreeMap, BTreeSet};
use std::env;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use dht_sensor::dht22::blocking as dht22_blocking;
#[cfg(target_os = "linux")]
use embedded_hal::digital::{ErrorKind as HalErrorKind, ErrorType, InputPin, OutputPin};
#[cfg(target_os = "linux")]
use linux_embedded_hal::Delay;
use serialport::SerialPort;
#[cfg(target_os = "linux")]
use sysfs_gpio::{Direction, Pin as SysfsPin};
use crate::protocol::{parse_known_event as parse_protocol_event, ParsedSensorEvent};

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

struct NativePin {
    gpio: u32,
    pin_label: String,
    sensor_id: String,
    feature: String,
    protocol: String,
    handle: NativeHandle,
}

enum NativeHandle {
    #[cfg(target_os = "linux")]
    Digital(SysfsPin),
    #[cfg(target_os = "linux")]
    Dht22(DhtSysfsPin),
    #[cfg(not(target_os = "linux"))]
    Unsupported,
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct DhtSysfsPin {
    pin: SysfsPin,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
struct DhtPinError(String);

#[cfg(target_os = "linux")]
impl fmt::Display for DhtPinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(target_os = "linux")]
impl embedded_hal::digital::Error for DhtPinError {
    fn kind(&self) -> HalErrorKind {
        HalErrorKind::Other
    }
}

#[cfg(target_os = "linux")]
impl ErrorType for DhtSysfsPin {
    type Error = DhtPinError;
}

#[cfg(target_os = "linux")]
impl OutputPin for DhtSysfsPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.pin
            .set_direction(Direction::Out)
            .map_err(|e| DhtPinError(format!("set_direction out failed: {e}")))?;
        self.pin
            .set_value(0)
            .map_err(|e| DhtPinError(format!("set_value low failed: {e}")))
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.pin
            .set_direction(Direction::Out)
            .map_err(|e| DhtPinError(format!("set_direction out failed: {e}")))?;
        self.pin
            .set_value(1)
            .map_err(|e| DhtPinError(format!("set_value high failed: {e}")))
    }
}

#[cfg(target_os = "linux")]
impl InputPin for DhtSysfsPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.pin
            .set_direction(Direction::In)
            .map_err(|e| DhtPinError(format!("set_direction in failed: {e}")))?;
        self.pin
            .get_value()
            .map(|v| v == 1)
            .map_err(|e| DhtPinError(format!("get_value failed: {e}")))
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|v| !v)
    }
}

pub struct NativeSensorSource {
    pins: Vec<NativePin>,
    next_index: usize,
    last_round_at: Option<Instant>,
    round_interval: Duration,
}
//Mark *1
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
            let ph7_sensor_id = parse_string_env("GATEWAY_NATIVE_GPIO_PH7_SENSOR_ID", "needle_ph7");
            let pc11_sensor_id = parse_string_env("GATEWAY_NATIVE_GPIO_PC11_SENSOR_ID", "needle_pc11");
            let ph7_feature = parse_string_env("GATEWAY_NATIVE_GPIO_PH7_FEATURE", &ph7_sensor_id);
            let pc11_feature = parse_string_env("GATEWAY_NATIVE_GPIO_PC11_FEATURE", &pc11_sensor_id);
            let default_protocol = parse_string_env("GATEWAY_NATIVE_GPIO_PROTOCOL", "gpio.digital.v1");
            let ph7_protocol = parse_string_env("GATEWAY_NATIVE_GPIO_PH7_PROTOCOL", &default_protocol);
            let pc11_protocol = parse_string_env("GATEWAY_NATIVE_GPIO_PC11_PROTOCOL", &default_protocol);
            if gpio_interval_ms == 0 {
                return Err("GATEWAY_NATIVE_GPIO_INTERVAL_MS must be >= 1".to_string());
            }

            let pins = vec![
                build_native_pin(ph7_gpio, "PH7", ph7_sensor_id, ph7_feature, ph7_protocol)?,
                build_native_pin(pc11_gpio, "PC11", pc11_sensor_id, pc11_feature, pc11_protocol)?,
            ];

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

        let idx = self.next_index;
        let total = self.pins.len();
        self.next_index = (self.next_index + 1) % total;
        let pin = &mut self.pins[idx];

        let raw_line = match read_native_sensor(pin) {
            Ok(line) => line,
            Err(err) => format!(
                "NATIVE_ERR sensor={} feature={} detail={}",
                pin.sensor_id,
                pin.feature,
                err.replace(' ', "_")
            ),
        };

        if let Some(parsed) = parse_protocol_event(&raw_line) {
            return Ok(from_parsed_event(parsed, raw_line));
        }

        let mut fields = parse_generic_fields(&raw_line);
        if fields.is_empty() {
            fields.insert("raw_text".to_string(), raw_line.replace(',', " "));
        }
        fields.insert("protocol".to_string(), pin.protocol.clone());
        fields.entry("pin".to_string()).or_insert_with(|| pin.pin_label.clone());
        fields.entry("gpio".to_string()).or_insert_with(|| pin.gpio.to_string());

        Ok(SensorEvent {
            sensor_id: pin.sensor_id.clone(),
            feature: pin.feature.clone(),
            fields,
            raw_line,
        })
    }
}

fn build_native_pin(
    gpio: u32,
    pin_label: &str,
    sensor_id: String,
    feature: String,
    protocol: String,
) -> Result<NativePin, String> {
    #[cfg(target_os = "linux")]
    {
    let pin = SysfsPin::new(gpio as u64);
    pin.export()
        .map_err(|e| format!("Failed to export gpio{gpio}: {e}"))?;
    pin.set_direction(Direction::In)
        .map_err(|e| format!("Failed to set gpio{gpio} direction to input: {e}"))?;

    let handle = if protocol.eq_ignore_ascii_case("dht22") {
        NativeHandle::Dht22(DhtSysfsPin { pin })
    } else {
        NativeHandle::Digital(pin)
    };

    Ok(NativePin {
        gpio,
        pin_label: pin_label.to_string(),
        sensor_id,
        feature,
        protocol,
        handle,
    })
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (gpio, pin_label);
        Ok(NativePin {
            gpio,
            pin_label: pin_label.to_string(),
            sensor_id,
            feature,
            protocol,
            handle: NativeHandle::Unsupported,
        })
    }
}

fn read_native_sensor(pin: &mut NativePin) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
    match (&pin.protocol.to_ascii_lowercase()[..], &mut pin.handle) {
        ("dht22", NativeHandle::Dht22(dht_pin)) => {
            let mut delay = Delay;
            let reading = dht22_blocking::read(&mut delay, dht_pin)
                .map_err(|e| format!("DHT22 read failed on gpio{}: {:?}", pin.gpio, e))?;
            Ok(format!(
                "DHT22 temp_c={:.1} hum={:.1}",
                reading.temperature,
                reading.relative_humidity
            ))
        }
        ("dht22", NativeHandle::Digital(_)) => Err(format!(
            "Protocol dht22 configured but pin handle is not dht-capable on gpio{}",
            pin.gpio
        )),
        (_, NativeHandle::Digital(gpio_pin)) => {
            let value = gpio_pin
                .get_value()
                .map_err(|e| format!("GPIO read failed on gpio{}: {e}", pin.gpio))?;
            if value != 0 && value != 1 {
                return Err(format!(
                    "Unexpected GPIO value on {} (gpio{}): {}",
                    pin.pin_label, pin.gpio, value
                ));
            }
            Ok(format!(
                "GPIO protocol=gpio.digital.v1 pin={} gpio={} value={} state={}",
                pin.pin_label,
                pin.gpio,
                value,
                if value == 1 { "active" } else { "inactive" }
            ))
        }
        (_, NativeHandle::Dht22(_)) => Err(format!(
            "Protocol mismatch on gpio{}: dht handle with protocol {}",
            pin.gpio, pin.protocol
        )),
    }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = pin;
        Err("Native direct sensor source is only supported on Linux".to_string())
    }
}

fn from_parsed_event(parsed: ParsedSensorEvent, raw_line: String) -> SensorEvent {
    SensorEvent {
        sensor_id: parsed.sensor_id,
        feature: parsed.feature,
        fields: parsed.fields,
        raw_line,
    }
}

#[cfg(target_os = "linux")]
fn parse_string_env(name: &str, default: &str) -> String {
    match env::var(name) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                default.to_string()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => default.to_string(),
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

                if let Some(event) = parse_protocol_event(&trimmed) {
                    found.managed_protocol_detected = true;
                    found.known_sensors.insert(event.sensor_id);
                    continue;
                }

                if let Some(feature) = extract_feature(&trimmed) {
                    if !parse_generic_fields(&trimmed).is_empty() {
                        found.managed_protocol_detected = true;
                    }
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

fn parse_known_event(line: &str) -> Option<SensorEvent> {
    parse_protocol_event(line).map(|parsed| from_parsed_event(parsed, line.to_string()))
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
    use super::{parse_adc_line, parse_dht22_line, parse_known_event, parse_mq7_line, parse_pcf8591_line};

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
    fn parse_known_event_supports_mq7() {
        let event = parse_known_event("MQ7 raw=206 voltage=0.166V").expect("should parse known mq7 event");
        assert_eq!(event.sensor_id, "mq7");
        assert_eq!(event.feature, "mq7");
        assert_eq!(event.fields.get("raw").map(|s| s.as_str()), Some("206"));
        assert_eq!(event.fields.get("voltage").map(|s| s.as_str()), Some("0.166"));
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

