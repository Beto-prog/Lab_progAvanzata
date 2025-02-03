use crate::get_drone_impl;

use crate::node_stats::DroneStats;
use crate::ui_commands::UICommand;

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
    event_sender: Sender<DroneEvent>,
    // Network topology information
    network_topology: HashMap<NodeId, HashSet<NodeId>>,
    // Map of node IDs to their types
    node_types: HashMap<NodeId, NodeType>,
    // Index of the next drone implementation to use
    next_drone_impl_index: u8,

    drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,

    ui_command_receiver: Receiver<UICommand>,
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
        //ui_context: Context,
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
            event_sender,
            network_topology,
            node_types,
            next_drone_impl_index,
            drone_stats,
            ui_command_receiver,
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

    fn handle_ui_command(&mut self, ui_command: UICommand) {
        match ui_command {
            UICommand::SetPDR(node_id, pdr) => {
                self.set_packet_drop_rate(node_id, pdr);
            }
            UICommand::CrashDrone(node_id) => {
                self.crash_drone(node_id);
            }
        }
    }

    /// Handles events received from drones
    fn handle_event(&mut self, event: DroneEvent) {
        match event {
            DroneEvent::PacketSent(packet) => {
                let node_id = packet.routing_header.previous_hop().unwrap();
                if *self.node_types.get(&node_id).unwrap() == NodeType::Drone {
                    match packet.pack_type {
                        PacketType::MsgFragment(_) => {}
                        PacketType::Ack(_) => {
                            self.drone_stats
                                .lock()
                                .unwrap()
                                .get_mut(&node_id)
                                .unwrap()
                                .acks_forwarded += 1;
                        }
                        PacketType::Nack(_) => {
                            self.drone_stats
                                .lock()
                                .unwrap()
                                .get_mut(&node_id)
                                .unwrap()
                                .nacks_forwarded += 1;
                        }
                        PacketType::FloodResponse(_) => {
                            self.drone_stats
                                .lock()
                                .unwrap()
                                .get_mut(&node_id)
                                .unwrap()
                                .flood_responses_forwarded += 1;
                        }
                        PacketType::FloodRequest(_) => {
                            self.drone_stats
                                .lock()
                                .unwrap()
                                .get_mut(&node_id)
                                .unwrap()
                                .flood_requests_forwarded += 1;
                        }
                    }
                }
            }
            DroneEvent::PacketDropped(packet) => {
                let node_id = packet.routing_header.previous_hop().unwrap();
                self.drone_stats
                    .lock()
                    .unwrap()
                    .get_mut(&node_id)
                    .unwrap()
                    .packets_dropped += 1;
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
                sender.send(packet).unwrap();
            }
        }
    }

    /// Sends a command to a specific drone
    pub fn send_command(&self, drone_id: NodeId, command: DroneCommand) -> bool {
        if self.is_command_allowed(drone_id, &command) {
            if let Some(sender) = self.node_command_senders.get(&drone_id) {
                sender.send(command).unwrap();
                return true;
            }
        }
        false
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

        let mut neighbour_packet_senders = HashMap::new();

        for neighbour_id in &connected_node_ids {
            if let Some(neighbour_packet_sender) = self.node_packet_senders.get(&neighbour_id) {
                neighbour_packet_senders
                    .insert(neighbour_id.clone(), neighbour_packet_sender.clone());
            }
        }

        let mut drone = get_drone_impl::get_drone_impl(
            self.next_drone_impl_index,
            drone_id,
            self.event_sender.clone(),
            command_receiver,
            packet_receiver,
            neighbour_packet_senders,
            pdr,
        );

        self.next_drone_impl_index += 1;

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
                    .expect(&format!("neigbour {neighbor_id} should be valid"));
            }
        }
    }

    /// Removes a drone from the network
    pub fn remove_drone(&mut self, drone_id: NodeId) {
        self.node_command_senders.remove(&drone_id);
        self.node_packet_senders.remove(&drone_id);
        self.network_topology.remove(&drone_id);
        self.node_types.remove(&drone_id);

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
        self.drone_stats
            .lock()
            .unwrap()
            .get_mut(&drone_id)
            .unwrap()
            .pdr = pdr;
    }

    /// Crashes a drone
    pub fn crash_drone(&mut self, drone_id: NodeId) {
        if self.send_command(drone_id, DroneCommand::Crash) {
            self.remove_drone(drone_id);
            self.drone_stats
                .lock()
                .unwrap()
                .get_mut(&drone_id)
                .unwrap()
                .crashed = true;
        }
    }

    /// Adds a connection between two nodes
    pub fn add_connection(&mut self, node1: NodeId, node2: NodeId) {
        if !self.is_adding_connection_valid(node1) || !self.is_adding_connection_valid(node2) {
            return;
        }

        self.network_topology
            .entry(node1)
            .or_insert(HashSet::new())
            .insert(node2);
        self.network_topology
            .entry(node2)
            .or_insert(HashSet::new())
            .insert(node1);

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
    pub fn is_command_allowed(&self, destination: NodeId, command: &DroneCommand) -> bool {
        match command {
            DroneCommand::RemoveSender(node_id) => {
                // Check if removing the connection will disconnect the network
                self.is_network_connected_after_edge_removal(destination, *node_id)
            }
            DroneCommand::AddSender(_, _) => {
                // Check if adding the connection will violate client/server connection rules
                self.is_adding_connection_valid(destination)
            }
            DroneCommand::SetPacketDropRate(_) => {
                // Changing PDR does not affect network topology, so it's always allowed
                true
            }
            DroneCommand::Crash => {
                // Check if crashing the drone will disconnect the network
                self.is_network_connected_after_crash(destination)
            }
        }
    }

    /// Checks if the network remains connected after removing a node
    fn is_network_connected_after_edge_removal(
        &self,
        destination: NodeId,
        node_id: NodeId,
    ) -> bool {
        let mut topology = self.network_topology.clone();

        if let Some(neighbors) = topology.get_mut(&destination) {
            neighbors.remove(&node_id);
        }
        if let Some(neighbors) = topology.get_mut(&node_id) {
            neighbors.remove(&destination);
        }

        // Check if the network is still connected
        self.is_network_connected(&topology)
    }

    /// Checks if the network remains connected after crashing a drone
    fn is_network_connected_after_crash(&self, node_id: NodeId) -> bool {
        // Simulate the crash by removing the drone and checking connectivity
        let mut topology = self.network_topology.clone();
        let neighbors = topology.get(&node_id).unwrap().clone();
        for neighbor in neighbors {
            topology.get_mut(&neighbor).unwrap().remove(&node_id);
        }
        topology.remove(&node_id);

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
    fn is_adding_connection_valid(&self, node_id: NodeId) -> bool {
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

fn initialize_mock_network() -> SimulationController {
    use crossbeam_channel::unbounded;
    use std::collections::{HashMap, HashSet};
    use std::thread;

    // Create channels for the simulation controller

    let mut node_senders = HashMap::new();
    let mut node_recievers = HashMap::new();
    let mut drone_command_senders = HashMap::new();
    let mut drone_command_recievers = HashMap::new();

    let (event_sender, event_receiver) = unbounded();

    let (ui_command_sender, ui_command_receiver) = unbounded();

    let mut network_topology = HashMap::new();
    let mut node_types = HashMap::new();

    let mut drone_stats = HashMap::new();

    network_topology.insert(1, HashSet::from([2, 3, 5]));
    network_topology.insert(2, HashSet::from([1, 3, 4, 6]));
    network_topology.insert(3, HashSet::from([2, 1, 4, 6]));
    network_topology.insert(4, HashSet::from([3, 2]));
    network_topology.insert(5, HashSet::from([1]));
    network_topology.insert(6, HashSet::from([2, 3]));

    for i in 1..=3 {
        let (drone_send, drone_recv) = unbounded();
        let (command_send, command_recv) = unbounded();

        node_senders.insert(i, drone_send.clone());
        node_recievers.insert(i, drone_recv.clone());
        drone_command_senders.insert(i, command_send.clone());
        drone_command_recievers.insert(i, command_recv.clone());

        node_types.insert(i, NodeType::Drone);
    }

    // Create mock clients
    for i in 4..=5 {
        let (client_send, client_recv) = unbounded();

        node_senders.insert(i, client_send.clone());
        node_recievers.insert(i, client_recv.clone());
        node_types.insert(i, NodeType::Client);
    }

    // Create mock server
    let (server_send, server_recv) = unbounded();
    node_senders.insert(6, server_send.clone());
    node_recievers.insert(6, server_recv.clone());
    node_types.insert(6, NodeType::Server);

    // Create mock drones
    for i in 1..=3 {
        let mut neighbor_senders = HashMap::new();
        for neighbor in network_topology.get(&i).unwrap() {
            neighbor_senders.insert(*neighbor, node_senders.get(neighbor).unwrap().clone());
        }

        drone_stats.insert(
            i,
            DroneStats::new(network_topology.get(&i).unwrap().clone(), 0.1),
        );

        // Create the drone using the `get_drone_impl` function
        let mut drone = get_drone_impl::get_drone_impl(
            i, // Implementation index
            i, // Drone ID
            event_sender.clone(),
            drone_command_recievers.get(&i).unwrap().clone(),
            node_recievers.get(&i).unwrap().clone(),
            neighbor_senders.clone(), // Neighbor senders (empty for now)
            0.1,                      // PDR
        );

        // Spawn the drone thread
        thread::spawn(move || drone.run());
    }

    SimulationController::new(
        drone_command_senders,
        node_senders,
        event_receiver,
        event_sender,
        network_topology,
        vec![1, 2, 3],
        vec![4, 5],
        vec![6],
        0,
        Arc::new(Mutex::new(drone_stats)),
        ui_command_receiver,
    )
}

#[test]
fn test_send_command() {
    let controller = initialize_mock_network();
    controller.send_command(1, DroneCommand::SetPacketDropRate(0.5));
}

#[test]
fn test_add_drone() {
    let mut controller = initialize_mock_network();
    controller.add_drone(7, vec![1, 2], 0.1);
    assert!(controller.network_topology.contains_key(&7));
    assert_eq!(controller.node_types.get(&7), Some(&NodeType::Drone));
}

#[test]
fn test_remove_drone() {
    let mut controller = initialize_mock_network();
    controller.remove_drone(1);
    assert!(!controller.network_topology.contains_key(&1));
    assert!(!controller.node_types.contains_key(&1));
}

#[test]
fn test_set_packet_drop_rate() {
    let controller = initialize_mock_network();
    controller.set_packet_drop_rate(1, 0.5);
}

#[test]
fn test_crash_drone() {
    let mut controller = initialize_mock_network();
    controller.crash_drone(1);
    assert!(!controller.network_topology.contains_key(&1));
    assert!(!controller.node_types.contains_key(&1));
}

#[test]
fn test_add_connection() {
    let mut controller = initialize_mock_network();
    controller.add_connection(1, 3);
    assert!(controller.network_topology[&1].contains(&3));
    assert!(controller.network_topology[&3].contains(&1));
}

#[test]
fn test_remove_connection() {
    let mut controller = initialize_mock_network();
    controller.remove_connection(1, 2);
    assert!(!controller.network_topology[&1].contains(&2));
    assert!(!controller.network_topology[&2].contains(&1));
}

#[test]
fn test_is_command_allowed() {
    let controller = initialize_mock_network();
    assert!(controller.is_command_allowed(1, &DroneCommand::SetPacketDropRate(0.5)));
    assert!(!controller.is_command_allowed(1, &DroneCommand::RemoveSender(5)));
}

#[test]
fn test_is_network_connected_after_removal() {
    let controller = initialize_mock_network();
    assert!(controller.is_network_connected_after_edge_removal(1, 2));
}

#[test]
fn test_is_network_connected_after_crash() {
    let controller = initialize_mock_network();
    assert!(controller.is_network_connected_after_crash(2));
}

#[test]
fn test_is_network_connected() {
    let controller = initialize_mock_network();
    assert!(controller.is_network_connected(&controller.network_topology));
}

#[test]
fn test_is_connection_valid() {
    let controller = initialize_mock_network();
    assert!(controller.is_adding_connection_valid(1)); // Drone
    assert!(!controller.is_adding_connection_valid(4)); // Client
    assert!(controller.is_adding_connection_valid(6)); // Server
}

#[test]
fn test_add_drone_no_connections() {
    let mut controller = initialize_mock_network();
    controller.add_drone(8, vec![], 0.1);
    assert!(controller.network_topology.contains_key(&8));
    assert_eq!(controller.network_topology[&8].len(), 0);
    assert_eq!(controller.node_types.get(&8), Some(&NodeType::Drone));
}

#[test]
fn test_remove_non_existent_drone() {
    let mut controller = initialize_mock_network();
    let initial_topology = controller.network_topology.clone();
    controller.remove_drone(99); // Non-existent drone ID
    assert_eq!(controller.network_topology, initial_topology);
}

#[test]
fn test_add_connection_non_existent_nodes() {
    let mut controller = initialize_mock_network();
    let initial_topology = controller.network_topology.clone();
    controller.add_connection(99, 100); // Non-existent node IDs
    assert_eq!(controller.network_topology, initial_topology);
}

#[test]
fn test_remove_connection_non_existent_nodes() {
    let mut controller = initialize_mock_network();
    let initial_topology = controller.network_topology.clone();
    controller.remove_connection(99, 100); // Non-existent node IDs
    assert_eq!(controller.network_topology, initial_topology);
}

#[test]
fn test_add_client_with_more_than_two_connections() {
    let mut controller = initialize_mock_network();
    controller.add_connection(4, 1); // Client 4 is already connected to 1 drone
    controller.add_connection(4, 2); // Client 4 is now connected to 2 drones
    controller.add_connection(4, 3); // Attempt to connect to a third drone
    assert_eq!(controller.network_topology[&4].len(), 2); // Should still have only 2 connections
}

#[test]
fn test_add_drone_with_duplicate_connections() {
    let mut controller = initialize_mock_network();
    controller.add_drone(8, vec![1, 1, 2, 2], 0.1); // Duplicate connections
    assert!(controller.network_topology.contains_key(&8));
    assert_eq!(controller.network_topology[&8].len(), 2); // Only unique connections should be added
}

#[test]
fn test_remove_connection_client_drone() {
    let mut controller = initialize_mock_network();
    controller.remove_connection(4, 1); // Client 4 is connected to drone 1
    assert!(controller.network_topology[&4].len() >= 1); // Client should still have at least one connection
}

#[test]
fn test_add_connection_client_server() {
    let mut controller = initialize_mock_network();
    let initial_topology = controller.network_topology.clone();
    controller.add_connection(4, 6); // Client 4 and server 6
    assert_eq!(controller.network_topology, initial_topology); // Connection should not be added
}
