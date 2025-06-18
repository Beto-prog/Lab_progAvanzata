#![allow(clippy::too_many_lines)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::too_many_arguments)]

use common::get_drone_impl;

use crate::node_stats::DroneStats;
use crate::ui_commands::{UICommand, UIResponse};

use crossbeam_channel::{select_biased, Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use wg_2024::packet::PacketType;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Client,
    Drone,
    Server,
}

/// Simulation Controller
pub struct SimulationController {
    // Map of node IDs to their respective senders for commands
    node_command_senders: HashMap<NodeId, Sender<DroneCommand>>,
    // Map of node IDs to their respective senders for packets
    node_packet_senders: HashMap<NodeId, Sender<Packet>>,
    // Receiver for events from drones
    event_receiver: Receiver<DroneEvent>,
    // Sender for events to drones
    _event_sender: Sender<DroneEvent>,
    // Network topology information
    network_topology: HashMap<NodeId, HashSet<NodeId>>,
    // Map of node IDs to their types
    node_types: HashMap<NodeId, NodeType>,
    // Index of the next drone implementation to use
    _next_drone_impl_index: u8,

    drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,

    ui_command_receiver: Receiver<UICommand>,

    ui_response_sender: Sender<UIResponse>,

    crash_event_senders: Vec<Sender<NodeId>>,
}

impl SimulationController {
    /// Creates a new Simulation Controller
    pub fn new(
        node_command_senders: HashMap<NodeId, Sender<DroneCommand>>,
        node_packet_senders: HashMap<NodeId, Sender<Packet>>,
        event_receiver: Receiver<DroneEvent>,
        event_sender: Sender<DroneEvent>,
        network_topology: HashMap<NodeId, HashSet<NodeId>>,
        drone_nodes: Vec<NodeId>,
        client_nodes: Vec<NodeId>,
        server_nodes: Vec<NodeId>,
        next_drone_impl_index: u8,
        drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,

        ui_command_receiver: Receiver<UICommand>,
        ui_response_sender: Sender<UIResponse>,

        crash_event_senders: Vec<Sender<NodeId>>,
    ) -> Self {
        let mut node_types = HashMap::new();

        for drone_id in drone_nodes {
            node_types.insert(drone_id, NodeType::Drone);
        }

        for client_id in client_nodes {
            node_types.insert(client_id, NodeType::Client);
        }

        for server_id in server_nodes {
            node_types.insert(server_id, NodeType::Server);
        }

        Self {
            node_command_senders,
            node_packet_senders,
            event_receiver,
            _event_sender: event_sender,
            network_topology,
            node_types,
            _next_drone_impl_index: next_drone_impl_index,
            drone_stats,
            ui_command_receiver,
            ui_response_sender,
            crash_event_senders,
        }
    }

