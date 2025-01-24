use crate::get_drone_impl;
use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::thread;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

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
    // Sender to pass events from drones to the controller
    event_sender: Sender<DroneEvent>,
    // Receiver for events from drones
    event_receiver: Receiver<DroneEvent>,
    // Network topology information
    network_topology: HashMap<NodeId, HashSet<NodeId>>,
    // Map of node IDs to their types
    node_types: HashMap<NodeId, NodeType>,
    // Index of the next drone implementation to use
    next_drone_impl_index: u8,
}

impl SimulationController {
    /// Creates a new Simulation Controller
    pub fn new(
        node_command_senders: HashMap<NodeId, Sender<DroneCommand>>,
        node_packet_senders: HashMap<NodeId, Sender<Packet>>,
        event_sender: Sender<DroneEvent>,
        event_receiver: Receiver<DroneEvent>,
        network_topology: HashMap<NodeId, HashSet<NodeId>>,
        drone_nodes: Vec<NodeId>,
        client_nodes: Vec<NodeId>,
        server_nodes: Vec<NodeId>,
        next_drone_impl_index: u8,
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
            event_sender,
            event_receiver,
            network_topology,
            node_types,
            next_drone_impl_index,
        }
    }

    /// Runs the simulation controller
    pub fn run(&mut self) {
        loop {
            // Handle events from drones
            if let Ok(event) = self.event_receiver.recv() {
                self.handle_event(event);
            }
        }
    }

    /// Handles events received from drones
    fn handle_event(&mut self, event: DroneEvent) {
        match event {
            DroneEvent::PacketSent(packet) => {
                println!("Packet sent: {:?}", packet);
            }
            DroneEvent::PacketDropped(packet) => {
                println!("Packet dropped: {:?}", packet);
            }
            DroneEvent::ControllerShortcut(packet) => {
                // Handle Ack, Nack, or FloodResponse packets that need to be sent directly to the destination
                self.send_packet_directly(packet);
            }
        }
    }

    /// Sends a packet directly to the destination using the simulation controller
    fn send_packet_directly(&self, packet: Packet) {
        if let Some(destination) = packet.routing_header.destination() {
            if let Some(sender) = self.node_packet_senders.get(&destination) {
                sender.send(packet).unwrap();
            }
        }
    }

    /// Sends a command to a specific drone
    pub fn send_command(&self, drone_id: NodeId, command: DroneCommand) {
        if self.is_command_allowed(&command) {
            if let Some(sender) = self.node_command_senders.get(&drone_id) {
                sender.send(command).unwrap();
            }
        }
    }

    /// Adds a new drone to the network
    pub fn add_drone(&mut self, drone_id: NodeId, connected_node_ids: Vec<NodeId>, pdr: f32) {
        // Create channels for the drone
        let (command_sender, command_receiver) = crossbeam_channel::unbounded();
        let (packet_sender, packet_receiver) = crossbeam_channel::unbounded();

        // Add the drone to the network topology
        self.network_topology
            .insert(drone_id, connected_node_ids.iter().cloned().collect());

        // Add the drone's type to the node_types map
        self.node_types.insert(drone_id, NodeType::Drone);

        self.node_command_senders
            .insert(drone_id, command_sender.clone());

        // Add the drone's packet sender to the controller's map
        self.node_packet_senders
            .insert(drone_id, packet_sender.clone());

        let mut drone = get_drone_impl::get_drone_impl(
            self.next_drone_impl_index,
            drone_id,
            self.event_sender.clone(),
            command_receiver,
            packet_receiver,
            self.node_packet_senders.clone(),
            pdr,
        );
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
                    .unwrap();
            }

            // Notify the new drone to add its neighbors as senders
            if let Some(neighbor_packet_sender) = self.node_packet_senders.get(&neighbor_id) {
                command_sender
                    .send(DroneCommand::AddSender(
                        neighbor_id,
                        neighbor_packet_sender.clone(),
                    ))
                    .unwrap();
            }
        }
    }

    /// Removes a drone from the network
    pub fn remove_drone(&mut self, drone_id: NodeId) {
        self.node_command_senders.remove(&drone_id);
        self.node_packet_senders.remove(&drone_id);
        self.network_topology.remove(&drone_id);

        // Notify neighbors to remove the crashed drone from their connections
        if let Some(neighbors) = self.network_topology.get(&drone_id) {
            for neighbor in neighbors {
                self.send_command(*neighbor, DroneCommand::RemoveSender(drone_id));
            }
        }
    }

    /// Updates the Packet Drop Rate (PDR) of a drone
    pub fn set_packet_drop_rate(&self, drone_id: NodeId, pdr: f32) {
        self.send_command(drone_id, DroneCommand::SetPacketDropRate(pdr));
    }

    /// Crashes a drone
    pub fn crash_drone(&mut self, drone_id: NodeId) {
        self.send_command(drone_id, DroneCommand::Crash);
        self.remove_drone(drone_id);
    }

    /// Adds a connection between two nodes
    pub fn add_connection(&mut self, node1: NodeId, node2: NodeId) {
        if let Some(neighbors) = self.network_topology.get_mut(&node1) {
            neighbors.insert(node2);
        }
        if let Some(neighbors) = self.network_topology.get_mut(&node2) {
            neighbors.insert(node1);
        }

        // Notify both nodes to add the connection
        if let Some(sender1) = self.node_command_senders.get(&node1) {
            sender1
                .send(DroneCommand::AddSender(
                    node2,
                    self.node_packet_senders[&node2].clone(),
                ))
                .unwrap();
        }
        if let Some(sender2) = self.node_command_senders.get(&node2) {
            sender2
                .send(DroneCommand::AddSender(
                    node1,
                    self.node_packet_senders[&node1].clone(),
                ))
                .unwrap();
        }
    }

    /// Removes a connection between two nodes
    pub fn remove_connection(&mut self, node1: NodeId, node2: NodeId) {
        if let Some(neighbors) = self.network_topology.get_mut(&node1) {
            neighbors.remove(&node2);
        }
        if let Some(neighbors) = self.network_topology.get_mut(&node2) {
            neighbors.remove(&node1);
        }

        // Notify both nodes to remove the connection
        self.send_command(node1, DroneCommand::RemoveSender(node2));
        self.send_command(node2, DroneCommand::RemoveSender(node1));
    }

    /// Checks if executing a command is allowed based on network requirements
    pub fn is_command_allowed(&self, command: &DroneCommand) -> bool {
        match command {
            DroneCommand::RemoveSender(node_id) => {
                // Check if removing the connection will disconnect the network
                self.is_network_connected_after_removal(*node_id)
            }
            DroneCommand::AddSender(node1, _) => {
                // Check if adding the connection will violate client/server connection rules
                self.is_connection_valid(*node1)
            }
            DroneCommand::SetPacketDropRate(_) => {
                // Changing PDR does not affect network topology, so it's always allowed
                true
            }
            DroneCommand::Crash => {
                // Check if crashing the drone will disconnect the network
                self.is_network_connected_after_crash()
            }
        }
    }

    /// Checks if the network remains connected after removing a node
    fn is_network_connected_after_removal(&self, node_id: NodeId) -> bool {
        let mut topology = self.network_topology.clone();
        topology.remove(&node_id);

        // Check if the network is still connected
        self.is_network_connected(&topology)
    }

    /// Checks if the network remains connected after crashing a drone
    fn is_network_connected_after_crash(&self) -> bool {
        // Simulate the crash by removing the drone and checking connectivity
        let mut topology = self.network_topology.clone();
        topology.retain(|&id, _| self.node_command_senders.contains_key(&id));

        // Check if the network is still connected
        self.is_network_connected(&topology)
    }

    /// Checks if the network is connected
    fn is_network_connected(&self, topology: &HashMap<NodeId, HashSet<NodeId>>) -> bool {
        if topology.is_empty() {
            return true; // Empty network is trivially connected
        }

        // Perform a Depth-First Search (DFS) to check connectivity
        let mut visited = HashSet::new();
        let start_node = *topology.keys().next().unwrap(); // Start from any node
        self.dfs(start_node, topology, &mut visited);

        // If all nodes are visited, the network is connected
        visited.len() == topology.len()
    }

    /// Depth-First Search (DFS) helper function
    fn dfs(
        &self,
        node: NodeId,
        topology: &HashMap<NodeId, HashSet<NodeId>>,
        visited: &mut HashSet<NodeId>,
    ) {
        visited.insert(node);
        if let Some(neighbors) = topology.get(&node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    self.dfs(*neighbor, topology, visited);
                }
            }
        }
    }

    /// Checks if adding a connection is valid for clients and servers
    fn is_connection_valid(&self, node_id: NodeId) -> bool {
        if let Some(node_type) = self.node_types.get(&node_id) {
            match node_type {
                NodeType::Client => {
                    // Clients must have 1-2 drone connections
                    self.network_topology
                        .get(&node_id)
                        .map_or(false, |neighbors| neighbors.len() < 2)
                }
                NodeType::Server => {
                    // Servers must have at least 2 drone connections
                    true // Adding more connections is always allowed for servers
                }
                NodeType::Drone => {
                    // Drones can have any number of connections
                    true
                }
            }
        } else {
            false // Node not found in topology
        }
    }
}
