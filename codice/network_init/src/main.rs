use crossbeam_channel::unbounded;
use serde::Deserialize;
use simulation_controller::SimulationController;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::thread;

use wg_2024::network::NodeId;

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

    fn get_network_topology(config: &NetworkConfig) -> HashMap<NodeId, HashSet<NodeId>> {
        let mut topology: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

        // Add drones to the topology
        for drone in &config.drone {
            let mut connected_nodes = HashSet::new();
            for &node_id in &drone.connected_node_ids {
                connected_nodes.insert(node_id);
            }
            topology.insert(drone.id, connected_nodes);
        }

        // Add clients to the topology
        for client in &config.client {
            let mut connected_nodes = HashSet::new();
            for &node_id in &client.connected_drone_ids {
                connected_nodes.insert(node_id);
            }
            topology.insert(client.id, connected_nodes);
        }

        // Add servers to the topology
        for server in &config.server {
            let mut connected_nodes = HashSet::new();
            for &node_id in &server.connected_drone_ids {
                connected_nodes.insert(node_id);
            }
            topology.insert(server.id, connected_nodes);
        }

        topology
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

            let mut drone = simulation_controller::get_drone_impl::get_drone_impl(
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

        let next_drone_impl_index = config.drone.len() % 10;

        let network_topology = Self::get_network_topology(&config);

        // Spawn simulation controller thread
        let mut simulation_controller = SimulationController::new(
            drone_command_senders,
            node_senders,
            controller_event_send,
            controller_event_recv,
            network_topology,
            config.drone.iter().map(|c| c.id).collect(),
            config.client.iter().map(|c| c.id).collect(),
            config.server.iter().map(|c| c.id).collect(),
            next_drone_impl_index as u8,
        );

        thread::spawn(move || simulation_controller.run());
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
