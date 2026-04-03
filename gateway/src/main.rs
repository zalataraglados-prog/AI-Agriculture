use std::env;
use std::io::{BufRead, BufReader, ErrorKind};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use serialport::SerialPort;

const DEFAULT_TARGET: &str = "127.0.0.1:9000";
const DEFAULT_INTERVAL_MS: u64 = 5000;
const DEFAULT_ACK_TIMEOUT_MS: u64 = 3000;
const DEFAULT_EXPECTED_ACK: &str = "ack:success";
const DEFAULT_PAYLOAD_SUCCESS: &str = "success";
const DEFAULT_SERIAL_BAUD: u32 = 115200;
const DEFAULT_SERIAL_TIMEOUT_MS: u64 = 1200;

#[derive(Debug, Clone, Copy)]
enum PayloadMode {
    FixedSuccess,
    SerialMq7,
}

#[derive(Debug)]
struct Config {
    target: String,
    count: Option<u64>,
    interval: Duration,
    wait_ack: bool,
    ack_timeout: Duration,
    expected_ack: String,
    payload_mode: PayloadMode,
    serial_port: Option<String>,
    serial_baud: u32,
}

#[derive(Debug)]
struct Mq7Reading {
    raw: u16,
    voltage: f32,
}

struct SerialMq7Source {
    reader: BufReader<Box<dyn SerialPort>>,
}

impl SerialMq7Source {
    fn open(port: &str, baud: u32) -> Result<Self, String> {
        let serial = serialport::new(port, baud)
            .timeout(Duration::from_millis(DEFAULT_SERIAL_TIMEOUT_MS))
            .open()
            .map_err(|e| format!("Failed to open serial port {port} at {baud} baud: {e}"))?;

        Ok(Self {
            reader: BufReader::new(serial),
        })
    }

    fn next_payload(&mut self) -> Result<String, String> {
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

fn parse_mq7_line(line: &str) -> Option<Mq7Reading> {
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

fn print_usage(binary: &str) {
    eprintln!(
        "Usage:
  {binary} [--target <ip:port>] [--count <n>] [--interval-ms <ms>] [--no-wait-ack]
          [--ack-timeout-ms <ms>] [--expected-ack <payload>]
          [--serial-port </dev/ttyUSB0>] [--serial-baud <baud>]

Defaults:
  --target {DEFAULT_TARGET}
  --interval-ms {DEFAULT_INTERVAL_MS}
  --count not set (send forever)
  --ack-timeout-ms {DEFAULT_ACK_TIMEOUT_MS}
  --expected-ack {DEFAULT_EXPECTED_ACK}
  --serial-baud {DEFAULT_SERIAL_BAUD}

Payload mode:
  1) default (no --serial-port): fixed payload \"{DEFAULT_PAYLOAD_SUCCESS}\"
  2) with --serial-port: parse serial line \"MQ7 raw=<n> voltage=<v>V\"
     and send payload \"mq7:raw=<n>,voltage=<v>\""
    );
}

fn parse_args() -> Result<Config, String> {
    let mut target = DEFAULT_TARGET.to_string();
    let mut count: Option<u64> = None;
    let mut interval_ms = DEFAULT_INTERVAL_MS;
    let mut wait_ack = true;
    let mut ack_timeout_ms = DEFAULT_ACK_TIMEOUT_MS;
    let mut expected_ack = DEFAULT_EXPECTED_ACK.to_string();
    let mut serial_port: Option<String> = None;
    let mut serial_baud = DEFAULT_SERIAL_BAUD;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --target".to_string())?;
                target = value;
            }
            "--count" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --count".to_string())?;
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "Invalid --count, expected unsigned integer".to_string())?;
                count = Some(parsed);
            }
            "--interval-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --interval-ms".to_string())?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| "Invalid --interval-ms, expected unsigned integer".to_string())?;
            }
            "--no-wait-ack" => {
                wait_ack = false;
            }
            "--ack-timeout-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --ack-timeout-ms".to_string())?;
                ack_timeout_ms = value.parse::<u64>().map_err(|_| {
                    "Invalid --ack-timeout-ms, expected unsigned integer".to_string()
                })?;
            }
            "--expected-ack" => {
                expected_ack = args
                    .next()
                    .ok_or_else(|| "Missing value for --expected-ack".to_string())?;
            }
            "--serial-port" => {
                serial_port = Some(
                    args.next()
                        .ok_or_else(|| "Missing value for --serial-port".to_string())?,
                );
            }
            "--serial-baud" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --serial-baud".to_string())?;
                serial_baud = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid --serial-baud, expected unsigned integer".to_string())?;
                if serial_baud == 0 {
                    return Err("--serial-baud must be >= 1".to_string());
                }
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    if count == Some(0) {
        return Err("--count must be >= 1 when provided".to_string());
    }
    if interval_ms == 0 {
        return Err("--interval-ms must be >= 1".to_string());
    }
    if ack_timeout_ms == 0 {
        return Err("--ack-timeout-ms must be >= 1".to_string());
    }

    let payload_mode = if serial_port.is_some() {
        PayloadMode::SerialMq7
    } else {
        PayloadMode::FixedSuccess
    };

    Ok(Config {
        target,
        count,
        interval: Duration::from_millis(interval_ms),
        wait_ack,
        ack_timeout: Duration::from_millis(ack_timeout_ms),
        expected_ack,
        payload_mode,
        serial_port,
        serial_baud,
    })
}

