use std::io::ErrorKind;
use std::net::UdpSocket;
use std::thread;

use crate::config::{Config, PayloadMode, SerialFormat};
use crate::constants::DEFAULT_PAYLOAD_SUCCESS;
use crate::serial::SerialSensorSource;

pub fn run(config: &Config) -> Result<(), String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind local UDP socket: {e}"))?;
    if config.wait_ack {
        socket
            .set_read_timeout(Some(config.ack_timeout))
            .map_err(|e| format!("Failed to set ACK timeout: {e}"))?;
    }

    let mut serial_source = match config.payload_mode {
        PayloadMode::FixedSuccess => None,
        PayloadMode::SerialSensor => {
            let port = config
                .serial_port
                .as_deref()
                .ok_or_else(|| "Serial mode enabled but --serial-port is missing".to_string())?;
            Some(SerialSensorSource::open(
                port,
                config.serial_baud,
                config.serial_format,
            )?)
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
        PayloadMode::SerialSensor => {
            let port = config
                .serial_port
                .as_deref()
                .ok_or_else(|| "Serial mode enabled but --serial-port is missing".to_string())?;
            let sensor_kind = match config.serial_format {
                SerialFormat::Mq7 => "MQ-7",
                SerialFormat::Dht22 => "DHT22",
                SerialFormat::Adc => "ADC",
                SerialFormat::Pcf8591 => "PCF8591",
            };
            println!(
                "[gateway-wsl] Payload mode: serial {} from {} @ {} baud",
                sensor_kind, port, config.serial_baud
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
                println!("[gateway-wsl] Sent packet #{index}/{total} to {}", config.target);
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
