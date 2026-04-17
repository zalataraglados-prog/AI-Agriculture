mod config;
mod constants;
mod datasource;
mod gateway;
mod persist;
mod protocol;
mod serial;

use std::env;

use config::{parse_args, print_usage};
use gateway::run_command;

fn main() {
    let binary = env::args().next().unwrap_or_else(|| "gateway".to_string());
    let command = match parse_args() {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("Argument error: {err}\n");
            print_usage(&binary);
            std::process::exit(2);
        }
    };

    if let Err(err) = run_command(command) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