fn run(config: &Config) -> Result<(), String> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind local UDP socket: {e}"))?;
    if config.wait_ack {
        socket
            .set_read_timeout(Some(config.ack_timeout))
            .map_err(|e| format!("Failed to set ACK timeout: {e}"))?;
    }

    let mut serial_source = match config.payload_mode {
        PayloadMode::FixedSuccess => None,
        PayloadMode::SerialMq7 => {
            let port = config
                .serial_port
                .as_deref()
                .ok_or_else(|| "Serial mode enabled but --serial-port is missing".to_string())?;
            Some(SerialMq7Source::open(port, config.serial_baud)?)
        }
    };

    println!(
        "[gateway-wsl] Start sending Orange Pi Zero3 simulated packets -> {}",
        config.target
    );
    match config.payload_mode {
        PayloadMode::FixedSuccess => {
            println!(
                "[gateway-wsl] Payload mode: fixed \"{}\"",
                DEFAULT_PAYLOAD_SUCCESS
            );
            println!("[gateway-wsl] Interval: {} ms", config.interval.as_millis());
        }
        PayloadMode::SerialMq7 => {
            let port = config
                .serial_port
                .as_deref()
                .ok_or_else(|| "Serial mode enabled but --serial-port is missing".to_string())?;
            println!(
                "[gateway-wsl] Payload mode: serial MQ-7 from {} @ {} baud",
                port, config.serial_baud
            );
            println!("[gateway-wsl] Interval: ignored in serial mode (send per serial line)");
        }
    }
    if let Some(total) = config.count {
        println!("[gateway-wsl] Mode: finite loop, count={total}");
    } else {
        println!("[gateway-wsl] Mode: infinite loop");
    }
    if config.wait_ack {
        println!(
            "[gateway-wsl] ACK mode: wait up to {} ms for \"{}\"",
            config.ack_timeout.as_millis(),
            config.expected_ack
        );
    } else {
        println!("[gateway-wsl] ACK mode: disabled");
    }

    let mut index: u64 = 1;
    loop {
        let payload = match serial_source.as_mut() {
            Some(source) => source.next_payload()?,
            None => DEFAULT_PAYLOAD_SUCCESS.to_string(),
        };

        socket
            .send_to(payload.as_bytes(), &config.target)
            .map_err(|e| format!("Send failed at packet #{index}: {e}"))?;
        if config.wait_ack {
            let mut ack_buf = [0_u8; 1024];
            let (ack_size, ack_peer) = match socket.recv_from(&mut ack_buf) {
                Ok(v) => v,
                Err(err)
                    if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
                {
                    return Err(format!(
                        "ACK timeout at packet #{index}, expected \"{}\"",
                        config.expected_ack
                    ));
                }
                Err(err) => return Err(format!("ACK receive failed at packet #{index}: {err}")),
            };
            let ack_payload = String::from_utf8_lossy(&ack_buf[..ack_size]).to_string();
            if ack_payload != config.expected_ack {
                return Err(format!(
                    "ACK mismatch at packet #{index}: got \"{}\" from {}, expected \"{}\"; sent payload=\"{}\"",
                    ack_payload, ack_peer, config.expected_ack, payload
                ));
            }
            println!("[gateway-wsl] ACK packet #{index} from {ack_peer}: \"{ack_payload}\"");
        }

        match config.count {
            Some(total) => {
                println!(
                    "[gateway-wsl] Sent packet #{index}/{total} to {}",
                    config.target
                );
                if index >= total {
                    println!("[gateway-wsl] Done.");
                    break;
                }
            }
            None => {
                println!(
                    "[gateway-wsl] Sent packet #{index}/inf to {} payload=\"{}\"",
                    config.target, payload
                );
            }
        }

        index += 1;
        if matches!(config.payload_mode, PayloadMode::FixedSuccess) {
            thread::sleep(config.interval);
        }
    }

    Ok(())
}

fn main() {
    let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
    let config = match parse_args() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Argument error: {err}\n");
            print_usage(&binary);
            std::process::exit(2);
        }
    };

    if let Err(err) = run(&config) {
        eprintln!("{err}");
        std::process::exit(1);
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
