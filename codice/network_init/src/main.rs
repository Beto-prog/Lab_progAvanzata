use bagel_bomber::BagelBomber;
use crossbeam_channel::{unbounded, Receiver, Sender};
use getdroned::GetDroned;
use lockheedrustin_drone::LockheedRustin;
use null_pointer_drone::MyDrone as NullPointerDrone;
use rust_do_it::RustDoIt;
use rust_roveri::RustRoveri;
use rustafarian_drone::RustafarianDrone;
use rustastic_drone::RustasticDrone;
use rusty_drones::RustyDrone;
use serde::Deserialize;
use simulation_controller::SimulationController;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use trust_drone::TrustDrone;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

#[derive(Debug, Deserialize)]
struct DroneConfig {
    id: NodeId,
    connected_node_ids: Vec<NodeId>,
    pdr: f32,
}

#[derive(Debug, Deserialize)]
struct ClientConfig {
    id: NodeId,
    connected_drone_ids: Vec<NodeId>,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    id: NodeId,
    connected_drone_ids: Vec<NodeId>,
}

#[derive(Debug, Deserialize)]
struct NetworkConfig {
    drone: Vec<DroneConfig>,
    client: Vec<ClientConfig>,
    server: Vec<ServerConfig>,
}

struct NetworkInitializer;

