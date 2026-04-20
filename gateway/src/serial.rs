use std::collections::{BTreeMap, BTreeSet};
use std::env;
#[cfg(target_os = "linux")]
use std::fs;
use std::io::{ErrorKind, Write};
use std::thread;
use std::time::{Duration, Instant};

use serialport::{DataBits, Parity, SerialPort, StopBits};

const MODBUS_SLAVE_ID: u8 = 0x02;
const MODBUS_FUNC_READ_HOLDING: u8 = 0x03;
const MODBUS_START_ADDR: u16 = 0x0000;
const MODBUS_REG_COUNT: u16 = 0x0003;
const MODBUS_RESPONSE_LEN: usize = 11;
const MODBUS_SENSOR_ID: &str = "soil_modbus_02";
const MODBUS_FEATURE: &str = "soil_modbus";
const DEFAULT_MODBUS_PORT: &str = "/dev/ttyUSB0";

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
    serial: Box<dyn SerialPort>,
    port: String,
    baud: u32,
    last_poll_at: Option<Instant>,
    poll_interval: Duration,
}

impl SerialEsp32Source {
    pub fn open(port: &str, baud: u32) -> Result<Self, String> {
        let serial = serialport::new(port, baud)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .timeout(Duration::from_millis(200))
            .open()
            .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

        Ok(Self {
            serial,
            port: port.to_string(),
            baud,
            last_poll_at: None,
            poll_interval: Duration::from_secs(1),
        })
    }

    pub fn describe(&self) -> String {
        format!("{}@{}", self.port, self.baud)
    }

    pub fn next_event(&mut self) -> Result<SensorEvent, String> {
        if let Some(last) = self.last_poll_at {
            let elapsed = last.elapsed();
            if elapsed < self.poll_interval {
                thread::sleep(self.poll_interval - elapsed);
            }
        }
        self.last_poll_at = Some(Instant::now());

        let request =
            build_modbus_read_holding_request(MODBUS_SLAVE_ID, MODBUS_START_ADDR, MODBUS_REG_COUNT);
        self.serial
            .write_all(&request)
            .map_err(|e| format!("Failed to write Modbus request on {}: {e}", self.port))?;
        self.serial
            .flush()
            .map_err(|e| format!("Failed to flush Modbus request on {}: {e}", self.port))?;

        // Industrial RS485 sensor typically needs a short processing delay before replying.
        thread::sleep(Duration::from_millis(80));

        let mut frame = [0_u8; MODBUS_RESPONSE_LEN];
        read_exact_with_deadline(&mut *self.serial, &mut frame, Duration::from_millis(900))?;
        let (temp_raw, vwc_raw, ec_raw) = parse_modbus_response_frame(&frame)?;

        let mut fields = BTreeMap::new();
        fields.insert("vwc".to_string(), format!("{:.1}", (vwc_raw as f32) / 10.0));
        fields.insert(
            "temp_c".to_string(),
            format!("{:.1}", (temp_raw as f32) / 10.0),
        );
        fields.insert("ec".to_string(), ec_raw.to_string());
        fields.insert("protocol".to_string(), "modbus.rtu.v1".to_string());
        fields.insert("slave_id".to_string(), MODBUS_SLAVE_ID.to_string());

        let raw_line = frame
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        Ok(SensorEvent {
            sensor_id: MODBUS_SENSOR_ID.to_string(),
            feature: MODBUS_FEATURE.to_string(),
            fields,
            raw_line,
        })
    }
}

fn read_exact_with_deadline(
    serial: &mut dyn SerialPort,
    buf: &mut [u8],
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let mut offset = 0_usize;

    while offset < buf.len() {
        if Instant::now() >= deadline {
            return Err(format!(
                "Modbus response timeout: expected {} bytes, got {} bytes",
                buf.len(),
                offset
            ));
        }

        match serial.read(&mut buf[offset..]) {
            Ok(0) => continue,
            Ok(size) => {
                offset += size;
            }
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                continue;
            }
            Err(err) => return Err(format!("Failed to read Modbus response: {err}")),
        }
    }

    Ok(())
}

pub fn list_serial_ports() -> Result<Vec<String>, String> {
    if let Ok(port) = env::var("GATEWAY_MODBUS_PORT") {
        let trimmed = port.trim();
        if !trimmed.is_empty() {
            return Ok(vec![trimmed.to_string()]);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let default_port = DEFAULT_MODBUS_PORT.to_string();
        if let Ok(entries) = fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let Some(name) = entry.file_name().to_str().map(|v| v.to_string()) else {
                    continue;
                };
                if format!("/dev/{name}") == default_port {
                    return Ok(vec![default_port]);
                }
            }
        }
        return Ok(vec![default_port]);
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(vec![DEFAULT_MODBUS_PORT.to_string()])
    }
}

