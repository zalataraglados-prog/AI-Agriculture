use std::env;
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

const DEFAULT_TARGET: &str = "127.0.0.1:9000";
const DEFAULT_COUNT: u32 = 1;
const DEFAULT_INTERVAL_MS: u64 = 1000;
const PAYLOAD: &[u8] = b"success";

#[derive(Debug)]
struct Config {
    target: String,
    count: u32,
    interval: Duration,
}

fn print_usage(binary: &str) {
    eprintln!(
        "Usage:\n  {binary} [--target <ip:port>] [--count <n>] [--interval-ms <ms>]\n\nDefaults:\n  --target {DEFAULT_TARGET}\n  --count {DEFAULT_COUNT}\n  --interval-ms {DEFAULT_INTERVAL_MS}\n\nPacket payload is fixed as: \"success\""
    );
}

fn parse_args() -> Result<Config, String> {
    let mut target = DEFAULT_TARGET.to_string();
    let mut count = DEFAULT_COUNT;
    let mut interval_ms = DEFAULT_INTERVAL_MS;

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
                count = value
                    .parse::<u32>()
                    .map_err(|_| "Invalid --count, expected unsigned integer".to_string())?;
            }
            "--interval-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Missing value for --interval-ms".to_string())?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| "Invalid --interval-ms, expected unsigned integer".to_string())?;
            }
            "-h" | "--help" => {
                let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
                print_usage(&binary);
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    if count == 0 {
        return Err("--count must be >= 1".to_string());
    }

    Ok(Config {
        target,
        count,
        interval: Duration::from_millis(interval_ms),
    })
}

fn run(config: &Config) -> Result<(), String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind local UDP socket: {e}"))?;

    println!(
        "[gateway-wsl] Start sending Orange Pi Zero3 simulated packets -> {}",
        config.target
    );
    println!(
        "[gateway-wsl] Payload fixed to \"{}\"",
        String::from_utf8_lossy(PAYLOAD)
    );

    for index in 1..=config.count {
        socket
            .send_to(PAYLOAD, &config.target)
            .map_err(|e| format!("Send failed at packet #{index}: {e}"))?;

        println!(
            "[gateway-wsl] Sent packet #{index}/{} to {}",
            config.count, config.target
        );

        if index < config.count {
            thread::sleep(config.interval);
        }
    }

    println!("[gateway-wsl] Done.");
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
