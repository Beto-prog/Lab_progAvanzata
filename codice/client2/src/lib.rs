#![allow(warnings)]

mod repackager;

mod logger;

pub mod client2_ui;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use wg_2024::packet::*;
use wg_2024::network::*;
use crossbeam_channel::{Sender, Receiver, select_biased, unbounded};
use std::io::{BufRead, BufReader};
use crate::client2_ui::Client2_UI;
use crate::repackager::Repackager;
use crate::logger::logger::{init_logger, write_log};


pub struct Client2 {
    node_id: NodeId,
    discovered_drones: HashMap<NodeId, NodeType>,
    neighbor_senders: HashMap<NodeId, Sender<Packet>>,
    network_graph: HashMap<NodeId, HashSet<NodeId>>,
    servers: Arc<Mutex<HashMap<NodeId,String>>>,
    sent_packets: HashMap<u64, Packet>, // Store sent packets by session_id
    repackager: Repackager,
    receiver_channel: Receiver<Packet>,
    saved_files: HashSet<String>,
    ui_snd: Option<Sender<Client2_UI>>,
    other_client_ids: Arc<Mutex<Vec<NodeId>>>,
    files_names: Arc<Mutex<Vec<String>>>,
    //reader: BufReader<TcpStream>,
    //writer: TcpStream,
}

