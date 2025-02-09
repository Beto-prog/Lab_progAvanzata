#![allow(warnings)]

mod repackager;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use wg_2024::packet::*;
use wg_2024::network::*;
use crossbeam_channel::{Sender, Receiver};
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use crate::repackager::Repackager;


pub struct Client2 {
    node_id: NodeId,
    discovered_drones: Arc<Mutex<HashMap<NodeId, NodeType>>>,
    neighbor_senders: HashMap<NodeId, Sender<Packet>>,
    network_graph: Arc<Mutex<HashMap<NodeId, HashSet<NodeId>>>>,
    server: NodeId,
    server_type: String,
    sent_packets: HashMap<u64, Packet>, // Store sent packets by session_id
    repackager: Repackager,
    receiver_channel: Receiver<Packet>,
    received_floods: HashSet<u64>,
    saved_files: HashSet<String>,
    //reader: BufReader<TcpStream>,
    //writer: TcpStream,
}

impl Client2 {
    pub fn new(node_id: NodeId, neighbor_senders: HashMap<NodeId, Sender<Packet>>, receiver_channel: Receiver<Packet>) -> Self {
        //let (reader, writer) = setup_window();
        Self {
            node_id,
            discovered_drones: Arc::new(Mutex::new(HashMap::new())),
            neighbor_senders,
            network_graph: Arc::new(Mutex::new(HashMap::new())),
            server: 255,
            server_type: "".to_string(),
            sent_packets: HashMap::new(), // Initialize the sent packets map
            repackager: Repackager::new(),
            receiver_channel,
            received_floods: HashSet::new(),
            saved_files: HashSet::new(),
            //reader,
            //writer,
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

    pub fn create_flood_response(&self, session_id: u64, flood_request: FloodRequest) -> Packet {
        let mut hops = vec![];
        for (node_id, node_type) in flood_request.path_trace.clone() {
            hops.push((node_id));
        }

        let response = FloodResponse {
            flood_id: flood_request.flood_id,
            path_trace: flood_request.path_trace,
        };
        let srh = SourceRoutingHeader::with_first_hop(hops);

        Packet::new_flood_response(srh, session_id, response)
    }

    pub fn handle_flood_request(&mut self, flood_request: FloodRequest, session_id: u64) {
        println!("Client {} received FloodRequest: {:?}", self.node_id, flood_request);

        // Check if this flood request has already been processed
        if self.received_floods.contains(&flood_request.flood_id) {
            println!("Client {} already processed FloodRequest with ID {}", self.node_id, flood_request.flood_id);
            return;  // Skip processing if already handled
        }

        // Mark this flood request as processed
        self.received_floods.insert(flood_request.flood_id);

        // Create and send the flood response back
        let response_packet = self.create_flood_response(session_id, flood_request);
        self.forward_packet(response_packet);
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
                self.server = *node_id;
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
        let path = Self::bfs_shortest_path(self.network_graph.clone(), self.node_id, server_id);

        for fragment in fragments {
            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header: SourceRoutingHeader::with_first_hop(path.clone().unwrap()),
                session_id,
            };

            // Store the sent packet
            self.sent_packets.insert(session_id, packet.clone());

            self.forward_packet(packet);
        }
    }

    //Handle of commands
    pub fn handle_command(&mut self, command: &str) {
        match command {
            cmd if cmd == "server_type?2" || cmd == "files_list?" || cmd == "registration_to_chat" || cmd == "client_list?" => {
                //server_type?
                //files_list?
                //registration_to_chat
                //client_list?
                self.send_message(self.server, cmd, None);
            }
            cmd if cmd.starts_with("file?(") && cmd.ends_with(")") => {
                //file?(file_id)
                let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.send_message(self.server, cmd, None);
            }
            cmd if cmd.starts_with("media?(") && cmd.ends_with(")") => {
                //media?(media_id)
                let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.send_message(self.server, cmd, None);
            }
            cmd if cmd.starts_with("message_for?(") && cmd.ends_with(")") => {
                //message_for?(client_id, message)
                let text = cmd.strip_prefix("message_for?(").and_then(|s| s.strip_suffix(")"));
                let mut val = text.unwrap().split_once(", ");
                self.send_message(self.server, cmd, None);
            }
            cmd if cmd == "commands" => {
                "commands:\
                                             server_type?\
                                             files_list?\
                                             registration_to_chat\
                                             file?(file_id)\
                                             media?(media_id)\
                                             message_for?(client_id, message)";
            }
            _ => { "Wrong command, type 'commands' to see the full list of the commands."; }
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

    //Function that handles messages from the server
    pub fn handle_messages(&mut self, message: String, session_id: u64, sender: NodeId) {
        match message {
            msg if msg.starts_with("server_type!(") && msg.ends_with(")") => {
                //server_type!(type)
                let txt = msg.strip_prefix("server_type!(").and_then(|s| s.strip_suffix(")"));
                self.server_type = txt.unwrap().to_string();
            }
            msg if msg.starts_with("files_list!(") && msg.ends_with(")") => {
                //files_list!(list_of_file_ids)
                let files = msg.strip_prefix("files_list!(").and_then(|s| s.strip_suffix(")"));
                let ff = files.unwrap().split(",").map(|s| s.to_string()).collect::<Vec<String>>();
                //TODO SHOW FILES
            }
            msg if msg.starts_with("file!(") && msg.ends_with(")") => {
                //file!(file_size, file)
                let txt = msg.strip_prefix("server_type!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.unwrap().split_once(", ");
                self.saved_files.insert(values.unwrap().1.to_string());
            }
            msg if msg.starts_with("media!(") && msg.ends_with(")") => {
                //media!(media)
                // TODO
                let txt = msg.strip_prefix("media!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.unwrap().split_once(", ");
            }
            msg if msg == "error_requested_not_found!" || msg == "error_unsupported_request!" || msg == "error_wrong_client_id!" => {
                //error_requested_not_found!
                //error_unsupported_request!
                //error_wrong_client_id!
                //TODO send to view
            }
            msg if msg.starts_with("client_list!(") && msg.ends_with(")") => {
                //client_list!(list_of_client_ids)
                let txt = msg.strip_prefix("client_list!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.unwrap().split(", ").map(|s| s.to_string()).collect::<Vec<String>>();
            }
            msg if msg.starts_with("message_from!(") && msg.ends_with(")") => {
                //message_from!(client_id, message)
                let txt = msg.strip_prefix("media!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.unwrap().split_once(", ");
                //TODO show message
            }

            _ => {} //TODO VIEW ERROR
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
            PacketType::MsgFragment(ref fragment) => {
                self.handle_msg_fragment(fragment.clone(), packet.clone());
                self.forward_packet(Packet::new_ack(SourceRoutingHeader::with_first_hop(Self::bfs_shortest_path(self.network_graph.clone(), self.node_id, self.server).unwrap()), packet.session_id, 0))
            },
            PacketType::FloodRequest(request) => self.handle_flood_request(request, packet.session_id),
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

    pub fn handle_msg_fragment(&mut self, fragment: Fragment, packet: Packet) {
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
                println!("CLIENT2: Not all fragments received yet for message ID {}", packet.session_id);
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
    fn bfs_shortest_path(graph: Arc<Mutex<HashMap<NodeId, HashSet<NodeId>>>>,
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

            // Lock the graph to access neighbors
            if let Ok(graph) = graph.lock() {
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
            } else {
                println!("Failed to lock the graph");
                return None;
            }
        }

        None  // No path found
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
/*

fn setup_window() -> (BufReader<TcpStream>, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    // Launch terminal with persistent shell
    std::process::Command::new("xterm")
        .args(&[
            "-e",
            &format!(
                "sh -c ' nc localhost {}; read -p \"Press enter to exit...\"'",
                port
            ),
        ])
        .spawn()
        .unwrap_or_else(|_| panic!("Failed to open terminal window"));

    let (stream, _) = listener.accept().unwrap();
    let reader = BufReader::new(stream.try_clone().unwrap());
    let writer = stream;

    return (reader, writer);
}

 */

// //Esempio di utilizzo (il thread non è necessario, serve solo a simulare un client)
// fn main() {
//     let thread1 = thread::spawn(|| {
//         let (mut reader, mut writer) = setup_window();
//         let mut buffer = String::new();
//         loop {
//             //Read values
//             reader.read_line(&mut buffer).unwrap();
//             //print values
//             writeln!(writer, "{}", buffer).unwrap();
//             //in questo caso si chiama clear perchè riutilizziamo sempre lo stesso buffer, ma in genere non è necessario
//             buffer.clear();
//         }
//     });
//
//     thread1.join().unwrap();
// }