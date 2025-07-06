#![allow(warnings)]

mod repackager;


pub mod client2_ui;

use crate::client2_ui::Client2_UI;
use crate::repackager::Repackager;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use egui::debug_text::print;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io;
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, RwLock};
use wg_2024::network::*;
use wg_2024::packet::*;

pub struct Client2 {
    node_id: NodeId,
    discovered_drones: HashMap<NodeId, NodeType>,
    neighbor_senders: HashMap<NodeId, Sender<Packet>>,
    network_graph: HashMap<NodeId, HashSet<NodeId>>,
    servers: Arc<RwLock<HashMap<NodeId, String>>>,
    sent_packets: HashMap<u64, Packet>, // Store sent packets by session_id
    repackager: Repackager,
    receiver_channel: Receiver<Packet>,
    saved_files: HashSet<String>,
    other_client_ids: Arc<Mutex<Vec<NodeId>>>,
    files_names: Arc<Mutex<Vec<String>>>,
    cmd_rcv: Receiver<String>,
    msg_snd: Sender<String>,
    drone_rcv: Receiver<NodeId>,
    fragment_buffers: FileToRecieve,
    //reader: BufReader<TcpStream>,
    //writer: TcpStream,
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub struct FileToRecieve{
    file_name: String,
    data: Vec<u8>,
    session_id: u64,
}

impl Client2 {
    pub fn new(
        node_id: NodeId,
        neighbor_senders: HashMap<NodeId, Sender<Packet>>,
        receiver_channel: Receiver<Packet>,
        drone_rcv: Receiver<NodeId>
    ) -> (Self, Client2_UI) {
        let other_client_ids = Arc::new(Mutex::new(vec![]));
        let files_names = Arc::new(Mutex::new(vec![]));
        let servers = Arc::new(RwLock::new(HashMap::new()));

        let (cmd_snd, cmd_rcv) = unbounded::<String>();
        let (msg_snd, msg_rcv) = unbounded::<String>();
        let cl2_ui = Client2_UI::new(
            node_id,
            Arc::clone(&other_client_ids),
            Arc::clone(&servers),
            Arc::clone(&files_names),
            cmd_snd,
            msg_rcv,
        );
        //let (reader, writer) = setup_window();
        (
            Self {
                node_id,
                discovered_drones: HashMap::new(),
                drone_rcv,
                neighbor_senders,
                network_graph: HashMap::new(),
                servers,
                sent_packets: HashMap::new(), // Initialize the sent packets map
                repackager: Repackager::new(),
                receiver_channel,
                saved_files: HashSet::new(),
                other_client_ids,
                files_names,
                cmd_rcv,
                msg_snd,
                fragment_buffers: FileToRecieve{
                    file_name: "".to_string(),
                    data: Vec::new(),
                    session_id: 0,
                },
                //reader,
                //writer,
            },
            cl2_ui,
        )
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
            sender
                .send(Packet {
                    pack_type: PacketType::FloodRequest(request.clone()),
                    routing_header: self.create_source_routing_header(*drone_id),
                    session_id: self.generate_session_id(),
                })
                .expect("Failed to send FloodRequest");
        }
    }

    pub fn create_flood_response(&self, session_id: u64, flood_request: FloodRequest) -> Packet {
        let mut hops = vec![];
        for (node_id, node_type) in flood_request.path_trace.iter().rev() {
            hops.push(*node_id);
        }
        let response = FloodResponse {
            flood_id: flood_request.flood_id,
            path_trace: flood_request.path_trace,
        };
        let srh = SourceRoutingHeader::with_first_hop(hops);
        Packet::new_flood_response(srh, session_id, response)
    }

    pub fn handle_flood_request(&mut self, mut flood_request: FloodRequest, session_id: u64) {
        flood_request
            .path_trace
            .push((self.node_id, NodeType::Client));
        // Check if this flood request has already been processed
        if flood_request.initiator_id == self.node_id {
            return; // Skip because its the same to create the flood request
        }

        // Create and send the flood response back
        let response_packet = self.create_flood_response(session_id, flood_request.clone());
        self.forward_packet(response_packet.clone());
    }

