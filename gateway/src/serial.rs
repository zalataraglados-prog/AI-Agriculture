use std::io::{BufRead, BufReader, ErrorKind};
use std::time::Duration;

use serialport::SerialPort;

use crate::constants::DEFAULT_SERIAL_TIMEOUT_MS;

#[derive(Debug)]
pub struct Mq7Reading {
    pub raw: u16,
    pub voltage: f32,
}

pub struct SerialMq7Source {
    reader: BufReader<Box<dyn SerialPort>>,
}

impl SerialMq7Source {
    pub fn open(port: &str, baud: u32) -> Result<Self, String> {
        let serial = serialport::new(port, baud)
            .timeout(Duration::from_millis(DEFAULT_SERIAL_TIMEOUT_MS))
            .open()
            .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

        Ok(Self {
            reader: BufReader::new(serial),
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
                    match parse_mq7_line(trimmed) {
                        Some(reading) => {
                            println!(
                                "[gateway-wsl] SERIAL <- {} | parsed raw={} voltage={:.3}V",
                                trimmed, reading.raw, reading.voltage
                            );
                            return Ok(format!(
                                "mq7:raw={},voltage={:.3}",
                                reading.raw, reading.voltage
                            ));
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

#[cfg(test)]
mod tests {
    use super::parse_mq7_line;

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
}