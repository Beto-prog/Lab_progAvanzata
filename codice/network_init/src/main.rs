#![allow(clippy::too_many_lines)]
mod config;
mod initializer;
mod ui;
mod validation;

use initializer::NetworkInitializer;

fn main() {
    // Initialize the network
    if let Err(e) = NetworkInitializer::run("network_config.toml") {
        eprintln!("Error initializing network: {e}");
        //return Err(Box::new(e));
    }
}
