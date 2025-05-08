#![allow(clippy::too_many_lines)]
mod config;
mod initializer;
mod validation;

use initializer::NetworkInitializer;

fn main() {
    if let Err(e) = NetworkInitializer::run("network_config.toml") {
        eprintln!("Error initializing network: {e}");
    }
}
