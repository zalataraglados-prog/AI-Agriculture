use std::env;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

const DEFAULT_TARGET: &str = "127.0.0.1:9000";
const DEFAULT_INTERVAL_MS: u64 = 5000;
const DEFAULT_ACK_TIMEOUT_MS: u64 = 3000;
const DEFAULT_EXPECTED_ACK: &str = "ack:success";
const PAYLOAD: &[u8] = b"success";

#[derive(Debug)]
struct Config {
    target: String,
    count: Option<u64>,
    interval: Duration,
    wait_ack: bool,
    ack_timeout: Duration,
    expected_ack: String,
}

fn print_usage(binary: &str) {
    eprintln!(
        "Usage:\n  {binary} [--target <ip:port>] [--count <n>] [--interval-ms <ms>] [--no-wait-ack] [--ack-timeout-ms <ms>] [--expected-ack <payload>]\n\nDefaults:\n  --target {DEFAULT_TARGET}\n  --interval-ms {DEFAULT_INTERVAL_MS}\n  --count not set (send forever)\n  --ack-timeout-ms {DEFAULT_ACK_TIMEOUT_MS}\n  --expected-ack {DEFAULT_EXPECTED_ACK}\n\nPacket payload is fixed as: \"success\""
    );
}

fn parse_args() -> Result<Config, String> {
    let mut target = DEFAULT_TARGET.to_string();
    let mut count: Option<u64> = None;
    let mut interval_ms = DEFAULT_INTERVAL_MS;
    let mut wait_ack = true;
    let mut ack_timeout_ms = DEFAULT_ACK_TIMEOUT_MS;
    let mut expected_ack = DEFAULT_EXPECTED_ACK.to_string();

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
                ack_timeout_ms = value
                    .parse::<u64>()
                    .map_err(|_| "Invalid --ack-timeout-ms, expected unsigned integer".to_string())?;
            }
            "--expected-ack" => {
                expected_ack = args
                    .next()
                    .ok_or_else(|| "Missing value for --expected-ack".to_string())?;
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

    Ok(Config {
        target,
        count,
        interval: Duration::from_millis(interval_ms),
        wait_ack,
        ack_timeout: Duration::from_millis(ack_timeout_ms),
        expected_ack,
    })
}

fn run(config: &Config) -> Result<(), String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind local UDP socket: {e}"))?;
    if config.wait_ack {
        socket
            .set_read_timeout(Some(config.ack_timeout))
            .map_err(|e| format!("Failed to set ACK timeout: {e}"))?;
    }

    println!(
        "[gateway-wsl] Start sending Orange Pi Zero3 simulated packets -> {}",
        config.target
    );
    println!(
        "[gateway-wsl] Payload fixed to \"{}\"",
        String::from_utf8_lossy(PAYLOAD)
    );
    println!(
        "[gateway-wsl] Interval: {} ms",
        config.interval.as_millis()
    );
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
        socket
            .send_to(PAYLOAD, &config.target)
            .map_err(|e| format!("Send failed at packet #{index}: {e}"))?;
        if config.wait_ack {
            let mut ack_buf = [0_u8; 1024];
            let (ack_size, ack_peer) = match socket.recv_from(&mut ack_buf) {
                Ok(v) => v,
                Err(err) if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock => {
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
                    "ACK mismatch at packet #{index}: got \"{}\" from {}, expected \"{}\"",
                    ack_payload, ack_peer, config.expected_ack
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
                println!("[gateway-wsl] Sent packet #{index}/inf to {}", config.target);
            }
        }

        index += 1;
        thread::sleep(config.interval);
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
