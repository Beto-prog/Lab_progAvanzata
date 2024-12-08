use std::thread;
use serde::Deserialize;
use TrustDrone::TrustDrone as TrsDrone;
use wg_2024::drone::Drone as DroneTrait;
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
/*
fn start_drone <T> ( done : T) where T : DroneTrait
{
    thread::spawn(move || {
        // drone.run();
    });   
}*/

fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    // Read toml file
    let toml_content = std::fs::read_to_string("network_config.toml")?;

    
    // Serialize data 
    let config: NetworkConfig = toml::from_str(&toml_content)?;
    
    
    for drone in config.drone {

        
        //spawn drone 
        thread::spawn(move || {
            //start_drone (drone);
            // copy server
        });
        
    }
    
    for client in config.client {
        
        //spawn client 
        thread::spawn(move || {
            // copy drone
        });
    }
    
    for server in config.server {
        //spawn server 
        thread::spawn(move || {
            // copy server
        });
    }
    
    
  
    Ok(())
}
