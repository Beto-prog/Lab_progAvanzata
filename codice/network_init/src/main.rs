use std::thread;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct NetworkConfig {
    drone: Vec<Drone>,
    client: Vec<Client>,
    server: Vec<Server>,
}

#[derive(Debug, Deserialize)]
struct Drone {
    id: u8,
    connected_node_ids: Vec<u8>,
    pdr: f32,
}

#[derive(Debug, Deserialize)]
struct Client {
    id: u8,
    connected_drone_ids: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct Server {
    id: u8,
    connected_drone_ids: Vec<u8>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    // Read toml file
    let toml_content = std::fs::read_to_string("network_config.toml")?;

    // Deserializza il contenuto in una struttura Rust
    let config: NetworkConfig = toml::from_str(&toml_content)?;

    
    // Stampa i dettagli
    println!("{:#?}", config.client);
    thread::spawn(move || {
        drone.run();
    });
    Ok(())
}