pub fn discover_on_port(
    port: &str,
    baud: u32,
    window: Duration,
) -> Result<DiscoveryResult, String> {
    let mut serial = serialport::new(port, baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(200))
        .open()
        .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

    let deadline = Instant::now() + window;
    let mut found = DiscoveryResult::default();

    while Instant::now() < deadline {
        let request =
            build_modbus_read_holding_request(MODBUS_SLAVE_ID, MODBUS_START_ADDR, MODBUS_REG_COUNT);
        if let Err(err) = serial.write_all(&request) {
            return Err(format!(
                "Failed to write Modbus probe on {port}@{baud}: {err}"
            ));
        }
        if let Err(err) = serial.flush() {
            return Err(format!(
                "Failed to flush Modbus probe on {port}@{baud}: {err}"
            ));
        }

        thread::sleep(Duration::from_millis(80));

        let mut frame = [0_u8; MODBUS_RESPONSE_LEN];
        match read_exact_with_deadline(&mut *serial, &mut frame, Duration::from_millis(650)) {
            Ok(()) => {
                if let Ok((temp_raw, vwc_raw, ec_raw)) = parse_modbus_response_frame(&frame) {
                    found.managed_protocol_detected = true;
                    found.known_sensors.insert(MODBUS_SENSOR_ID.to_string());
                    if found.sample_lines.is_empty() {
                        found.sample_lines.push(format!(
                            "MODBUS slave=2 vwc={:.1} temp_c={:.1} ec={}",
                            (vwc_raw as f32) / 10.0,
                            (temp_raw as f32) / 10.0,
                            ec_raw
                        ));
                    }
                    break;
                }
            }
            Err(_) => {
                continue;
            }
        }
    }

    Ok(found)
}

fn build_modbus_read_holding_request(slave_id: u8, start_addr: u16, count: u16) -> [u8; 8] {
    let mut frame = [0_u8; 8];
    frame[0] = slave_id;
    frame[1] = MODBUS_FUNC_READ_HOLDING;
    frame[2] = (start_addr >> 8) as u8;
    frame[3] = (start_addr & 0xFF) as u8;
    frame[4] = (count >> 8) as u8;
    frame[5] = (count & 0xFF) as u8;

    let crc = modbus_crc16(&frame[..6]);
    frame[6] = (crc & 0xFF) as u8;
    frame[7] = (crc >> 8) as u8;
    frame
}

fn parse_modbus_response_frame(frame: &[u8]) -> Result<(u16, u16, u16), String> {
    if frame.len() != MODBUS_RESPONSE_LEN {
        return Err(format!(
            "Invalid Modbus response length: expected {}, got {}",
            MODBUS_RESPONSE_LEN,
            frame.len()
        ));
    }

    if frame[0] != MODBUS_SLAVE_ID {
        return Err(format!(
            "Unexpected slave id in response: expected {}, got {}",
            MODBUS_SLAVE_ID, frame[0]
        ));
    }

    if frame[1] != MODBUS_FUNC_READ_HOLDING {
        return Err(format!(
            "Unexpected function code in response: expected {}, got {}",
            MODBUS_FUNC_READ_HOLDING, frame[1]
        ));
    }

    if frame[2] != 0x06 {
        return Err(format!(
            "Unexpected byte count in response: expected 6, got {}",
            frame[2]
        ));
    }

    let crc_expected = modbus_crc16(&frame[..9]);
    let crc_actual = u16::from_le_bytes([frame[9], frame[10]]);
    if crc_expected != crc_actual {
        return Err(format!(
            "CRC mismatch in Modbus response: expected {:04X}, got {:04X}",
            crc_expected, crc_actual
        ));
    }

    // Soil sensor register order is: temperature, VWC, EC.
    let temp_raw = u16::from_be_bytes([frame[3], frame[4]]);
    let vwc_raw = u16::from_be_bytes([frame[5], frame[6]]);
    let ec_raw = u16::from_be_bytes([frame[7], frame[8]]);
    Ok((temp_raw, vwc_raw, ec_raw))
}

fn modbus_crc16(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for byte in data {
        crc ^= *byte as u16;
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::{
        build_modbus_read_holding_request, modbus_crc16, parse_modbus_response_frame,
        MODBUS_RESPONSE_LEN,
    };

    #[test]
    fn build_request_matches_known_bytes() {
        let frame = build_modbus_read_holding_request(0x02, 0x0000, 0x0003);
        assert_eq!(frame, [0x02, 0x03, 0x00, 0x00, 0x00, 0x03, 0x05, 0xF8]);
    }

    #[test]
    fn parse_response_maps_registers() {
        let mut frame = [0_u8; MODBUS_RESPONSE_LEN];
        frame[..9].copy_from_slice(&[0x02, 0x03, 0x06, 0x01, 0x0D, 0x00, 0xF8, 0x01, 0xB0]);
        let crc = modbus_crc16(&frame[..9]);
        frame[9] = (crc & 0xFF) as u8;
        frame[10] = (crc >> 8) as u8;

        let parsed = parse_modbus_response_frame(&frame).expect("should parse");
        assert_eq!(parsed, (269, 248, 432));
    }

    #[test]
    fn parse_response_rejects_bad_crc() {
        let frame = [
            0x02, 0x03, 0x06, 0x01, 0x0D, 0x00, 0xF8, 0x01, 0xB0, 0x00, 0x00,
        ];
        assert!(parse_modbus_response_frame(&frame).is_err());
    }
}
