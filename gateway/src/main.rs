mod config;
mod constants;
mod gateway;
mod serial;

use std::env;

use config::{parse_args, print_usage};
use gateway::run;

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
