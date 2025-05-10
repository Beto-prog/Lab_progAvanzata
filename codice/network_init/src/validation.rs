use super::config::NetworkConfig;
use std::collections::{HashMap, HashSet, VecDeque};
use wg_2024::network::NodeId;

pub fn validate_config(config: &NetworkConfig) -> Result<(), String> {
    let mut node_ids = HashSet::new();

    // Validate drones
    for drone in &config.drone {
        if !node_ids.insert(drone.id) {
            return Err(format!("Duplicate node ID: {}", drone.id));
        }
        if drone.connected_node_ids.contains(&drone.id) {
            return Err(format!("Drone {} is connected to itself", drone.id));
        }
        if drone.connected_node_ids.iter().collect::<HashSet<_>>().len() != drone.connected_node_ids.len() {
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
        if client.connected_drone_ids.is_empty() || client.connected_drone_ids.len() > 2 {
            return Err(format!("Client {} must be connected to 1 or 2 drones", client.id));
        }
        if client.connected_drone_ids.iter().collect::<HashSet<_>>().len() != client.connected_drone_ids.len() {
            return Err(format!("Client {} has duplicate connections", client.id));
        }
    }

    // Validate servers
    for server in &config.server {
        if !node_ids.insert(server.id) {
            return Err(format!("Duplicate node ID: {}", server.id));
        }
        if server.connected_drone_ids.len() < 2 {
            return Err(format!("Server {} must be connected to at least 2 drones", server.id));
        }
        if server.connected_drone_ids.iter().collect::<HashSet<_>>().len() != server.connected_drone_ids.len() {
            return Err(format!("Server {} has duplicate connections", server.id));
        }
    }

    if !is_graph_connected(config) {
        return Err("Network is not bidirectionally connected".to_string());
    }

    Ok(())
}

fn is_graph_connected(config: &NetworkConfig) -> bool {
    let adjacency_list = build_adjacency_list(config);
    is_bidirectional(&adjacency_list) && check_connectivity(&adjacency_list)
}

fn build_adjacency_list(config: &NetworkConfig) -> HashMap<NodeId, Vec<NodeId>> {
    let mut adjacency_list = HashMap::new();

    for drone in &config.drone {
        adjacency_list.insert(drone.id, drone.connected_node_ids.clone());
    }
    for client in &config.client {
        adjacency_list.insert(client.id, client.connected_drone_ids.clone());
    }
    for server in &config.server {
        adjacency_list.insert(server.id, server.connected_drone_ids.clone());
    }

    adjacency_list
}

fn is_bidirectional(adjacency_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    for (node_id, neighbors) in adjacency_list {
        for neighbor_id in neighbors {
            if !adjacency_list.get(neighbor_id).map_or(false, |ns| ns.contains(node_id)) {
                return false;
            }
        }
    }
    true
}

fn check_connectivity(adjacency_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    if let Some(&start_node) = adjacency_list.keys().next() {
        queue.push_back(start_node);
        visited.insert(start_node);
    }

    while let Some(node) = queue.pop_front() {
        if let Some(neighbors) = adjacency_list.get(&node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(*neighbor);
                    queue.push_back(*neighbor);
                }
            }
        }
    }

    visited.len() == adjacency_list.len()
}