    // Handle a received FloodResponse and build the network graph
    pub fn handle_flood_response(&mut self, response: FloodResponse) {
        //println!("CLIENT2: CLIENT{}: Received flood response: {:?}", self.node_id, response);
        let mut discovered_drones = &mut self.discovered_drones;
        let mut network_graph = &mut self.network_graph;

        // Collect node IDs that require handle_command
        let mut commands_to_send = Vec::new();

        for (node_id, node_type) in &response.path_trace {
            if node_id == &self.node_id {
                continue;
            }

            match node_type {
                NodeType::Drone => {
                    discovered_drones.entry(*node_id).or_insert(*node_type);
                }
                NodeType::Server => {
                    let mut servers = self.servers.write().expect("Failed to lock the servers map");
                    if !servers.contains_key(node_id) {
                        servers.insert(*node_id, "Unknown".to_string());
                        commands_to_send.push(*node_id); // Defer handle_command call
                    }
                }
                _ => {}
            }
        }

        // Update the network graph (adjacency list)
        for i in 0..response.path_trace.len() - 1 {
            let (node_a_id, _) = &response.path_trace[i];
            let (node_b_id, _) = &response.path_trace[i + 1];

            network_graph
                .entry(*node_a_id)
                .or_insert_with(HashSet::new)
                .insert(*node_b_id);
            network_graph
                .entry(*node_b_id)
                .or_insert_with(HashSet::new)
                .insert(*node_a_id);
        }

        //println!("CLIENT2: CLIENT{}: Discovered graph: {:?}", self.node_id, network_graph);

        // Now safely call handle_command for each server node_id
        for node_id in commands_to_send {
            //println!("CLIENT2: CLIENT{}: Sending server_type to server {}", self.node_id, node_id);
            self.handle_command(format!("server_type?->{}", node_id));
        }
    }

