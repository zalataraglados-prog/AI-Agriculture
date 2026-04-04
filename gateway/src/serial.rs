use std::io::{BufRead, BufReader, ErrorKind};
use std::time::Duration;

use serialport::SerialPort;

use crate::config::SerialFormat;
use crate::constants::DEFAULT_SERIAL_TIMEOUT_MS;

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

pub struct SerialSensorSource {
    reader: BufReader<Box<dyn SerialPort>>,
    format: SerialFormat,
}

impl SerialSensorSource {
    pub fn open(port: &str, baud: u32, format: SerialFormat) -> Result<Self, String> {
        let serial = serialport::new(port, baud)
            .timeout(Duration::from_millis(DEFAULT_SERIAL_TIMEOUT_MS))
            .open()
            .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

        Ok(Self {
            reader: BufReader::new(serial),
            format,
        })
    }

    pub fn next_payload(&mut self) -> Result<String, String> {
        let mut line = String::new();

        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => continue,
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match parse_line(trimmed, self.format) {
                        Some(parsed) => {
                            println!(
                                "[gateway-wsl] SERIAL <- {} | parsed {}",
                                trimmed, parsed.summary
                            );
                            return Ok(parsed.payload);
                        }
                        None => {
                            println!("[gateway-wsl] SERIAL skip: {}", trimmed);
                        }
                    }
                }
                Err(err)
                    if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
                {
                    continue;
                }
                Err(err) => return Err(format!("Failed to read from serial source: {err}")),
            }
        }
    }
}

#[derive(Debug)]
struct ParsedLine {
    payload: String,
    summary: String,
}

fn parse_line(line: &str, format: SerialFormat) -> Option<ParsedLine> {
    match format {
        SerialFormat::Mq7 => parse_mq7_line(line).map(|reading| ParsedLine {
            payload: format!("mq7:raw={},voltage={:.3}", reading.raw, reading.voltage),
            summary: format!("raw={} voltage={:.3}V", reading.raw, reading.voltage),
        }),
        SerialFormat::Dht22 => parse_dht22_line(line).map(|reading| ParsedLine {
            payload: format!("dht22:temp_c={:.1},hum={:.1}", reading.temp_c, reading.hum),
            summary: format!("temp_c={:.1} hum={:.1}", reading.temp_c, reading.hum),
        }),
        SerialFormat::Adc => parse_adc_line(line).map(|reading| ParsedLine {
            payload: format!(
                "adc:pin={},raw={},voltage={:.3}",
                reading.pin, reading.raw, reading.voltage
            ),
            summary: format!(
                "pin={} raw={} voltage={:.3}V",
                reading.pin, reading.raw, reading.voltage
            ),
        }),
        SerialFormat::Pcf8591 => parse_pcf8591_line(line).map(|reading| ParsedLine {
            payload: format!(
                "pcf8591:addr={},ain0={},ain1={},ain2={},ain3={}",
                reading.addr, reading.ain0, reading.ain1, reading.ain2, reading.ain3
            ),
            summary: format!(
                "addr={} ain0={} ain1={} ain2={} ain3={}",
                reading.addr, reading.ain0, reading.ain1, reading.ain2, reading.ain3
            ),
        }),
    }
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
        let reading = parse_pcf8591_line("AIN0=172 AIN1=255 AIN2=90 AIN3=129")
            .expect("should parse");
        assert_eq!(reading.addr, "0x48");
    }

    #[test]
    fn parse_invalid_pcf8591_line_returns_none() {
        assert!(parse_pcf8591_line("PCF8591 addr=0x48 AIN0=172 AIN1=255").is_none());
        assert!(parse_pcf8591_line("PCF8591 addr=0x48 AIN0=abc AIN1=255 AIN2=90 AIN3=129").is_none());
    }
}
