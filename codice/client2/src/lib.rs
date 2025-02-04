#![allow(warnings)]

mod repackager;

use std::collections::{HashMap, HashSet, VecDeque, BTreeMap};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use wg_2024::packet::*;
use wg_2024::network::*;
use crossbeam_channel::Sender;
use crate::repackager::Repackager;

pub struct Client2 {
    node_id: NodeId,
    discovered_drones: Arc<Mutex<HashMap<NodeId, NodeType>>>,
    neighbor_senders: HashMap<NodeId, Sender<Packet>>,
    network_graph: Arc<Mutex<HashMap<NodeId, HashSet<NodeId>>>>,
    server: Option<NodeId>,
    sent_packets: HashMap<u64, Packet>, // Store sent packets by session_id
    repackager: Repackager,
}

impl Client2 {
    pub fn new(node_id: NodeId, neighbor_senders: HashMap<NodeId, Sender<Packet>>) -> Self {
        Self {
            node_id,
            discovered_drones: Arc::new(Mutex::new(HashMap::new())),
            neighbor_senders,
            network_graph: Arc::new(Mutex::new(HashMap::new())),
            server: None,
            sent_packets: HashMap::new(), // Initialize the sent packets map
            repackager: Repackager::new(),
        }
    }
    // Discover network through drones
    pub fn discover_network(&self) {
        let flood_id = self.generate_flood_id();
        let request = FloodRequest {
            flood_id,
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };

        for (drone_id, sender) in &self.neighbor_senders {
            println!("CLIENT2: Sending FloodRequest to Drone {}", drone_id);
            sender.send(Packet {
                pack_type: PacketType::FloodRequest(request.clone()),
                routing_header: self.create_source_routing_header(*drone_id),
                session_id: self.generate_session_id(),
            }).unwrap();
        }
    }

    // Handle a received FloodResponse and build the network graph
    pub fn handle_flood_response(&mut self, response: FloodResponse) {
        println!("CLIENT2: Client {} received FloodResponse: {:?}", self.node_id, response);
        let mut discovered_drones = self.discovered_drones.lock().unwrap();
        let mut network_graph = self.network_graph.lock().unwrap();

        // Update the discovered drones list
        for (node_id, node_type) in &response.path_trace {
            if node_type == &NodeType::Drone {
                discovered_drones.insert(*node_id, node_type.clone());
            }
            if node_type == &NodeType::Server {
                self.server = Some(*node_id);
            }
        }

        // Update the network graph (adjacency list)
        for i in 0..response.path_trace.len() - 1 {
            let (node_a_id, _) = &response.path_trace[i];
            let (node_b_id, _) = &response.path_trace[i + 1];

            // Add an edge between node_a and node_b
            network_graph.entry(*node_a_id).or_insert_with(HashSet::new).insert(*node_b_id);
            network_graph.entry(*node_b_id).or_insert_with(HashSet::new).insert(*node_a_id);
        }
    }

