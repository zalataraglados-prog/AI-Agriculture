use std::env;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::time::Duration;

const DEFAULT_BIND: &str = "0.0.0.0:9000";
const DEFAULT_EXPECTED: &str = "success";
const DEFAULT_ONCE: bool = true;

#[derive(Debug)]
struct Config {
    bind: String,
    expected: String,
    once: bool,
    max_packets: Option<u64>,
    timeout: Option<Duration>,
}

fn print_usage(binary: &str) {
    eprintln!(
        "Usage:
  {binary} [--bind <ip:port>] [--expected <payload>] [--once] [--max-packets <n>] [--timeout-ms <ms>]

Defaults:
  --bind {DEFAULT_BIND}
  --expected {DEFAULT_EXPECTED}
  --once {DEFAULT_ONCE}

Notes:
  --once          Exit after first matching packet.
  --max-packets   Exit after receiving N packets (match or mismatch).
  --timeout-ms 0  Disable read timeout."
    );
}

fn parse_args() -> Result<Config, String> {
    let mut bind = DEFAULT_BIND.to_string();
    let mut expected = DEFAULT_EXPECTED.to_string();
    let mut once = DEFAULT_ONCE;
    let mut max_packets: Option<u64> = None;
    let mut timeout = Some(Duration::from_secs(30));

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                bind = args
                    .next()
                    .ok_or_else(|| "Missing value for --bind".to_string())?;
            }
            "--expected" => {
                expected = args
                    .next()
                    .ok_or_else(|| "Missing value for --expected".to_string())?;
            }
            "--once" => {
                once = true;
            }
            "--max-packets" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "Missing value for --max-packets".to_string())?;
                let value = raw
                    .parse::<u64>()
                    .map_err(|_| "Invalid --max-packets, expected unsigned integer".to_string())?;
                if value == 0 {
                    return Err("--max-packets must be >= 1".to_string());
                }
                max_packets = Some(value);
                once = false;
            }
            "--timeout-ms" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "Missing value for --timeout-ms".to_string())?;
                let ms = raw
                    .parse::<u64>()
                    .map_err(|_| "Invalid --timeout-ms, expected unsigned integer".to_string())?;
                timeout = if ms == 0 {
                    None
                } else {
                    Some(Duration::from_millis(ms))
                };
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    Ok(Config {
        bind,
        expected,
        once,
        max_packets,
        timeout,
    })
}

fn run(cfg: &Config) -> Result<(), String> {
    let socket = UdpSocket::bind(&cfg.bind).map_err(|e| format!("Bind failed on {}: {e}", cfg.bind))?;
    socket
        .set_read_timeout(cfg.timeout)
        .map_err(|e| format!("Failed to set read timeout: {e}"))?;

    println!("[cloud] Listening on {}", cfg.bind);
    println!("[cloud] Expected payload: \"{}\"", cfg.expected);
    println!(
        "[cloud] Mode: {}",
        if cfg.once {
            "exit after first success"
        } else {
            "continuous/limited receive"
        }
    );

    let mut buf = [0_u8; 2048];
    let mut received_count: u64 = 0;
    let mut success_count: u64 = 0;

    loop {
        let (size, peer) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(err) if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock => {
                return Err("Receive timeout reached without enough packets".to_string());
            }
            Err(err) => return Err(format!("Receive failed: {err}")),
        };

        received_count += 1;
        let payload = String::from_utf8_lossy(&buf[..size]).to_string();
        let is_match = payload == cfg.expected;

        if is_match {
            success_count += 1;
        }

        println!(
            "[cloud] Packet #{received_count} from {peer}: \"{payload}\" => {}",
            if is_match { "MATCH" } else { "MISMATCH" }
        );

        if cfg.once && is_match {
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

fn main() {
    let binary = env::args().next().unwrap_or_else(|| "cloud".to_string());
    let cfg = match parse_args() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Argument error: {err}\n");
            print_usage(&binary);
            std::process::exit(2);
        }
    };

    if let Err(err) = run(&cfg) {
        eprintln!("[cloud] ERROR: {err}");
        std::process::exit(1);
    }
}