    // Send a message to a server through drones
    pub fn send_message(&mut self, server_id: NodeId, message: &str, file_path: Option<&str>) {
        // Create fragments using the Repackager
        let fragments = Repackager::create_fragments(message, file_path)
            .expect("CLIENT2: Failed to create fragments");

        let session_id = self.generate_session_id();
        let path = Self::bfs_shortest_path(self.network_graph.clone(), self.node_id, server_id);

        for fragment in fragments {
            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header: SourceRoutingHeader::with_first_hop(path.clone().expect("No path found")),
                session_id,
            };

            // Store the sent packet
            self.sent_packets.insert(session_id, packet.clone());
            self.forward_packet(packet);
        }
    }

    //Handle of commands
    pub fn handle_command(&mut self, command: String) -> String {
        let (comm, dest) = command
            .split_once("->")
            .expect("Command was formated wrong.");
        let server_id = dest.parse().expect("Failed to parse server_id");
        match comm {
            cmd if cmd == "server_type?"
                || cmd == "files_list?"
                || cmd == "client_list?" =>
            {
                //server_type?
                //files_list?
                //client_list?
                self.send_message(server_id, cmd, None);
                return "CLIENT2: OK".to_string();
            }
            cmd if cmd.starts_with("file?(") && cmd.ends_with(")") => {
                //file?(file_id)
                let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.fragment_buffers = FileToRecieve{
                    file_name: text.expect("Couldnt get file name").to_string(),
                    data: Vec::new(),
                    session_id: 0,
                };
                self.send_message(server_id, cmd, None);
                return "CLIENT2: OK".to_string();
            }
            cmd if cmd.starts_with("media?(") && cmd.ends_with(")") => {
                //media?(media_id)
                let text = cmd.strip_prefix("media?(").and_then(|s| s.strip_suffix(")"));
                self.fragment_buffers = FileToRecieve{
                    file_name: text.expect("Couldnt get file name").to_string(),
                    data: Vec::new(),
                    session_id: 0,
                };
                self.send_message(server_id, cmd, None);
                return "CLIENT2: OK".to_string();
            }
            cmd if cmd.starts_with("message_for?(") && cmd.ends_with(")") => {
                //message_for?(client_id, message)
                //println!("CLIENT2: CLIENT{}: {})", self.node_id, cmd);
                let text = cmd
                    .strip_prefix("message_for?(")
                    .and_then(|s| s.strip_suffix(")"));
                self.send_message(server_id, cmd, None);
                return "CLIENT2: OK".to_string();
            }
            _ => {
                "Wrong command, type 'commands' to see the full list of the commands.";
                return "CLIENT2: OK".to_string();
            }
        }
    }

    // Forward packet to the first drone in the routing path
    fn forward_packet(&self, packet: Packet) {
        if let Some(first_hop) = packet.routing_header.hops.get(1) {
            if let Some(sender) = self.neighbor_senders.get(first_hop) {
                //println!("CLIENT2: CLIENT{}: forwarding to Drone {}, packet: {:?}", self.node_id, first_hop, packet);
                //println!("CLIENT2: CLIENT{}: SENDING PACKET {}", self.node_id, packet);
                sender.send(packet).expect("Failed to send packet");
            } else {
                // println!(
                //     "CLIENT2: CLIENT{}: not found in neighbors for packet {}",
                //     first_hop, packet
                // );
            }
        } else {
            // println!(
            //     "CLIENT2: CLIENT{}: No valid routing path found for packet",
            //     self.node_id
            // );
        }
    }

    //Function that handles messages from the server
    pub fn handle_messages(&mut self, message: String, session_id: u64, sender: NodeId) {
        //println!("CLIENT2: CLIENT{}: Received message: {}", self.node_id, message);
        self.msg_snd
            .send(message.clone())
            .expect("Failed to send message");

        if let Some(svtype) = message.strip_prefix("server_type!(").and_then(|s| s.strip_suffix(")")) {
            let mut servers = self.servers.write().expect("Failed to lock servers map");
            servers.entry(sender).and_modify(|existing| {
                if *existing == "Unknown" {
                    *existing = svtype.to_string();
                }
            }).or_insert(svtype.to_string());
    
        if let sender = self.msg_snd.clone() {
                sender.send("REFRESH_UI".to_string()).expect("Failed to send message");
            }
        }
        
        match message {
            msg if msg.starts_with("files_list!([") && msg.ends_with("])") => {
                //files_list!(list_of_file_ids)
                let files = msg
                    .strip_prefix("files_list!([")
                    .and_then(|s| s.strip_suffix("])"));
                let new_files = files
                    .expect("Failed to parse files list")
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .collect::<Vec<String>>();

                if let Ok(mut files_lock) = self.files_names.lock() {
                    *files_lock = new_files;
                }
            }
            msg if msg.starts_with("client_list!([") && msg.ends_with("])") => {
                //client_list!([list_of_client_ids])
                let txt = msg
                    .strip_prefix("client_list!([")
                    .and_then(|s| s.strip_suffix("])"));
                if let Some(txt) = txt {
                    let values = txt
                        .split(", ")
                        .filter_map(|s| s.parse::<u8>().ok())
                        .filter(|id| *id != self.node_id) // exclude your own ID// convert to u8 (NodeId)
                        .collect::<Vec<NodeId>>();
                    for id in values {
                        if !self.other_client_ids.lock().expect("Failed to lock client ids").contains(&id) {
                            self.other_client_ids.lock().expect("Failed to lock client ids").push(id);
                        }
                    }
                }
            }
            msg if msg.starts_with("message_from!(") && msg.ends_with(")") => {
                //message_from!(client_id, message)
                let txt = msg
                    .strip_prefix("message_from!(")
                    .and_then(|s| s.strip_suffix(")"));
            }

            _ => {}
        }
    }

    // Create a source routing header from client to server through discovered drones
    fn create_source_routing_header(&self, destination: NodeId) -> SourceRoutingHeader {
        let discovered_drones = self.discovered_drones.clone();
        let mut hops = vec![self.node_id];

        if let Some((&drone_id, _)) = discovered_drones.iter().next() {
            hops.push(drone_id);
        }
        hops.push(destination);

        SourceRoutingHeader { hop_index: 1, hops }
    }

    fn handle_crash(&mut self, drone_id: NodeId) {
        self.neighbor_senders.remove(&drone_id);
        self.network_graph.remove(&drone_id);

        // Only remove connections to this drone
        for neighbors in self.network_graph.values_mut() {
            neighbors.remove(&drone_id);
        }

        // Only rediscover if we have no connections left
        if self.neighbor_senders.is_empty() {
            self.discover_network();
        }
    }

    // Handle received packet (Ack, Nack, etc.)
    pub fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::MsgFragment(ref fragment) => {
                self.handle_msg_fragment(fragment.clone(), packet.clone());

                // Get the source node from the packet's routing header
                let source = packet.routing_header.source();

                if let Some(source_id) = source {
                    // Try to find a path back to the source
                    if let Some(path) = Self::bfs_shortest_path(
                        self.network_graph.clone(),
                        self.node_id,
                        source_id
                    ) {
                        self.forward_packet(Packet::new_ack(
                            SourceRoutingHeader::with_first_hop(path),
                            packet.session_id,
                            fragment.fragment_index,
                        ));
                    } else {
                        // If no path found, trigger network rediscovery
                        //println!("CLIENT2: No path found to node {}, triggering rediscovery", source_id);
                        self.discovered_drones = HashMap::new();
                        self.network_graph = HashMap::new();
                        self.discover_network();
                    }
                }
            }
            PacketType::FloodRequest(request) => {
                self.handle_flood_request(request, packet.session_id)
            }
            PacketType::FloodResponse(response) => {
                self.handle_flood_response(response);
            }
            PacketType::Ack(ack) => {}
            PacketType::Nack(nack) => {
                //Check again all nodes, servers and connections
                self.discovered_drones = HashMap::new();
                self.network_graph = HashMap::new();
                self.discover_network();
                // Resend the original packet
                if let Some(original_packet) = self.sent_packets.get(&packet.session_id) {
                    // println!(
                    //     "CLIENT2: CLIENT{}: Resending packet for session ID {}",
                    //     self.node_id, packet.session_id
                    // );
                    self.forward_packet(original_packet.clone());
                } else {
                    // println!(
                    //     "CLIENT2: CLIENT{}: No original packet found for session ID {}",
                    //     self.node_id, packet.session_id
                    // );
                }
            }
            _ => {
                // println!(
                //     "CLIENT2: CLIENT{}: received unknown packet type",
                //     self.node_id
                // );
            }
        }
    }

    pub fn handle_msg_fragment(&mut self, fragment: Fragment, packet: Packet) {
        if fragment.fragment_index == 0 && fragment.total_n_fragments > 1 {
            self.fragment_buffers.session_id = packet.session_id;
        }
        if fragment.fragment_index < fragment.total_n_fragments && self.fragment_buffers.session_id == packet.session_id {
            // Calculate position and copy data into the struct's data vector
            let start = (fragment.fragment_index * 128) as usize;
            let end = start + fragment.length as usize;

            // Ensure the vector is large enough
            if self.fragment_buffers.data.len() < end {
                self.fragment_buffers.data.resize(end, 0);
            }

            // Copy the fragment data into the correct position
            self.fragment_buffers.data[start..end].copy_from_slice(&fragment.data[..fragment.length as usize]);

            if fragment.fragment_index != fragment.total_n_fragments - 1 {
                return;
            } else {
                let output_path = format!("C:\\Temp\\Client2\\{}", self.fragment_buffers.file_name);

                // Call assemble_file with the correct parameters
                match Repackager::assemble_file(self.fragment_buffers.data.clone(), &output_path) {
                    Ok(msg) => self.msg_snd.send("File saved successfuly to C:\\Temp\\Client2".to_string()).expect("Failed to send message"),
                    Err(e) => self.msg_snd.send(format!("Error assembling file: {}", e)).expect("Failed to send message"),
                }
                self.fragment_buffers = FileToRecieve{
                    file_name: "".to_string(),
                    data: Vec::new(),
                    session_id: 0,
                };
                return;
            }
        }


        let session_id = self.generate_session_id();
        let src_id = self.node_id as u64;

        match self
            .repackager
            .process_fragment(session_id, src_id, fragment)
        {
            Ok(Some(reassembled_message)) => {
                // Process the complete message
                let msg = Repackager::assemble_string(reassembled_message);
                if let Ok(message) = msg.clone() {
                    // Send to UI for logging only when message is completely assembled
                    self.msg_snd
                        .send(message.clone())
                        .expect("Failed to send message");
                    self.handle_messages(
                        message,
                        packet.session_id,
                        *packet.routing_header.hops.first().expect("No hops found"),
                    );
                }
            }
            Ok(None) => {
                // Still waiting for more fragments, do nothing
            }
            Err(error_code) => {
                // Log error in assembling fragments
                self.msg_snd
                    .send(format!("Error processing fragment: {}", error_code))
                    .expect("Failed to send message");
            }
        }
    }

    // BFS function to find the shortest path in the network graph
    fn bfs_shortest_path(
        graph: HashMap<NodeId, HashSet<NodeId>>,
        start: NodeId,
        goal: NodeId,
    ) -> Option<Vec<NodeId>> {
        let mut visited: HashSet<NodeId> = HashSet::new(); // Track visited nodes
        let mut queue: VecDeque<Vec<NodeId>> = VecDeque::new(); // Queue to store paths
        queue.push_back(vec![start]); // Initialize queue with the starting node
        visited.insert(start);

        while let Some(path) = queue.pop_front() {
            let node = *path.last().expect("No nodes in path");

            if node == goal {
                return Some(path); // Goal found, return the path
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

        None // No path found
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
        let receiver_channel = self.receiver_channel.clone();

        //Packet handle part
        loop {
            select_biased! {
                // Handle packets in the meantime
                    recv(receiver_channel) -> packet =>{
                        match packet{
                                Ok(packet) => {
                                        self.handle_packet(packet); //, &msg_snd
                                },
                                Err(e) => ()//println!("Err2: {e}")
                        }
                    }
                    recv(self.cmd_rcv) -> cmd => {
                        match cmd {
                            Ok(cmd) => {
                                match self.handle_command(cmd.clone()).as_str() {
                                    "CLIENT2: OK" => (),
                                    e => () //println!("Err2: {e}")
                                }
                            }
                            Err(e) =>  ()//println!("Err3: {e}") // Normal that prints at the end, the UI is closed
                        }
                    }
                    recv(self.drone_rcv) -> id => {
                        match id{
                            Ok(id) =>{
                                self.handle_crash(id);
                            }
                            Err(_) => ()
                        }
                    }
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, RwLock};

    // Helper function to create a test client
    fn create_test_client(node_id: NodeId) -> (Client2, Client2_UI, Receiver<Packet>, Sender<NodeId>) {
        let (packet_snd, packet_rcv) = unbounded();
        let (drone_snd, drone_rcv) = unbounded();
        let neighbor_senders = HashMap::new();

        let (client, ui) = Client2::new(
            node_id,
            neighbor_senders,
            packet_rcv.clone(),
            drone_rcv,
        );

        (client, ui, packet_rcv, drone_snd)
    }

    #[test]
    fn test_client_creation() {
        let (client, _ui, _packet_rcv, _drone_snd) = create_test_client(1);
        assert_eq!(client.node_id, 1);
        assert!(client.discovered_drones.is_empty());
        assert!(client.neighbor_senders.is_empty());
        assert!(client.network_graph.is_empty());
    }

    #[test]
    fn test_handle_flood_request() {
        let (mut client, _ui, _packet_rcv, _drone_snd) = create_test_client(1);
        let flood_request = FloodRequest {
            flood_id: 123,
            initiator_id: 2,
            path_trace: vec![(2, NodeType::Client)],
        };

        client.handle_flood_request(flood_request.clone(), 456);
        // Verify the path trace was updated
        // This is tricky since we don't have access to the modified flood_request
        // You might need to modify the function to return the modified request
    }

    #[test]
    fn test_bfs_shortest_path() {
        let mut graph = HashMap::new();
        graph.insert(1, HashSet::from([2]));
        graph.insert(2, HashSet::from([1, 3]));
        graph.insert(3, HashSet::from([2, 4]));
        graph.insert(4, HashSet::from([3]));

        let path = Client2::bfs_shortest_path(graph, 1, 4);
        assert_eq!(path, Some(vec![1, 2, 3, 4]));

        // Test no path exists
        let graph = HashMap::from([(1, HashSet::from([2])), (3, HashSet::from([4]))]);
        let path = Client2::bfs_shortest_path(graph, 1, 4);
        assert_eq!(path, None);
    }

    #[test]
    fn test_create_source_routing_header() {
        let (mut client, _ui, _packet_rcv, _drone_snd) = create_test_client(1);
        client.discovered_drones.insert(3, NodeType::Drone);

        let header = client.create_source_routing_header(4);
        assert_eq!(header.hops, vec![1, 3, 4]);
        assert_eq!(header.hop_index, 1);
    }
}