    // Send a message to a server through drones
    pub fn send_message(&mut self, server_id: NodeId, message: &str, file_path: Option<&str>) {
        // Create fragments using the Repackager
        let fragments = Repackager::create_fragments(message, file_path).expect("Failed to create fragments");

        let session_id = self.generate_session_id();

        for fragment in fragments {
            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header: self.create_source_routing_header(server_id),
                session_id,
            };

            // Store the sent packet
            self.sent_packets.insert(session_id, packet.clone());

            self.forward_packet(packet);
        }
    }

    // Forward packet to the first drone in the routing path
    fn forward_packet(&self, packet: Packet) {
        if let Some(first_hop) = packet.routing_header.hops.get(1) {
            if let Some(sender) = self.neighbor_senders.get(first_hop) {
                println!("CLIENT2: Client {} forwarding packet to Drone {}", self.node_id, first_hop);
                sender.send(packet).unwrap();
            } else {
                println!("CLIENT2: Drone {} not found in neighbors", first_hop);
            }
        } else {
            println!("CLIENT2: No valid routing path found for packet");
        }
    }

    // Create a source routing header from client to server through discovered drones
    fn create_source_routing_header(&self,
                                    destination: NodeId) -> SourceRoutingHeader {
        let discovered_drones = self.discovered_drones.lock().unwrap();
        let mut hops = vec![self.node_id];

        if let Some((&drone_id, _)) = discovered_drones.iter().next() {
            hops.push(drone_id);
        }
        hops.push(destination);

        SourceRoutingHeader {
            hop_index: 1,
            hops,
        }
    }

    // Handle received packet (Ack, Nack, etc.)
    pub fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::MsgFragment(fragment) => {
                self.handle_msg_fragment(fragment, packet.session_id)
            }
            PacketType::FloodResponse(response) => self.handle_flood_response(response),
            PacketType::Ack(ack) => {
                println!("CLIENT2: Client {} received Ack: {:?}", self.node_id, ack);
            }
            PacketType::Nack(nack) => {
                //Check again all nodes and connections
                self.discovered_drones = Arc::new(Mutex::new(HashMap::new()));
                self.network_graph = Arc::new(Mutex::new(HashMap::new()));
                self.discover_network();
                println!("CLIENT2: Client {} received Nack: {:?}", self.node_id, nack);
                // Resend the original packet
                if let Some(original_packet) = self.sent_packets.get(&packet.session_id) {
                    println!("CLIENT2: Resending packet for session ID {}", packet.session_id);
                    self.forward_packet(original_packet.clone());
                } else {
                    println!("CLIENT2: No original packet found for session ID {}", packet.session_id);
                }
            }
            _ => {
                println!("CLIENT2: Client {} received unknown packet type", self.node_id);
            }
        }
    }

    pub fn handle_msg_fragment(&mut self, fragment: Fragment, message_id: u64) {
        // Call process_fragment to handle the incoming fragment
        let session_id = self.generate_session_id(); // Assuming you have access to session_id
        let src_id = self.node_id as u64; // Assuming src_id is the node_id of the client

        match self.repackager.process_fragment(session_id, src_id, fragment) {
            Ok(Some(reassembled_message)) => {
                // Process the complete message
                let msg = Repackager::assemble_string(reassembled_message);
                println!("CLIENT2: Converted fragments into message: {:?}", msg);
            }
            Ok(None) => {
                println!("CLIENT2: Not all fragments received yet for message ID {}", message_id);
            }
            Err(error_code) => {
                println!("CLIENT2: Error processing fragment: {}", error_code);
            }
        }
    }

    /// Function to find the shortest path between two nodes using BFS.
    ///
    /// # Arguments:
    /// * `graph`: A reference to the graph represented as an adjacency list.
    /// * `start`: The starting node.
    /// * `goal`: The target node.
    ///
    /// # Returns:
    /// * `Some(Vec<NodeId>)`: The shortest path as a vector of nodes if a path exists.
    /// * `None`: If no path exists.
    fn bfs_shortest_path(graph: &HashMap<NodeId, HashSet<NodeId>>,
                         start: NodeId,
                         goal: NodeId) -> Option<Vec<NodeId>> {
        let mut visited: HashSet<NodeId> = HashSet::new();  // Keep track of visited nodes
        let mut queue: VecDeque<Vec<NodeId>> = VecDeque::new();  // Queue to store paths

        // Start BFS from the starting node
        queue.push_back(vec![start]);
        visited.insert(start);

        while let Some(path) = queue.pop_front() {
            let node = *path.last().unwrap();  // Get the current node

            // If we reached the goal, return the path
            if node == goal {
                return Some(path);
            }

            // Explore neighbors
            if let Some(neighbors) = graph.get(&node) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        let mut new_path = path.clone();
                        new_path.push(neighbor);
                        queue.push_back(new_path);
                        visited.insert(neighbor);
                    }
                }
            }
        }

        // If we exhaust the queue without finding the goal
        None
    }

    // Helpers
    fn generate_flood_id(&self) -> u64 {
        rand::random()
    }

    fn generate_session_id(&self) -> u64 {
        rand::random()
    }

    pub fn run(&mut self) {
        self.discover_network();
    }
}