    /// Runs the simulation controller
    pub fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.event_receiver) -> event => {
                   if let Ok (event) = event {
                        self.handle_event(event);
                }
                }
                recv(self.ui_command_receiver) -> ui_command => {
                    if let Ok (ui_command) = ui_command {

                    self.handle_ui_command(ui_command);
                    }
                }
            }
        }
    }

    /// Handles a UI command.
    fn handle_ui_command(&mut self, ui_command: UICommand) {
        match ui_command {
            UICommand::SetPDR(node_id, pdr) => {
                self.set_packet_drop_rate(node_id, pdr);
            }
            UICommand::CrashDrone(node_id) => {
                self.crash_drone(node_id);
            }
            UICommand::AddConnection(node1, node2) => {
                self.add_connection(node1, node2);
            }
            UICommand::RemoveConnection(node1, node2) => {
                self.remove_connection(node1, node2);
            }
        }
    }

    /// Handles events received from drones
    fn handle_event(&mut self, event: DroneEvent) {
        match event {
            DroneEvent::PacketSent(packet) => {
                let node_id: NodeId;
                match packet.pack_type {
                    PacketType::MsgFragment(_) => {
                        node_id = packet
                            .routing_header
                            .previous_hop()
                            .expect("there should always be a previous hop");
                        if *self
                            .node_types
                            .get(&node_id)
                            .expect("Previous hops should always exist")
                            == NodeType::Drone
                        {
                            self.drone_stats
                                .lock()
                                .expect("Should be able to unlock")
                                .get_mut(&node_id)
                                .expect("The node Id should be valid")
                                .fragments_forwarded += 1;
                        }
                    }
                    PacketType::Ack(_) => {
                        node_id = packet
                            .routing_header
                            .previous_hop()
                            .expect("there should always be a previous hop");

                        if *self
                            .node_types
                            .get(&node_id)
                            .expect("Previous hops should always exist")
                            == NodeType::Drone
                        {
                            self.drone_stats
                                .lock()
                                .expect("Should be able to unlock")
                                .get_mut(&node_id)
                                .expect("Previous hops should always exist")
                                .acks_forwarded += 1;
                        }
                    }
                    PacketType::Nack(_) => {
                        node_id = packet
                            .routing_header
                            .previous_hop()
                            .expect("there should always be a previous hop");
                        if *self
                            .node_types
                            .get(&node_id)
                            .expect("Previous hops should always exist")
                            == NodeType::Drone
                        {
                            self.drone_stats
                                .lock()
                                .expect("Should be able to unlock")
                                .get_mut(&node_id)
                                .expect("Previous hops should always exist")
                                .nacks_forwarded += 1;
                        }
                    }
                    PacketType::FloodRequest(flood_request) => {
                        node_id = flood_request
                            .path_trace
                            .last()
                            .expect("Flood requests should have a last hop")
                            .0;

                        if *self
                            .node_types
                            .get(&node_id)
                            .expect("A node inside a flood request should exist")
                            == NodeType::Drone
                        {
                            self.drone_stats
                                .lock()
                                .expect("Should be able to unlock")
                                .get_mut(&node_id)
                                .expect("The node Id should be valid")
                                .flood_requests_forwarded += 1;
                        }
                    }
                    PacketType::FloodResponse(flood_response) => {
                        node_id = flood_response
                            .path_trace
                            .last()
                            .expect("Flood requests should have a last hop")
                            .0;

                        if *self
                            .node_types
                            .get(&node_id)
                            .expect("A node inside a flood request should exist")
                            == NodeType::Drone
                        {
                            self.drone_stats
                                .lock()
                                .expect("Should be able to unlock")
                                .get_mut(&node_id)
                                .expect("The node Id should be valid")
                                .flood_responses_forwarded += 1;
                        }
                    }
                }
                if *self
                    .node_types
                    .get(&node_id)
                    .expect("Node id should be valid")
                    == NodeType::Drone
                {
                    self.drone_stats
                        .lock()
                        .expect("Should be able to unlock")
                        .get_mut(&node_id)
                        .expect("The node Id should be valid")
                        .packets_forwarded += 1;
                }
            }
            DroneEvent::PacketDropped(packet) => {
                let node_id = packet
                    .routing_header
                    .previous_hop()
                    .expect("Previous hop should always be valid");
                if let Some(stats) = self
                    .drone_stats
                    .lock()
                    .expect("Should always be able to unlock")
                    .get_mut(&node_id)
                {
                    stats.packets_dropped += 1;
                }
            }
            DroneEvent::ControllerShortcut(packet) => {
                self.send_packet_directly(packet);
            }
        }
    }

    /// Sends a packet directly to the destination using the simulation controller
    fn send_packet_directly(&self, packet: Packet) {
        if let Some(destination) = packet.routing_header.destination() {
            if let Some(sender) = self.node_packet_senders.get(&destination) {
                sender
                    .send(packet)
                    .expect("Clients should always be able to reeive packets");
            }
        }
    }

    /// Sends a command to a specific drone
    fn send_command(&self, drone_id: NodeId, command: &DroneCommand) -> Result<(), String> {
        self.is_command_allowed(drone_id, command)?;
        if let Some(sender) = self.node_command_senders.get(&drone_id) {
            sender
                .send(command.clone())
                .map_err(|_| format!("Failed to send command to drone {}", drone_id))?;
            Ok(())
        } else {
            Err(format!("Drone {} does not exist in the network", drone_id))
        }
    }

    /// Adds a new drone to the network
    fn _add_drone(&mut self, drone_id: NodeId, connected_node_ids: Vec<NodeId>, pdr: f32) {
        // Create channels for the drone
        let (command_sender, command_receiver) = crossbeam_channel::unbounded();
        let (packet_sender, packet_receiver) = crossbeam_channel::unbounded();

        // Add the drone to the network topology
        self.network_topology
            .insert(drone_id, connected_node_ids.iter().copied().collect());

        // Add the drone's type to the node_types map
        self.node_types.insert(drone_id, NodeType::Drone);

        self.node_command_senders
            .insert(drone_id, command_sender.clone());

        // Add the drone's packet sender to the controller's map
        self.node_packet_senders
            .insert(drone_id, packet_sender.clone());

        let mut neighbour_packet_senders = HashMap::new();

        for neighbour_id in &connected_node_ids {
            if let Some(neighbour_packet_sender) = self.node_packet_senders.get(neighbour_id) {
                neighbour_packet_senders.insert(*neighbour_id, neighbour_packet_sender.clone());
            }
        }

        let mut drone = get_drone_impl::get_drone_impl(
            self._next_drone_impl_index,
            drone_id,
            self._event_sender.clone(),
            command_receiver,
            packet_receiver,
            neighbour_packet_senders,
            pdr,
        );

        self._next_drone_impl_index += 1;

        // Spawn a thread for the drone
        thread::spawn(move || {
            drone.run();
        });

        // Add the drone's command sender to the controller's map

        // Notify neighbors to add the new drone as a sender
        for neighbor_id in connected_node_ids {
            if let Some(neighbor_sender) = self.node_command_senders.get(&neighbor_id) {
                neighbor_sender
                    .send(DroneCommand::AddSender(drone_id, packet_sender.clone()))
                    .unwrap_or_else(|_| panic!("neigbour {neighbor_id} should be valid"));
            }
        }
    }

    /// Removes a drone from the network
    fn remove_drone(&mut self, drone_id: NodeId) {
        // Notify neighbors to remove the crashed drone from their connections
        if let Some(neighbors) = self.network_topology.get(&drone_id) {
            for neighbor in neighbors {
                // We ignore errors here since the neighbor might already be disconnected
                let _ = self.send_command(*neighbor, &DroneCommand::RemoveSender(drone_id));
            }
        }
        //self.node_command_senders.remove(&drone_id);
        self.node_packet_senders.remove(&drone_id);
        self.network_topology.remove(&drone_id);
        //self.node_types.remove(&drone_id);
    }

    /// Updates the Packet Drop Rate (PDR) of a drone
    fn set_packet_drop_rate(&self, drone_id: NodeId, pdr: f32) {
        match self.send_command(drone_id, &DroneCommand::SetPacketDropRate(pdr)) {
            Ok(()) => {
                self.drone_stats
                    .lock()
                    .expect("Should be able to unlock")
                    .get_mut(&drone_id)
                    .expect("Should always be able to change pdr")
                    .pdr = pdr;

                self.ui_response_sender
                    .send(UIResponse::Success(
                        "Packet drop rate successfully updated".to_string(),
                    ))
                    .expect("Should be able to send");
            }
            Err(e) => {
                self.ui_response_sender
                    .send(UIResponse::Falure(e))
                    .expect("Should be able to send");
            }
        }
    }

    /// Crashes a drone
    fn crash_drone(&mut self, drone_id: NodeId) {
        match self.send_command(drone_id, &DroneCommand::Crash) {
            Ok(()) => {
                self.remove_drone(drone_id);
                for sender in self.crash_event_senders.iter() {
                    sender
                        .send(drone_id)
                        .expect("Should be able to send crash event");
                }
                self.drone_stats
                    .lock()
                    .expect("Should be able to unlock")
                    .get_mut(&drone_id)
                    .expect("Should always be able to change crashed")
                    .crashed = true;
                self.ui_response_sender
                    .send(UIResponse::Success(
                        "Drone successfully crashed".to_string(),
                    ))
                    .expect("Should be able to send");
            }
            Err(e) => {
                self.ui_response_sender
                    .send(UIResponse::Falure(format!(
                        "If this drone crashed, this would happen: {}",
                        e
                    )))
                    .expect("Should be able to send");
            }
        }
    }

    /// Adds a connection between two nodes
    fn add_connection(&mut self, node1: NodeId, node2: NodeId) {
        match self.is_adding_connection_valid(node1, node2) {
            Ok(()) => {
                self.network_topology
                    .entry(node1)
                    .or_default()
                    .insert(node2);
                self.network_topology
                    .entry(node2)
                    .or_default()
                    .insert(node1);

                // Notify both nodes to add the connection
                if let Some(sender1) = self.node_command_senders.get(&node1) {
                    sender1
                        .send(DroneCommand::AddSender(
                            node2,
                            self.node_packet_senders[&node2].clone(),
                        ))
                        .expect("Neighbor sender should be valid");
                }
                if let Some(sender2) = self.node_command_senders.get(&node2) {
                    sender2
                        .send(DroneCommand::AddSender(
                            node1,
                            self.node_packet_senders[&node1].clone(),
                        ))
                        .expect("Neighbor sender should be valid");
                }

                //update the stats
                self.drone_stats
                    .lock()
                    .expect("Should be able to unlock")
                    .get_mut(&node1)
                    .expect("The node Id should be valid")
                    .neigbours
                    .insert(node2);
                self.drone_stats
                    .lock()
                    .expect("Should be able to unlock")
                    .get_mut(&node2)
                    .expect("The node Id should be valid")
                    .neigbours
                    .insert(node1);

                //send response to the UI
                self.ui_response_sender
                    .send(UIResponse::Success(
                        "Connection successfully added".to_string(),
                    ))
                    .expect("Should be able to send");
            }
            Err(e) => {
                self.ui_response_sender
                    .send(UIResponse::Falure(e))
                    .expect("Should be able to send");
            }
        }
    }

    /// Removes a connection between two nodes
    fn remove_connection(&mut self, node1: NodeId, node2: NodeId) {
        match self.is_connection_removal_valid(node1, node2) {
            Ok(()) => {
                if let Some(neighbors) = self.network_topology.get_mut(&node1) {
                    neighbors.remove(&node2);
                }
                if let Some(neighbors) = self.network_topology.get_mut(&node2) {
                    neighbors.remove(&node1);
                }

                // Notify both nodes to remove the connection
                // We ignore errors here since the nodes might already be disconnected
                let _ = self.send_command(node1, &DroneCommand::RemoveSender(node2));
                let _ = self.send_command(node2, &DroneCommand::RemoveSender(node1));

                //update drone stats
                if let Some(NodeType::Drone) = self.node_types.get_mut(&node1) {
                    self.drone_stats
                        .lock()
                        .expect("Should be able to unlock")
                        .get_mut(&node1)
                        .expect("The node Id should be valid")
                        .neigbours
                        .remove(&node2);
                }
                if let Some(NodeType::Drone) = self.node_types.get_mut(&node2) {
                    self.drone_stats
                        .lock()
                        .expect("Should be able to unlock")
                        .get_mut(&node2)
                        .expect("The node Id should be valid")
                        .neigbours
                        .remove(&node1);
                }

                self.ui_response_sender
                    .send(UIResponse::Success(
                        "Connection successfully removed".to_string(),
                    ))
                    .expect("Should be able to send");
            }
            Err(e) => {
                self.ui_response_sender
                    .send(UIResponse::Falure(e))
                    .expect("Should be able to send");
            }
        }
    }

    /// Checks if executing a command is allowed based on network requirements
    fn is_command_allowed(
        &self,
        destination: NodeId,
        command: &DroneCommand,
    ) -> Result<(), String> {
        match command {
            DroneCommand::RemoveSender(node_id) => {
                // Check if removing the connection will disconnect the network
                self.is_connection_removal_valid(destination, *node_id)
            }
            DroneCommand::AddSender(node_id, _) => {
                // Check if adding the connection will violate client/server connection rules
                self.is_adding_connection_valid(destination, *node_id)
            }
            DroneCommand::SetPacketDropRate(_) => {
                // Changing PDR does not affect network topology, so it's always allowed
                Ok(())
            }
            DroneCommand::Crash => {
                // Check if crashing the drone will disconnect the network
                self.is_network_connected_after_crash(destination)
            }
        }
    }

    /// Checks if the network remains connected after removing an edge
    fn is_connection_removal_valid(
        &self,
        destination: NodeId,
        node_id: NodeId,
    ) -> Result<(), String> {
        let mut topology = self.network_topology.clone();

        if let Some(neighbors) = topology.get_mut(&destination) {
            neighbors.remove(&node_id);
        } else {
            return Err(format!(
                "Node {} does not exist in the network",
                destination
            ));
        }
        if let Some(neighbors) = topology.get_mut(&node_id) {
            neighbors.remove(&destination);
        } else {
            return Err(format!("Node {} does not exist in the network", node_id));
        }

        // Check if the network is still connected
        Self::check_topology(&topology, &self.node_types)
    }

    /// Checks if the network remains connected after crashing a drone
    fn is_network_connected_after_crash(&self, node_id: NodeId) -> Result<(), String> {
        // Simulate the crash by removing the drone and checking connectivity
        let mut topology = self.network_topology.clone();

        let neighbors = topology
            .get(&node_id)
            .ok_or_else(|| format!("Node {} does not exist in the network", node_id))?
            .clone();
        for neighbor in neighbors {
            if let Some(neighbors) = topology.get_mut(&neighbor) {
                neighbors.remove(&node_id);
            }
        }
        topology.remove(&node_id);
        let mut node_types = self.node_types.clone();
        node_types.remove(&node_id);
        // Check if the network is still connected
        Self::check_topology(&topology, &node_types)
    }

    fn check_topology(
        topology: &HashMap<NodeId, HashSet<NodeId>>,
        node_types: &HashMap<NodeId, NodeType>,
    ) -> Result<(), String> {
        // Check client and server connections
        for (node_id, node_type) in node_types {
            match node_type {
                NodeType::Client => {
                    let Some(neighbors) = topology.get(node_id) else {
                        return Err(format!("Client {} has no connections", node_id));
                    };
                    if !neighbors
                        .iter()
                        .all(|n| node_types.get(n) == Some(&NodeType::Drone))
                    {
                        return Err(format!(
                            "Client {} is connected to non-drone nodes",
                            node_id
                        ));
                    }
                    if neighbors.is_empty() {
                        return Err(format!("Client {} has no connections", node_id));
                    }
                    if neighbors.len() > 2 {
                        return Err(format!("Client {} has more than 2 connections", node_id));
                    }
                }
                NodeType::Server => {
                    let Some(neighbors) = topology.get(node_id) else {
                        return Err(format!("Server {} has no connections", node_id));
                    };
                    if !neighbors
                        .iter()
                        .all(|n| node_types.get(n) == Some(&NodeType::Drone))
                    {
                        return Err(format!(
                            "Server {} is connected to non-drone nodes",
                            node_id
                        ));
                    }
                    if neighbors.len() < 2 {
                        return Err(format!("Server {} has fewer than 2 connections", node_id));
                    }
                }
                NodeType::Drone => {}
            }
        }

        // Check entire network connectivity
        if !Self::is_network_connected(topology) {
            return Err("Network is not fully connected".to_string());
        }

        // Check drones-only subgraph connectivity
        let drones: HashSet<NodeId> = node_types
            .iter()
            .filter(|(_, t)| **t == NodeType::Drone)
            .map(|(id, _)| *id)
            .collect();

        if drones.is_empty() {
            return Err("No drones in the network".to_string());
        }

        let mut drones_topology = HashMap::new();
        for drone_id in &drones {
            let neighbors = topology.get(drone_id).cloned().unwrap_or_default();
            drones_topology.insert(
                *drone_id,
                neighbors.intersection(&drones).copied().collect(),
            );
        }

        let mut visited = HashSet::new();
        let start_drone = *drones
            .iter()
            .next()
            .expect("It should be impossible to have no drones");
        Self::dfs_drones(start_drone, &drones_topology, &mut visited);

        if visited.len() != drones.len() {
            return Err("Drone subgraph is not fully connected".to_string());
        }

        Ok(())
    }

    /// Checks if the entire network is connected.
    fn is_network_connected(topology: &HashMap<NodeId, HashSet<NodeId>>) -> bool {
        if topology.is_empty() {
            return true;
        }
        let mut visited = HashSet::new();
        let start_node = *topology
            .keys()
            .next()
            .expect("There should always be at least a node");
        Self::dfs(start_node, topology, &mut visited);
        visited.len() == topology.len()
    }

    /// DFS helper for entire network.
    fn dfs(
        node: NodeId,
        topology: &HashMap<NodeId, HashSet<NodeId>>,
        visited: &mut HashSet<NodeId>,
    ) {
        if visited.insert(node) {
            if let Some(neighbors) = topology.get(&node) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        Self::dfs(*neighbor, topology, visited);
                    }
                }
            }
        }
    }

    /// DFS helper for drones-only subgraph.
    fn dfs_drones(
        node: NodeId,
        drones_topology: &HashMap<NodeId, HashSet<NodeId>>,
        visited: &mut HashSet<NodeId>,
    ) {
        if visited.insert(node) {
            if let Some(neighbors) = drones_topology.get(&node) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        Self::dfs_drones(*neighbor, drones_topology, visited);
                    }
                }
            }
        }
    }

    /// Checks if adding a connection is valid for clients and servers
    fn is_adding_connection_valid(&self, node_id1: NodeId, node_id2: NodeId) -> Result<(), String> {
        let mut topology = self.network_topology.clone();

        let insert1_successful = match topology.get_mut(&node_id1) {
            Some(neighbors) => neighbors.insert(node_id2),
            None => return Err(format!("Node {} does not exist in the network", node_id1)),
        };

        let insert2_successful = match topology.get_mut(&node_id2) {
            Some(neighbors) => neighbors.insert(node_id1),
            None => return Err(format!("Node {} does not exist in the network", node_id2)),
        };

        if insert1_successful && insert2_successful {
            return Self::check_topology(&topology, &self.node_types);
        }

        Err("Connection already exists".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn _initialize_mock_network() -> SimulationController {
        use crossbeam_channel::unbounded;
        use std::collections::{HashMap, HashSet};
        use std::thread;

        let mut node_senders = HashMap::new();
        let mut node_recievers = HashMap::new();
        let mut drone_command_senders = HashMap::new();
        let mut drone_command_recievers = HashMap::new();

        let (event_sender, event_receiver) = unbounded();

        let (ui_command_sender, ui_command_receiver) = unbounded();
        let (ui_response_sender, ui_response_receiver) = unbounded();

        let mut network_topology = HashMap::new();

        let mut drone_stats = HashMap::new();

        network_topology.insert(1, HashSet::from([2, 3, 4, 6]));
        network_topology.insert(2, HashSet::from([1, 3, 5, 7]));
        network_topology.insert(3, HashSet::from([2, 1, 5, 7]));
        network_topology.insert(4, HashSet::from([1]));
        network_topology.insert(5, HashSet::from([3, 2]));
        network_topology.insert(6, HashSet::from([1, 2]));
        network_topology.insert(7, HashSet::from([3, 2]));

        //create mock drones
        for i in 1..=4 {
            let (drone_send, drone_recv) = unbounded();
            let (command_send, command_recv) = unbounded();

            node_senders.insert(i, drone_send.clone());
            node_recievers.insert(i, drone_recv.clone());
            drone_command_senders.insert(i, command_send.clone());
            drone_command_recievers.insert(i, command_recv.clone());
        }

        // Create mock clients
        for i in 5..=6 {
            let (client_send, client_recv) = unbounded();

            node_senders.insert(i, client_send.clone());
            node_recievers.insert(i, client_recv.clone());
        }

        // Create mock server
        let (server_send, server_recv) = unbounded();
        node_senders.insert(7, server_send.clone());
        node_recievers.insert(7, server_recv.clone());

        // Create mock drones
        for i in 1..=4 {
            let mut neighbor_senders = HashMap::new();
            for neighbor in network_topology
                .get(&i)
                .expect("Should always be able to get neighbors")
            {
                neighbor_senders.insert(
                    *neighbor,
                    node_senders
                        .get(neighbor)
                        .expect("Should always be able to get neighbors senders")
                        .clone(),
                );
            }

            drone_stats.insert(
                i,
                DroneStats::new(
                    network_topology
                        .get(&i)
                        .expect("Should always be able to get drone")
                        .clone(),
                    0.1,
                ),
            );

            // Create the drone using the `get_drone_impl` function
            let mut drone = get_drone_impl::get_drone_impl(
                i, // Implementation index
                i, // Drone ID
                event_sender.clone(),
                drone_command_recievers
                    .get(&i)
                    .expect("Should always be able to get command receiver")
                    .clone(),
                node_recievers
                    .get(&i)
                    .expect("Should always be able to get node receiver")
                    .clone(),
                neighbor_senders.clone(), // Neighbor senders (empty for now)
                0.1,                      // PDR
            );

            // Spawn the drone thread
            thread::spawn(move || drone.run());

            struct MockUi {
                _receiver: Receiver<UIResponse>,
                _sender: Sender<UICommand>,
            }

            impl MockUi {
                fn _run(&self) {
                    loop {}
                }
            }

            let mock_ui = MockUi {
                _receiver: ui_response_receiver.clone(),
                _sender: ui_command_sender.clone(),
            };

            thread::spawn(move || mock_ui._run());
        }

        SimulationController::new(
            drone_command_senders,
            node_senders,
            event_receiver,
            event_sender,
            network_topology,
            vec![1, 2, 3, 4],
            vec![5, 6],
            vec![7],
            0,
            Arc::new(Mutex::new(drone_stats)),
            ui_command_receiver,
            ui_response_sender,
        )
    }

    #[test]
    fn test_send_command() {
        let controller = _initialize_mock_network();
        controller.send_command(1, &DroneCommand::SetPacketDropRate(0.5));
    }

    #[test]
    fn test_add_drone() {
        let mut controller = _initialize_mock_network();
        controller._add_drone(7, vec![1, 2], 0.1);
        assert!(controller.network_topology.contains_key(&7));
        assert_eq!(controller.node_types.get(&7), Some(&NodeType::Drone));
    }

    #[test]
    fn test_remove_drone() {
        let mut controller = _initialize_mock_network();
        controller.remove_drone(1);
        assert!(!controller.network_topology.contains_key(&1));
        assert!(!controller.node_types.contains_key(&1));
    }

    #[test]
    fn test_set_packet_drop_rate() {
        let controller = _initialize_mock_network();
        controller.set_packet_drop_rate(1, 0.5);
    }

    #[test]
    fn test_crash_drone() {
        let mut controller = _initialize_mock_network();
        controller.crash_drone(4);
        assert!(!controller.network_topology.contains_key(&4));
        assert!(!controller.node_types.contains_key(&4));
    }

    #[test]
    fn test_is_network_connected_after_crash() {
        let controller = _initialize_mock_network();
        assert!(controller.is_network_connected_after_crash(4).is_ok());
    }

    #[test]
    fn test_add_connection() {
        let mut controller = _initialize_mock_network();
        controller.add_connection(1, 3);
        assert!(controller.network_topology[&1].contains(&3));
        assert!(controller.network_topology[&3].contains(&1));
    }

    #[test]
    fn test_remove_connection() {
        let mut controller = _initialize_mock_network();
        controller.remove_connection(1, 2);
        assert!(!controller.network_topology[&1].contains(&2));
        assert!(!controller.network_topology[&2].contains(&1));

        controller.remove_connection(2, 7);
        assert!(controller.network_topology[&2].contains(&7));
        assert!(controller.network_topology[&7].contains(&2));
    }

    #[test]
    fn test_is_network_connected_after_removal() {
        let controller = _initialize_mock_network();
        assert!(controller.is_connection_removal_valid(1, 2).is_ok());
    }

    #[test]
    fn test_is_network_connected() {
        let controller = _initialize_mock_network();
        assert!(SimulationController::check_topology(
            &controller.network_topology,
            &controller.node_types
        )
        .is_ok());
    }

    #[test]
    fn test_is_connection_valid() {
        let controller = _initialize_mock_network();
        assert!(controller.is_adding_connection_valid(4, 2).is_ok()); // add connection between drones
        assert!(!controller.is_adding_connection_valid(5, 6).is_ok()); // add connection between two clients
        assert!(!controller.is_adding_connection_valid(7, 6).is_ok()); // add connection between server and client
        assert!(!controller.is_adding_connection_valid(5, 1).is_ok()); // add more than two connections to a client
    }

    #[test]
    fn test_add_drone_no_connections() {
        let mut controller = _initialize_mock_network();
        controller._add_drone(8, vec![], 0.1);
        assert!(controller.network_topology.contains_key(&8));
        assert_eq!(controller.network_topology[&8].len(), 0);
        assert_eq!(controller.node_types.get(&8), Some(&NodeType::Drone));
    }

    #[test]
    fn test_remove_non_existent_drone() {
        let mut controller = _initialize_mock_network();
        let initial_topology = controller.network_topology.clone();
        controller.remove_drone(99); // Non-existent drone ID
        assert_eq!(controller.network_topology, initial_topology);
    }

    #[test]
    fn test_add_connection_non_existent_nodes() {
        let mut controller = _initialize_mock_network();
        let initial_topology = controller.network_topology.clone();
        controller.add_connection(99, 100); // Non-existent node IDs
        assert_eq!(controller.network_topology, initial_topology);
    }

    #[test]
    fn test_remove_connection_non_existent_nodes() {
        let mut controller = _initialize_mock_network();
        let initial_topology = controller.network_topology.clone();
        controller.remove_connection(99, 100); // Non-existent node IDs
        assert_eq!(controller.network_topology, initial_topology);
    }

    #[test]
    fn test_add_client_with_more_than_two_connections() {
        let mut controller = _initialize_mock_network();
        controller.add_connection(5, 1); // Client 4 is already connected to 1 drone
        controller.add_connection(5, 2); // Client 4 is now connected to 2 drones
        controller.add_connection(5, 3); // Attempt to connect to a third drone
        assert_eq!(controller.network_topology[&5].len(), 2); // Should still have only 2 connections
    }

    #[test]
    fn test_add_drone_with_duplicate_connections() {
        let mut controller = _initialize_mock_network();
        controller._add_drone(8, vec![1, 1, 2, 2], 0.1); // Duplicate connections
        assert!(controller.network_topology.contains_key(&8));
        assert_eq!(controller.network_topology[&8].len(), 2); // Only unique connections should be added
    }

    #[test]
    fn test_remove_connection_client_drone() {
        let mut controller = _initialize_mock_network();
        controller.remove_connection(5, 1); // Client 4 is connected to drone 1
        assert!(controller.network_topology[&5].len() >= 1); // Client should still have at least one connection
    }

    #[test]
    fn test_add_connection_client_server() {
        let mut controller = _initialize_mock_network();
        let initial_topology = controller.network_topology.clone();
        controller.add_connection(5, 7); // Client 4 and server 6
        assert_eq!(controller.network_topology, initial_topology); // Connection should not be added
    }
}
