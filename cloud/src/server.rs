use std::io::ErrorKind;
use std::net::UdpSocket;

use crate::constants::UDP_BUFFER_SIZE;
use crate::model::{EvalResult, RuntimeConfig};
use crate::payload::evaluate_payload;

pub(crate) fn run(cfg: &RuntimeConfig) -> Result<(), String> {
    let socket =
        UdpSocket::bind(&cfg.bind).map_err(|e| format!("Bind failed on {}: {e}", cfg.bind))?;
    socket
        .set_read_timeout(cfg.timeout)
        .map_err(|e| format!("Failed to set read timeout: {e}"))?;

    println!("[cloud] Listening on {}", cfg.bind);
    println!(
        "[cloud] Loaded rules: exact={}, sensors={}",
        cfg.exact_rules.len(),
        cfg.sensor_rules.len()
    );
    println!(
        "[cloud] ACK defaults: mismatch=\"{}\", unknown_sensor=\"{}\"",
        cfg.ack_mismatch, cfg.ack_unknown_sensor
    );
    println!(
        "[cloud] Mode: {}",
        if cfg.once {
            "exit after first successful match"
        } else {
            "continuous/limited receive"
        }
    );

    let mut buf = [0_u8; UDP_BUFFER_SIZE];
    let mut received_count: u64 = 0;
    let mut success_count: u64 = 0;

    loop {
        let (size, peer) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                return Err("Receive timeout reached without enough packets".to_string());
            }
            Err(err) => return Err(format!("Receive failed: {err}")),
        };

        received_count += 1;
        let result = if size == buf.len() {
            EvalResult {
                matched: false,
                ack: cfg.ack_mismatch.clone(),
                detail: "packet size reached receive buffer limit; possible truncation".to_string(),
            }
        } else {
            let payload = String::from_utf8_lossy(&buf[..size]).trim().to_string();
            evaluate_payload(&payload, cfg)
        };
        let payload_display = String::from_utf8_lossy(&buf[..size]).trim().to_string();

        if result.matched {
            success_count += 1;
        }

        socket
            .send_to(result.ack.as_bytes(), peer)
            .map_err(|e| format!("ACK send failed to {peer}: {e}"))?;

        println!(
            "[cloud] Packet #{received_count} from {peer}: \"{payload_display}\" => {} ; ACK=\"{}\" ; {}",
            if result.matched { "MATCH" } else { "MISMATCH" },
            result.ack,
            result.detail
        );

        if cfg.once && result.matched {
            break;
        }

        if let Some(max) = cfg.max_packets {
            if received_count >= max {
                break;
            }
        }
    }

    println!("[cloud] Summary: received={received_count}, matched={success_count}");

    if success_count == 0 {
        return Err("No matching packet received".to_string());
    }

    Ok(())
}