impl Client2 {
    pub fn new(node_id: NodeId, neighbor_senders: HashMap<NodeId, Sender<Packet>>, receiver_channel: Receiver<Packet>,
               ui_snd: Option<Sender<Client2_UI>>) -> Self {
        //let (reader, writer) = setup_window();
        Self {
            node_id,
            discovered_drones: HashMap::new(),
            neighbor_senders,
            network_graph: HashMap::new(),
            servers: Arc::new(Mutex::new(HashMap::new())),
            sent_packets: HashMap::new(), // Initialize the sent packets map
            repackager: Repackager::new(),
            receiver_channel,
            saved_files: HashSet::new(),
            ui_snd: Some(ui_snd.expect("Failed to get value")),
            other_client_ids: Arc::new(Mutex::new(vec![])),
            files_names: Arc::new(Mutex::new(vec![])),
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
            sender.send(Packet {
                pack_type: PacketType::FloodRequest(request.clone()),
                routing_header: self.create_source_routing_header(*drone_id),
                session_id: self.generate_session_id(),
            }).unwrap();
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
                    let mut servers = self.servers.lock().expect("Failed to lock the servers map");
                    if !servers.contains_key(node_id) {
                        servers.insert(*node_id, "Unknown".to_string());
                        commands_to_send.push(*node_id);
                    }
                }
                // Uncomment and adjust if you want to handle clients here:
                // NodeType::Client => {
                //     let mut other_client_ids = self.other_client_ids.lock().unwrap();
                //     if !other_client_ids.contains(node_id) {
                //         other_client_ids.push(*node_id);
                //     }
                // }
                _ => {}
            }
        }

        // Update the network graph (adjacency list)
        for i in 0..response.path_trace.len() - 1 {
            let (node_a_id, _) = &response.path_trace[i];
            let (node_b_id, _) = &response.path_trace[i + 1];

            network_graph
                .entry(*node_a_id)
                .or_insert_with(std::collections::HashSet::new)
                .insert(*node_b_id);
            network_graph
                .entry(*node_b_id)
                .or_insert_with(std::collections::HashSet::new)
                .insert(*node_a_id);
        }

        for node_id in commands_to_send {
            self.handle_command(format!("server_type?->{}", node_id));
        }

        // println!("CLIENT2: CLIENT{}: Discovered graph: {:?}", self.node_id, network_graph);
    }

    // Send a message to a server through drones
    pub fn send_message(&mut self, server_id: NodeId, message: &str, file_path: Option<&str>) {
        println!("sending message {}", message);
        // Create fragments using the Repackager
        let fragments = Repackager::create_fragments(message, file_path).expect("CLIENT2: Failed to create fragments");

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
    pub fn handle_command(&mut self, command: String) -> String {
        if command == "commands" {
            println!("server_type?->NodeId
server_list
files_list?->NodeId
registration_to_chat->NodeId
file?(file_id)->NodeId
media?(media_id)->NodeId
message_for?(client_id, message)->NodeId");
            return "CLIENT1: OK".to_string()
        } else if command == "server_list" {
            let servers = self.servers.lock().unwrap();
            for(server_id, server_type) in servers.iter() {
                println!("{}, {}", server_id, server_type);
            }
            return "CLIENT1: OK".to_string()
        }
        let (comm, dest) = command.split_once("->").expect("Command was formated wrong.");
        let server_id = dest.parse().unwrap();
        match comm {
            cmd if cmd == "server_type?" || cmd == "files_list?" || cmd == "registration_to_chat" || cmd == "client_list?" => {
                //server_type?
                //files_list?
                //registration_to_chat
                //client_list?
                self.send_message(server_id, cmd, None);
                return "CLIENT1: OK".to_string()
            }
            cmd if cmd.starts_with("file?(") && cmd.ends_with(")") => {
                //file?(file_id)
                let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.send_message(server_id, cmd, None);
                return "CLIENT1: OK".to_string()
            }
            cmd if cmd.starts_with("media?(") && cmd.ends_with(")") => {
                //media?(media_id)
                let text = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"));
                self.send_message(server_id, cmd, None);
                return "CLIENT1: OK".to_string()
            }
            cmd if cmd.starts_with("message_for?(") && cmd.ends_with(")") => {
                //message_for?(client_id, message)
                let text = cmd.strip_prefix("message_for?(").and_then(|s| s.strip_suffix(")"));
                let mut val = text.unwrap().split_once(", ");
                self.send_message(server_id, cmd, None);
                return "CLIENT1: OK".to_string()
            }
            _ => { "Wrong command, type 'commands' to see the full list of the commands.";
                return "CLIENT1: OK".to_string()}
        }
    }

    // Forward packet to the first drone in the routing path
    fn forward_packet(&self, packet: Packet) {
        if let Some(first_hop) = packet.routing_header.hops.get(1) {
            if let Some(sender) = self.neighbor_senders.get(first_hop) {
                //println!("CLIENT2: CLIENT{}: forwarding to Drone {}, packet: {:?}", self.node_id, first_hop, packet);
                sender.send(packet).unwrap();
            } else {
                println!("CLIENT2: CLIENT{}: not found in neighbors for packet {}", first_hop, packet);
            }
        } else {
            println!("CLIENT2: CLIENT{}: No valid routing path found for packet", self.node_id);
        }
    }

    //Function that handles messages from the server
    pub fn handle_messages(&mut self, message: String, session_id: u64, sender: NodeId) {
        match message {
            msg if msg.starts_with("server_type!(") && msg.ends_with(")") => {
                //server_type!(type)
                let svtype = msg.strip_prefix("server_type!(").and_then(|s| s.strip_suffix(")"));
                let mut servers = self.servers
                    .lock()
                    .expect("Failed to lock the servers map");

                let entry = servers.entry(sender).or_insert_with(|| svtype.as_ref().unwrap().to_string());

                if entry == "Unknown" {
                    *entry = svtype.unwrap().to_string();
                }
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
            msg if msg.starts_with("client_list!([") && msg.ends_with("])") => {
                //client_list!([list_of_client_ids])
                let txt = msg.strip_prefix("client_list!([").and_then(|s| s.strip_suffix("])"));
                if let Some(txt) = txt {
                    let values = txt
                        .split(", ")
                        .filter_map(|s| s.parse::<u8>().ok())
                        .filter(|id| *id != self.node_id) // exclude your own ID// convert to u8 (NodeId)
                        .collect::<Vec<NodeId>>();
                    for id in values {
                        if !self.other_client_ids.lock().unwrap().contains(&id) {
                            self.other_client_ids.lock().unwrap().push(id);
                        }
                    }
                }
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
    pub fn handle_packet(&mut self, packet: Packet, msg_snd: &Sender<String>) {
        match packet.pack_type {
            PacketType::MsgFragment(ref fragment) => {
                self.handle_msg_fragment(fragment.clone(), packet.clone(), msg_snd);
                self.forward_packet(Packet::new_ack(SourceRoutingHeader::with_first_hop(Self::bfs_shortest_path(self.network_graph.clone(), self.node_id, packet.routing_header.source().unwrap()).unwrap()), packet.session_id, fragment.fragment_index))
            }
            PacketType::FloodRequest(request) => self.handle_flood_request(request, packet.session_id),
            PacketType::FloodResponse(response) => {
                self.handle_flood_response(response);
            }
            PacketType::Ack(ack) => {}
            PacketType::Nack(nack) => {
                //Check again all nodes, servers and connections
                self.servers = Arc::new(Mutex::new(HashMap::new()));
                self.discovered_drones = HashMap::new();
                self.network_graph = HashMap::new();
                self.discover_network();
                // Resend the original packet
                if let Some(original_packet) = self.sent_packets.get(&packet.session_id) {
                    println!("CLIENT2: CLIENT{}: Resending packet for session ID {}", self.node_id, packet.session_id);
                    self.forward_packet(original_packet.clone());
                } else {
                    println!("CLIENT2: CLIENT{}: No original packet found for session ID {}", self.node_id, packet.session_id);
                }
            }
            _ => {
                println!("CLIENT2: CLIENT{}: received unknown packet type", self.node_id);
            }
        }
    }

    pub fn handle_msg_fragment(&mut self, fragment: Fragment, packet: Packet, msg_snd: &Sender<String>) {
        // Call process_fragment to handle the incoming fragment
        let session_id = self.generate_session_id(); // Assuming you have access to session_id
        let src_id = self.node_id as u64; // Assuming src_id is the node_id of the client

        match self.repackager.process_fragment(session_id, src_id, fragment) {
            Ok(Some(reassembled_message)) => {
                // Process the complete message
                let msg = Repackager::assemble_string(reassembled_message);
                println!("CLIENT2: CLIENT{}: Converted fragments into message: {:?}", self.node_id, msg);
                msg_snd.send(msg.clone().unwrap().to_string()).expect("Failed to send message");
                self.handle_messages(msg.unwrap().to_string(), packet.session_id, *packet.routing_header.hops.first().unwrap());
            }
            Ok(None) => {
                println!("CLIENT2: CLIENT{}: Not all fragments received yet for message ID {}", self.node_id, packet.session_id);
            }
            Err(error_code) => {
                println!("CLIENT2: CLIENT{}: Error processing fragment: {}", self.node_id, error_code);
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
            let node = *path.last().unwrap();  // Get the last node in the current path

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

//     pub fn run(&mut self) {
//         self.discover_network();
//         let mut input = "files_list?->6";
//         let mut executed = false;
//         loop {
//             select_biased! {
//                 recv(self.receiver_channel) -> packet => {
//                     match packet {
//                         Ok(packet) => {
//                             //println!("CLIENT2: CLIENT{}: Received packet: {:?}", self.node_id, packet);
//                             self.handle_packet(packet);
//                         },
//                         Err(e) => {
//                             eprintln!("Server {} error processing RECIEVED PACKET: {}", self.node_id, e);
//                         }
//                     }
//                 }
//
//             }
//             if !self.servers.is_empty() {
//                 // if(!executed) {
//                 //     //self.handle_command("server_type?->6");
//                 //     //self.handle_command("files_list?->6");
//                 //     //self.handle_command("registration_to_chat->6");
//                 //     // self.handle_command("client_list->6");
//                 //     //self.handle_command("message_for?(10, hahaha)->6");
//                 //     self.handle_command("server_list");
//                 //     executed = true;
//                 // }
//             }
//         }
//     }
pub fn run(&mut self){
    init_logger();
    self.discover_network();
    let (cmd_snd,cmd_rcv) = unbounded::<String>();
    let (msg_snd,msg_rcv) = unbounded::<String>();
    let receiver_channel = self.receiver_channel.clone();

    let cl2_ui = Client2_UI::new(
        self.node_id.clone(),
        Arc::clone(&self.other_client_ids),
        Arc::clone(&self.servers),
        Arc::clone(&self.files_names),
        cmd_snd,
        msg_rcv
    );
    if let Some(ui_snd) = self.ui_snd.take(){
        ui_snd.send(cl2_ui).expect("Failed to send");
    }

    //Packet handle part
    loop {
        select_biased! {
                // Handle packets in the meantime
                    recv(receiver_channel) -> packet =>{
                        match packet{
                                Ok(packet) => {
                                        self.handle_packet(packet, &msg_snd); //, &msg_snd
                                },
                                Err(e) => ()//println!("Err2: {e}")
                        }
                    }
                    recv(cmd_rcv) -> cmd => {
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
                }
    }
}

}

/*#[test]
fn test_discovery(){
    let (snd, rcv) = unbounded::<Packet>();
    let mut cl = Client2::new(1, HashMap::new(), rcv);
    cl.neighbor_senders.insert(10, snd);
    cl.network_graph.lock().unwrap().insert(1, HashSet::from_iter(vec![2, 3]));
    cl.network_graph.lock().unwrap().insert(2, HashSet::from_iter(vec![3]));
    cl.network_graph.lock().unwrap().insert(3, HashSet::from_iter(vec![2]));
    cl.network_graph.lock().unwrap().insert(4, HashSet::from_iter(vec![2, 3]));
    cl.discover_network();
    println!("The graph is {:?}", cl.network_graph.lock().unwrap());
}*/