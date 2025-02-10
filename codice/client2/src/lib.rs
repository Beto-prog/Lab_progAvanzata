mod repackager;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Write};
use wg_2024::packet::*;
use wg_2024::network::*;
use crossbeam_channel::{Sender, Receiver, select_biased};
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use crate::repackager::Repackager;

pub struct Client2 {
    node_id: NodeId,
    discovered_drones: HashMap<NodeId, NodeType>,   //Drones that have been found
    neighbor_senders: HashMap<NodeId, Sender<Packet>>,  //Direct drones connected to the client
    network_graph: HashMap<NodeId, HashSet<NodeId>>,
    servers: HashMap<NodeId, String>,   //List of the servers
    sent_packets: HashMap<u64, Packet>, // Store sent packets by session_id
    repackager: Repackager,     //This is where the fragmentation is done
    receiver_channel: Receiver<Packet>,
    saved_files: HashSet<String>,   //Path of saved files
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Client2 {
    pub fn new(node_id: NodeId, neighbor_senders: HashMap<NodeId, Sender<Packet>>, receiver_channel: Receiver<Packet>) -> Self {
        let (reader, writer) = setup_window();
        Self {
            node_id,
            discovered_drones: HashMap::new(),
            neighbor_senders,
            network_graph: HashMap::new(),
            servers: HashMap::new(),
            sent_packets: HashMap::new(), // Initialize the sent packets map
            repackager: Repackager::new(),
            receiver_channel,
            saved_files: HashSet::new(),
            reader,
            writer,
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

        // Cycle neighbors and send the flood request
        for (drone_id, sender) in &self.neighbor_senders {
            sender.send(Packet {
                pack_type: PacketType::FloodRequest(request.clone()),
                routing_header: self.create_source_routing_header(*drone_id),
                session_id: self.generate_session_id(),
            }).expect("Sending FloodRequest failed");
        }
    }

    // Function that creates the flood response
    pub fn create_flood_response(&self, session_id: u64, flood_request: FloodRequest) -> Packet {
        let mut hops = vec![];
        for &e in flood_request.path_trace.iter().rev() {
            hops.push(e.0);
        }
        hops.reverse();

        let response = FloodResponse {
            flood_id: flood_request.flood_id,
            path_trace: flood_request.path_trace,
        };

        let srh = SourceRoutingHeader::with_first_hop(hops);
        Packet::new_flood_response(srh, session_id, response)
    }

    // Handles the floods requests
    pub fn handle_flood_request(&mut self, mut flood_request: FloodRequest, session_id: u64) {
        flood_request.path_trace.push((self.node_id, NodeType::Client));
        // Check if this flood request has already been processed
        if flood_request.initiator_id == self.node_id {
            return;  // Skip because its the same to create the flood request
        }

        // Create and send the flood response back
        let response_packet = self.create_flood_response(session_id, flood_request);
        self.forward_packet(response_packet);
    }

    // Handle a received FloodResponse and build the network graph
    pub fn handle_flood_response(&mut self, response: FloodResponse) {
        let discovered_drones = &mut self.discovered_drones;
        let network_graph = &mut self.network_graph;

        // Update the discovered drones list
        for (node_id, node_type) in &response.path_trace {
            if node_type == &NodeType::Drone {
                discovered_drones.entry(*node_id).or_insert(*node_type);
            }
            if node_type == &NodeType::Server {
                // Unknown is default value before asking the server with server_type?
                self.servers.insert(self.node_id, "Unknown".to_string());
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

        // Create each packet with the fragments
        for fragment in fragments {
            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header: SourceRoutingHeader::with_first_hop(path.clone().expect("Failed to get first hop")),
                session_id,
            };

            // Store the sent packet
            self.sent_packets.insert(session_id, packet.clone());
            self.forward_packet(packet);
        }
    }

    // Function that verifies each command and sends it to the server
    pub fn handle_command(&mut self, command: &str) {
        if command == "commands" {
            writeln!(&mut self.writer, "
server_type?->NodeId
server_list
client_list?->NodeId
files_list?->NodeId
registration_to_chat->NodeId
file?(file_id)->NodeId
media?(media_id)->NodeId
message_for?(client_id, message)->NodeId"
            ).expect("Error writing to writer");
            return;
        } else if command == "server_list" {
            for (server_id, server_type) in &self.servers {
                writeln!(&mut self.writer, "{}, {}", server_id, server_type).expect("Error writing to writer");
            }
            return;
        }
        let (comm, dest) = command.split_once("->").expect("Command was formated wrong.");
        let server_id = dest.parse().expect("Command did not parse as a server.");
        match comm {
            cmd if cmd == "server_type?" || cmd == "files_list?" || cmd == "registration_to_chat" || cmd == "client_list?" => {
                //server_type?
                //files_list?
                //registration_to_chat
                //client_list?
                self.send_message(server_id, cmd, None);
            }
            cmd if cmd.starts_with("file?(") && cmd.ends_with(")") => {
                //file?(file_id)
                //let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.send_message(server_id, cmd, None);
            }
            cmd if cmd.starts_with("media?(") && cmd.ends_with(")") => {
                //media?(media_id)
                //let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.send_message(server_id, cmd, None);
            }
            cmd if cmd.starts_with("message_for?(") && cmd.ends_with(")") => {
                //message_for?(client_id, message)
                //let text = cmd.strip_prefix("message_for?(").and_then(|s| s.strip_suffix(")"));
                //let mut val = text.expect("Error in getting the value").split_once(", ");
                self.send_message(server_id, cmd, None);
            }
            _ => { "Wrong command, type 'commands' to see the full list of the commands."; }
        }
    }

    // Forward packet to the first drone in the routing path
    fn forward_packet(&mut self, packet: Packet) {
        if let Some(first_hop) = packet.routing_header.hops.get(1) {
            if let Some(sender) = self.neighbor_senders.get(first_hop) {
                sender.send(packet).expect("Forwarding packet failed");
            } else {
                writeln!(&mut self.writer, "Not found in neighbors for packet {}", packet).expect("Error writing to writer");
            }
        } else {
            writeln!(&mut self.writer, "No valid routing path found for packet").expect("Error writing to writer");
        }
    }

    //Function that handles messages from the server
    pub fn handle_messages(&mut self, message: String, sender: NodeId) {
        match message {
            msg if msg.starts_with("server_type!(") && msg.ends_with(")") => {
                //server_type!(type)
                let svtype = msg.strip_prefix("server_type!(").and_then(|s| s.strip_suffix(")"));
                self.servers.entry(sender).or_insert_with(|| svtype.expect("Server type must be provided").to_string());
                writeln!(&mut self.writer, "{:?}", self.servers).expect("Error writing to writer");
            }
            msg if msg.starts_with("files_list!(") && msg.ends_with(")") => {
                //files_list!(list_of_file_ids)
                let files = msg.strip_prefix("files_list!(").and_then(|s| s.strip_suffix(")"));
                let ff = files.expect("No file inserted").split(",").map(|s| s.to_string()).collect::<Vec<String>>();
                writeln!(&mut self.writer, "{:?}", ff).expect("Error writing to writer");
            }
            msg if msg.starts_with("file!(") && msg.ends_with(")") => {
                //file!(file_size, file)
                let txt = msg.strip_prefix("server_type!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.expect("Couldn't parse the string").split_once(", ");
                self.saved_files.insert(values.expect("Couldn't insert the values").1.to_string());
            }
            msg if msg.starts_with("media!(") && msg.ends_with(")") => {
                //media!(media)
                let txt = msg.strip_prefix("media!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.expect("Couldn't get file path").split_once(", ");
            }
            msg if msg == "error_requested_not_found!" || msg == "error_unsupported_request!" || msg == "error_wrong_client_id!" => {
                //error_requested_not_found!
                //error_unsupported_request!
                //error_wrong_client_id!
                writeln!(&mut self.writer, "Error: {}", msg).expect("Error writing to writer");
            }
            msg if msg.starts_with("client_list!(") && msg.ends_with(")") => {
                //client_list!(list_of_client_ids)
                let txt = msg.strip_prefix("client_list!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.expect("Couldn't get the client list").split(", ").map(|s| s.to_string()).collect::<Vec<String>>();
                writeln!(&mut self.writer, "Clients: {:?}", values).expect("Error writing to writer");
            }
            msg if msg.starts_with("message_from!(") && msg.ends_with(")") => {
                //message_from!(client_id, message)
                let txt = msg.strip_prefix("message_from!(").and_then(|s| s.strip_suffix(")"));
                let values = txt.expect("Couldn't get the client and message").split_once(", ");
                writeln!(&mut self.writer, "Client {} sent {}", values.expect("Couldn't get the message").0, values.expect("Couldn't get the other client id").1).expect("Error writing to writer");
            }
            _ => { writeln!(&mut self.writer, "Wrong message received from the server").expect("Error writing to writer"); }
        }
    }

    // Create a source routing header from client to server through discovered drones
    fn create_source_routing_header(&self,
                                    destination: NodeId) -> SourceRoutingHeader {
        let discovered_drones = self.discovered_drones.clone();
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
                self.forward_packet(Packet::new_ack(SourceRoutingHeader::with_first_hop(Self::bfs_shortest_path(self.network_graph.clone(), self.node_id, packet.routing_header.source().expect("Error in finding source of routing header")).expect("Error in getting the node id.")), packet.session_id, fragment.fragment_index))
            }
            PacketType::FloodRequest(request) => self.handle_flood_request(request, packet.session_id),
            PacketType::FloodResponse(response) => {
                self.handle_flood_response(response);
            }
            PacketType::Ack(ack) => { writeln!(self.writer, "Ack received {:?}", ack).expect("Error writing to writer"); }
            PacketType::Nack(nack) => {
                writeln!(self.writer, "Nack received {:?}", nack).expect("Error writing to writer");
                //Check again all nodes, servers and connections
                self.servers = HashMap::new();
                self.discovered_drones = HashMap::new();
                self.network_graph = HashMap::new();
                self.discover_network();
                // Resend the original packet
                if let Some(original_packet) = self.sent_packets.get(&packet.session_id) {
                    writeln!(self.writer, "Resending packet for session ID {}", packet.session_id).expect("Error writing to writer");
                    self.forward_packet(original_packet.clone());
                } else {
                    writeln!(self.writer, "No original packet found for session ID {}", packet.session_id).expect("Error writing to writer");
                }
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
                writeln!(self.writer, "The message is {:?}", msg).expect("Error writing to writer");
            }
            Ok(None) => {
                writeln!(self.writer, "Not all fragments received yet for message ID {}", packet.session_id).expect("Error writing to writer");
            }
            Err(error_code) => {
                writeln!(self.writer, "Error processing fragment: {}", error_code).expect("Error writing to writer");
            }
        }
    }

    // BFS function to find the shortest path in the network graph
    fn bfs_shortest_path(graph: HashMap<NodeId, HashSet<NodeId>>, start: NodeId, goal: NodeId) -> Option<Vec<NodeId>> {
        let mut visited: HashSet<NodeId> = HashSet::new();  // Track visited nodes
        let mut queue: VecDeque<Vec<NodeId>> = VecDeque::new();  // Queue to store paths

        queue.push_back(vec![start]);  // Initialize queue with the starting node
        visited.insert(start);

        while let Some(path) = queue.pop_front() {
            let node = *path.last().expect("Couldn't get the last form the path");  // Get the last node in the current path

            if node == goal {
                return Some(path);  // Goal found, return the path
            }

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
        let (mut reader, mut writer) = setup_window();
        let mut buffer = String::new();
        writeln!(writer, "Client2 is running. Type your commands:").expect("Error writing output");
        self.discover_network();
        loop {
            select_biased! {
                recv(self.receiver_channel) -> packet => {
                    match packet {
                        Ok(packet) => {
                            self.handle_packet(packet);
                        },
                        Err(e) => {
                            writeln!(writer, "Server {} error processing received packet: {}", self.node_id, e).expect("Error writing output");
                        }
                    }
                }

            }
            if !self.servers.is_empty() {
                reader.read_line(&mut buffer).expect("Error reading input");
                self.handle_command(buffer.clone().as_str());
            }
        }
    }
}


fn setup_window() -> (BufReader<TcpStream>, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Error setting up listener");
    let port = listener.local_addr().expect("Error getting socket address").port();

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
        .expect("Error spawning xterm");

    let (stream, _) = listener.accept().expect("Error accepting connection");
    let reader = BufReader::new(stream.try_clone().expect("Error cloning stream"));
    let writer = stream;

    return (reader, writer);
}