impl NetworkInitializer {
    fn read_config(file_path: &str) -> Result<NetworkConfig, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string(file_path)?;
        let config: NetworkConfig = toml::from_str(&config_str)?;
        Ok(config)
    }

    fn is_graph_connected(config: &NetworkConfig) -> bool {
        let mut adjacency_list: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        // Build adjacency list
        for drone in &config.drone {
            adjacency_list.insert(drone.id, drone.connected_node_ids.clone());
        }
        for client in &config.client {
            adjacency_list.insert(client.id, client.connected_drone_ids.clone());
        }
        for server in &config.server {
            adjacency_list.insert(server.id, server.connected_drone_ids.clone());
        }

        // Perform BFS to check connectivity
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(&start_node) = adjacency_list.keys().next() {
            queue.push_back(start_node);
            visited.insert(start_node);
        }

        while let Some(node) = queue.pop_front() {
            if let Some(neighbors) = adjacency_list.get(&node) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // Check if all nodes are visited
        visited.len() == adjacency_list.len()
    }

    fn validate_config(config: &NetworkConfig) -> Result<(), String> {
        let mut node_ids = HashSet::new();

        // Validate drones
        for drone in &config.drone {
            if !node_ids.insert(drone.id) {
                return Err(format!("Duplicate node ID: {}", drone.id));
            }
            if drone.connected_node_ids.contains(&drone.id) {
                return Err(format!("Drone {} is connected to itself", drone.id));
            }
            if drone
                .connected_node_ids
                .iter()
                .collect::<HashSet<_>>()
                .len()
                != drone.connected_node_ids.len()
            {
                return Err(format!("Drone {} has duplicate connections", drone.id));
            }
            if drone.pdr < 0.0 || drone.pdr > 1.0 {
                return Err(format!("Drone {} has invalid PDR: {}", drone.id, drone.pdr));
            }
        }

        // Validate clients
        for client in &config.client {
            if !node_ids.insert(client.id) {
                return Err(format!("Duplicate node ID: {}", client.id));
            }
            if client.connected_drone_ids.len() < 1 || client.connected_drone_ids.len() > 2 {
                return Err(format!(
                    "Client {} must be connected to 1 or 2 drones",
                    client.id
                ));
            }
            if client
                .connected_drone_ids
                .iter()
                .collect::<HashSet<_>>()
                .len()
                != client.connected_drone_ids.len()
            {
                return Err(format!("Client {} has duplicate connections", client.id));
            }
        }

        // Validate servers
        for server in &config.server {
            if !node_ids.insert(server.id) {
                return Err(format!("Duplicate node ID: {}", server.id));
            }
            if server.connected_drone_ids.len() < 2 {
                return Err(format!(
                    "Server {} must be connected to at least 2 drones",
                    server.id
                ));
            }
            if server
                .connected_drone_ids
                .iter()
                .collect::<HashSet<_>>()
                .len()
                != server.connected_drone_ids.len()
            {
                return Err(format!("Server {} has duplicate connections", server.id));
            }
        }

        NetworkInitializer::is_graph_connected(config);

        Ok(())
    }

    fn initialize_network(config: NetworkConfig) {
        let (controller_event_send, controller_event_recv) = unbounded();
        let mut node_senders = HashMap::new();
        let mut node_recievers = HashMap::new();
        let mut drone_command_senders = HashMap::new();
        let mut drone_command_recievers = HashMap::new();
        //let mut client_senders = HashMap::new();
        //let mut server_senders = HashMap::new();

        // Initialize drones
        for drone_config in &config.drone {
            let (drone_send, drone_recv) = unbounded();
            let (command_send, command_recv) = unbounded();

            node_senders.insert(drone_config.id, drone_send.clone());
            node_recievers.insert(drone_config.id, drone_recv.clone());
            drone_command_senders.insert(drone_config.id, command_send.clone());
            drone_command_recievers.insert(drone_config.id, command_recv.clone());
        }

        for client_config in &config.client {
            let (client_send, client_recv) = unbounded();

            node_senders.insert(client_config.id, client_send.clone());
            node_recievers.insert(client_config.id, client_recv.clone());
        }

        for server_config in &config.server {
            let (server_send, server_recv) = unbounded();

            node_senders.insert(server_config.id, server_send.clone());
            node_recievers.insert(server_config.id, server_recv.clone());
        }

        for (index, drone_config) in config.drone.iter().enumerate() {
            let mut neighbor_senders = HashMap::new();

            for neighbor_id in &drone_config.connected_node_ids {
                neighbor_senders.insert(
                    neighbor_id.clone(),
                    node_senders.get(&neighbor_id).unwrap().clone(),
                );
            }

            let mut drone = NetworkInitializer::get_drone_impl(
                index as u8,
                drone_config.id,
                controller_event_send.clone(),
                drone_command_recievers
                    .get(&drone_config.id)
                    .unwrap()
                    .clone(),
                node_recievers.get(&drone_config.id).unwrap().clone(),
                neighbor_senders.clone(),
                drone_config.pdr,
            );

            std::thread::spawn(move || drone.run());
        }

        // Initialize clients
        for client_config in &config.client {
            //let (client_send, client_recv) = unbounded();
            //client_senders.insert(client_config.id, client_send.clone());

            std::thread::spawn(move || {
                // TODO: Implement client logic
            });
        }

        // Initialize servers
        for server_config in &config.server {
            //let (server_send, server_recv) = unbounded();
            //server_senders.insert(server_config.id, server_send.clone());
            std::thread::spawn(move || {
                // TODO: Implement server logic
            });
        }

        // Spawn simulation controller thread
        let simulation_controller = SimulationController::new(
            controller_event_recv,
            drone_command_senders,
            config.drone.iter().map(|c| c.id).collect(),
            config.client.iter().map(|c| c.id).collect(),
            config.server.iter().map(|c| c.id).collect(),
        );
        std::thread::spawn(move || {
            simulation_controller.run();
        });
    }

    fn get_drone_impl(
        index: u8,
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Drone + Send> {
        let index = index % 10 + 1;

        match index {
            1 => Box::new(BagelBomber::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            2 => Box::new(LockheedRustin::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            3 => Box::new(NullPointerDrone::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            4 => Box::new(RustafarianDrone::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            5 => Box::new(RustasticDrone::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            6 => Box::new(GetDroned::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            7 => Box::new(RustyDrone::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            8 => Box::new(RustDoIt::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            9 => Box::new(RustRoveri::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            10 => Box::new(TrustDrone::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                pdr,
            )),
            _ => panic!("id should always be in range 1-10"),
        }
    }

    pub fn run(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config = Self::read_config(file_path)?;
        Self::validate_config(&config)?;
        Self::initialize_network(config);
        Ok(())
    }
}

fn main() {
    if let Err(e) = NetworkInitializer::run("network_config.toml") {
        eprintln!("Error initializing network: {}", e);
    }
    loop {}
}
