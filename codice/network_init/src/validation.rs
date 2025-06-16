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

    if let Err(e) = is_graph_connected(config) {
        return Err(e);
    }

    Ok(())
}

fn is_graph_connected(config: &NetworkConfig) -> Result<(), String> {
    let adjacency_list = build_adjacency_list(config);
    
    // Check for bidirectional connections
    if let Err(e) = is_bidirectional(&adjacency_list) {
        return Err(e);
    }
    
    // Check for full connectivity
    if let Err(e) = check_connectivity(&adjacency_list) {
        return Err(e);
    }

    Ok(())
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

fn is_bidirectional(adjacency_list: &HashMap<NodeId, Vec<NodeId>>) -> Result<(), String> {
    for (node_id, neighbors) in adjacency_list {
        for neighbor_id in neighbors {
            if !adjacency_list.get(neighbor_id).map_or(false, |ns| ns.contains(node_id)) {
                return Err(format!(
                    "Connection is not bidirectional: node {} connects to {} but not vice versa",
                    node_id, neighbor_id
                ));
            }
        }
    }
    Ok(())
}

fn check_connectivity(adjacency_list: &HashMap<NodeId, Vec<NodeId>>) -> Result<(), String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut disconnected_nodes = Vec::new();

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

    // Find all nodes that weren't visited
    for node_id in adjacency_list.keys() {
        if !visited.contains(node_id) {
            disconnected_nodes.push(*node_id);
        }
    }

    if !disconnected_nodes.is_empty() {
        return Err(format!(
            "Network is not fully connected. The following nodes are disconnected: {:?}",
            disconnected_nodes
        ));
    }

    Ok(())
}