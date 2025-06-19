#![allow(warnings)]

/*
INFORMATION

This is the implementation of a Client made by Lorenzo Cortese for the AP project of academic year 2024/2025 held by professor Marco Patrignani.

CONTENTS

There are three files and a folder in total in the /src folder : 'lib', 'communication.rs', 'fragment_reassembler.rs', 'tests'.

Their contents are:

    * 'lib' : Client struct with the necessary methods and functions
    to handle the user input and the incoming packets and also some helpers functions.
    In order to check what commands can be executed by the user, digit 'Commands' in the cmd line.

    * 'communication.rs' : methods and functions related to the aforementioned file used to handle both user commands and messages received at a lower level
    and, in fact, do the effective communication part.

    * 'fragment_reassembler.rs' : FragmentReassembler struct with related methods used to store,reconstruct and assemble the MsgFragment packet types.

    * 'tests' folder: couple of files used to test the FragmentReassembler functionalities.

All the aforementioned files have some tests within to ensure their most important functions behave correctly.
*/
pub mod client1_ui;
mod communication;
mod fragment_reassembler;
mod logger;
use crate::client1_ui::Client1_UI;
use crate::logger::logger::{init_logger, write_log};
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use fragment_reassembler::*;
use std::collections::{HashMap, VecDeque};
use std::{env, thread};
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use wg_2024::network::*;
use wg_2024::packet::*;

//Client struct and functions/methods related. Client has some additional fields to handle more things
type Graph = HashMap<NodeId, Vec<NodeId>>;
type AckKey = (u64, u64);
type AckMap = Arc<Mutex<HashMap<AckKey, Packet>>>;
pub struct Client1 {
    node_id: NodeId,
    sender_channels: HashMap<NodeId, Sender<Packet>>,
    receiver_channel: Receiver<Packet>,
    crashed_drone_rcv: Receiver<NodeId>,
    flood_ids: Vec<(u64, NodeId)>,
    network: Graph,
    fragment_reassembler: FragmentReassembler, // Used to handle fragments
    received_files: Vec<String>,               // Path where to save files received
    other_client_ids: Arc<Mutex<Vec<NodeId>>>, // Storage other client IDs
    files_names: Arc<Mutex<HashMap<NodeId, Vec<String>>>>, // Storage of file names
    servers: Arc<Mutex<HashMap<NodeId, String>>>, // map of servers ID and relative type
    packet_sent: AckMap,
    selected_file_name: String,
    selected_server: NodeId,
    cmd_rcv: Receiver<String>,
    msg_snd: Sender<String>,
    redo_discovery: Arc<(Mutex<bool>,Condvar)>
}

impl Client1 {
    // Create a new Client with parameters from Network Initializer
    pub fn new(
        node_id: NodeId,
        sender_channels: HashMap<NodeId, Sender<Packet>>,
        receiver_channel: Receiver<Packet>,
        crashed_drone_rcv: Receiver<NodeId>
    ) -> (Self, Client1_UI) {
        let other_client_ids = Arc::new(Mutex::new(vec![]));
        let files_names = Arc::new(Mutex::new(HashMap::new()));
        let servers = Arc::new(Mutex::new(HashMap::new()));
        let packet_sent = Arc::new(Mutex::new(HashMap::<AckKey, Packet>::new()));

        let (cmd_snd, cmd_rcv) = unbounded::<String>();
        let (msg_snd, msg_rcv) = unbounded::<String>();

        let client_ui = Client1_UI::new(
            node_id,
            Arc::clone(&other_client_ids),
            Arc::clone(&servers),
            Arc::clone(&files_names),
            cmd_snd,
            msg_rcv,
        );
        let discovery_complete = Arc::new((Mutex::new(false),Condvar::new()));
        (
            Self {
                node_id,
                sender_channels,
                receiver_channel,
                crashed_drone_rcv,
                flood_ids: vec![],
                network: Graph::new(),
                fragment_reassembler: FragmentReassembler::new(),
                received_files: vec![],
                other_client_ids,
                files_names,
                servers,
                packet_sent,
                selected_file_name: String::new(),
                selected_server: NodeId::default(),
                cmd_rcv,
                msg_snd,
                redo_discovery: discovery_complete
            },
            client_ui,
        )
    }
    // Network discovery
    pub fn discover_network(&mut self) {
        let request = FloodRequest {
            flood_id: Self::generate_flood_id(),
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };
        let neighbors: Vec<_> = self.sender_channels.keys().cloned().collect();
        let session_id = Self::generate_session_id();
        for neighbor in neighbors {
            //println!("CLIENT1: Sending flood request to Drone {}",neighbor);
            //write_log(&format!("{:?}",neighbor));
            match self
                .sender_channels
                .get(&neighbor)
                .expect("CLIENT1: Didn't find neighbor 1")
                .send(self.create_flood_request(request.clone(), neighbor, session_id))
            {
                Ok(_) => (),
                Err(_) => {
                    self.sender_channels.remove(&neighbor);
                    self.discover_network()
                }
            }
        }
    }
    // Creation of Packet with packet.type = FloodResponse
    pub fn create_flood_response(&self, session_id: u64, request: FloodRequest) -> Packet {
        let mut hops: Vec<NodeId> = vec![];
        for &e in &request.path_trace {
            hops.push(e.0.clone());
        }
        hops.reverse();
        let srh = SourceRoutingHeader::with_first_hop(hops);
        let flood_resp = FloodResponse {
            flood_id: request.flood_id,
            path_trace: request.path_trace,
        };
        Packet::new_flood_response(srh, session_id, flood_resp)
    }
    // Creation of a packet of type FloodRequest
    pub fn create_flood_request(
        &self,
        request: FloodRequest,
        neighbor: NodeId,
        session_id: u64,
    ) -> Packet {
        let hops: Vec<NodeId> = vec![self.node_id, neighbor];
        let srh = SourceRoutingHeader::with_first_hop(hops);
        let res = Packet::new_flood_request(srh, session_id, request);
        //println!("DEBUG CLIENT 1: {:?}",res);
        res
    }
    // Function to handle received packets of type FloodRequest
    pub fn handle_flood_request(&mut self, packet: Packet) {
        //let packet_clone = packet.clone();
        match packet.pack_type {
            PacketType::FloodRequest(mut request) => {
                let mut previous = 0;
                //write_log(&format!("{:?}",request.path_trace.clone()));
                match request.path_trace.last() {

                    Some(last) => {
                        previous = last.0;
                        if self.sender_channels.get(&previous).is_some(){
                            if self
                                .flood_ids
                                .contains(&(request.flood_id, request.initiator_id))
                            {
                                request
                                    .path_trace
                                    .push((self.node_id.clone(), NodeType::Client));
                                let resp = self.create_flood_response(packet.session_id, request);
                                //write_log(&format!("{:?}",previous));
                                match self
                                    .sender_channels
                                    .get(&previous)
                                    .expect("CLIENT1: Didn't find neighbor 2")
                                    .send(resp)
                                {
                                    Ok(_) => (),
                                    Err(_) => {
                                        self.sender_channels.remove(&previous);
                                        self.redo_network();
                                    }
                                }
                            } else {
                                request
                                    .path_trace
                                    .push((self.node_id.clone(), NodeType::Client));
                                self.flood_ids
                                    .push((request.flood_id, request.initiator_id));
                                let resp = self.create_flood_response(packet.session_id, request);
                                //write_log(&format!("{:?}",previous));
                                match self
                                    .sender_channels
                                    .get(&previous)
                                    .expect("CLIENT1: Didn't find neighbor 3")
                                    .send(resp)
                                {
                                    Ok(_) => (),
                                    Err(_) => {
                                        self.sender_channels.remove(&previous);
                                        self.redo_network();
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        println!(
                            "CLIENT1: Can't find neighbour who sent this packet {} ",
                            request
                        );
                    }
                }
            }
            _ => {
                println!("CLIENT1: Wrong packet type received")
            }
        }
    }
    // Handle received packets of type FloodResponse. Update knowledge of the network
    pub fn handle_flood_response(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::FloodResponse(response) => {
                self.update_graph(response.clone());
                for node in &response.path_trace {
                    if node.1.eq(&NodeType::Server) {
                        self.servers
                            .lock()
                            .expect("Failed to lock")
                            .entry(node.0)
                            .or_insert("".to_string());
                        // Check for the server type immediately after receiving a response
                        let mut t = "server_type?->".to_string();
                        t.push_str(node.0.to_string().as_str());
                        self.handle_command(t.clone());
                    }
                }
            }
            _ => {
                println!("CLIENT1: Wrong packet type received")
            }
        }
    }
    // Handle received packets of type MsgFragment. Fragments put together and reassembled
    pub fn handle_msg_fragment(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::MsgFragment(fragment) => {
                let frag_index = fragment.fragment_index;
                // Check if a fragment with the same (session_id,src_id) has already been received
                match self
                    .fragment_reassembler
                    .add_fragment(packet.session_id, packet.routing_header.hops[0], fragment)
                    .expect("Failed to add fragment")
                {
                    Some(message) => {
                        match FragmentReassembler::assemble_string_file(message.clone()) {
                            // Check FragmentReassembler output and behave accordingly
                            Ok(msg) => {
                                let mut new_hops = packet.routing_header.hops.clone();
                                let dest_id = new_hops[0].clone();
                                new_hops.reverse();
                                let new_first_hop = new_hops[1];
                                //write_log(msg.as_str());
                                //Handle the reconstructed message
                                if msg.starts_with("server_type!(")
                                    || msg.starts_with("client_list!(")
                                    || msg.starts_with("files_list!(")
                                {
                                    self.handle_msg(msg, dest_id);
                                } else {
                                    self.msg_snd
                                        .clone()
                                        .send(self.handle_msg(msg, dest_id))
                                        .expect("Failed to send message");
                                }
                                // A message is reconstructed: create and send back an Ack
                                let new_pack = Packet::new_ack(
                                    SourceRoutingHeader::with_first_hop(new_hops),
                                    packet.session_id,
                                    frag_index,
                                );
                                //write_log(&format!("{:?}",new_first_hop));
                                match self
                                    .sender_channels
                                    .get(&new_first_hop)
                                    .expect("CLIENT1: Didn't find neighbor 4")
                                    .send(new_pack)
                                {
                                    Ok(_) => (),
                                    Err(_) => {
                                        // Error: the first node is crashed
                                        //POSSIBLE ERROR HERE TODO
                                        self.sender_channels.remove(&new_first_hop);
                                        self.redo_network();

                                        let new_path = Self::bfs_compute_path(
                                            &self.network,
                                            self.node_id,
                                            dest_id,
                                        )
                                        .expect("Failed to create path");
                                        let first_hop = new_path[1];

                                        let packet_sent = Packet::new_ack(
                                            SourceRoutingHeader::with_first_hop(new_path),
                                            packet.session_id,
                                            frag_index,
                                        );
                                        if let Some(sender) = self.sender_channels.get(&first_hop) {
                                            sender
                                                .send(packet_sent)
                                                .expect("CLIENT1: failed to send message");
                                        }
                                    }
                                }
                            }
                            // assemble_string_file encountered an error: the file is big --> it needs to be processed in a different way
                            Err(_) => {
                                let path =
                                    env::current_dir().expect("Failed to get current_dir value");
                                let mut file_path = path;
                                let path = self.selected_file_name.clone();
                                file_path.push(path.as_str());
                                let msg = FragmentReassembler::assemble_file(
                                    message,
                                    file_path
                                        .as_path()
                                        .to_str()
                                        .expect("Failed to convert to Path"),
                                )
                                .expect("Failed to assemble file ");
                                let mut new_hops = packet.routing_header.hops.clone();
                                let dest_id = new_hops[0].clone();
                                new_hops.reverse();
                                let new_first_hop = new_hops[1];

                                //Send the reconstructed message
                                self.msg_snd.send(msg).expect("Failed to send message");

                                // A message is reconstructed: create and send back an Ack
                                let new_pack = Packet::new_ack(
                                    SourceRoutingHeader::with_first_hop(new_hops),
                                    packet.session_id,
                                    frag_index,
                                );
                                //write_log(&format!("{:?}",new_first_hop));
                                match self
                                    .sender_channels
                                    .get(&new_first_hop)
                                    .expect("CLIENT1: Didn't find neighbor 5")
                                    .send(new_pack)
                                {
                                    Ok(_) => (),
                                    Err(_) => {
                                        // Error: the first node is crashed
                                        //POSSIBLE ERROR HERE TODO
                                        self.sender_channels.remove(&new_first_hop);
                                        self.redo_network();

                                        let new_path = Self::bfs_compute_path(
                                            &self.network,
                                            self.node_id,
                                            dest_id,
                                        )
                                        .expect("Failed to create path");
                                        let first_hop = new_path[1];

                                        let packet_sent = Packet::new_ack(
                                            SourceRoutingHeader::with_first_hop(new_path),
                                            packet.session_id,
                                            frag_index,
                                        );
                                        if let Some(sender) = self.sender_channels.get(&first_hop) {
                                            sender
                                                .send(packet_sent)
                                                .expect("CLIENT1: failed to send message");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // There are still Fragments missing: send back Ack for current fragment in the meantime
                    None => {
                        let mut new_hops = packet.routing_header.hops.clone();
                        let dest_id = new_hops[0].clone();
                        new_hops.reverse();
                        let new_first_hop = new_hops[1];
                        let new_pack = Packet::new_ack(
                            SourceRoutingHeader::with_first_hop(new_hops),
                            packet.session_id,
                            frag_index,
                        );
                        //write_log(&format!("{:?}",new_first_hop));
                        match self
                            .sender_channels
                            .get(&new_first_hop)
                            .expect("CLIENT1: Didn't find neighbor 6")
                            .send(new_pack)
                        {
                            Ok(_) => (),
                            Err(_) => {
                                // Error: the first node is crashed
                                //POSSIBLE ERROR HERE TODO
                                self.sender_channels.remove(&new_first_hop);
                                self.redo_network();

                                let new_path =
                                    Self::bfs_compute_path(&self.network, self.node_id, dest_id)
                                        .expect("Failed to create path");
                                let first_hop = new_path[1];

                                let packet_sent = Packet::new_ack(
                                    SourceRoutingHeader::with_first_hop(new_path),
                                    packet.session_id,
                                    frag_index,
                                );
                                if let Some(sender) = self.sender_channels.get(&first_hop) {
                                    sender
                                        .send(packet_sent)
                                        .expect("CLIENT1: failed to send message");
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                println!("CLIENT1: Wrong packet type received")
            }
        }
    }
    // Helper functions
    pub fn generate_flood_id() -> u64 {
        rand::random()
    }
    pub fn generate_session_id() -> u64 {
        rand::random()
    }

    // Update the knowledge of the network based on flood responses
    pub fn update_graph(&mut self, response: FloodResponse) {
        let path = &response.path_trace;
        // Iterate over consecutive pairs in the path_trace
        for window in path.windows(2) {
            if let [(node1, _type1), (node2, _type2)] = window {
                self.network.entry(*node1).or_insert_with(Vec::new);
                // Add node1 -> node2
                if let Some(neighbors) = self.network.get_mut(&node1) {
                    if !neighbors.contains(&node2) {
                        neighbors.push(*node2);
                    }
                }
                self.network.entry(*node2).or_insert_with(Vec::new);
                // Add node2 -> node1 (bidirectional edge)
                if let Some(neighbors) = self.network.get_mut(&node2) {
                    if !neighbors.contains(&node1) {
                        self.network
                            .entry(*node2)
                            .or_insert_with(Vec::new)
                            .push(*node1);
                    }
                }
            }
        }
        //println!("Network: {:?}",self.network);
    }
    // Calculates shortest path between two nodes
    pub fn bfs_compute_path(graph: &Graph, start: NodeId, end: NodeId) -> Option<Vec<NodeId>> {
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();
        let mut parent = HashMap::new();

        queue.push_back(start);
        visited.insert(start, true);

        while let Some(current) = queue.pop_front() {
            if current == end {
                let mut path = vec![end];
                let mut node = end;
                while let Some(&p) = parent.get(&node) {
                    path.push(p);
                    node = p;
                }
                path.reverse();
                return Some(path);
            }
            if let Some(neighbors) = graph.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains_key(&neighbor) {
                        queue.push_back(neighbor);
                        visited.insert(neighbor, true);
                        parent.insert(neighbor, current);
                    }
                }
            }
        }
        None
    }
    // Handle incoming packets and call different methods based on the packet type. Ack and Nack handled in the upper function
    pub fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::FloodRequest(_) => {
                self.handle_flood_request(packet);
            }
            PacketType::FloodResponse(_) => {
                self.handle_flood_response(packet);
            }
            PacketType::MsgFragment(_) => {
                self.handle_msg_fragment(packet);
            }
            PacketType::Nack(nack) =>{
                match nack.nack_type{
                    NackType::ErrorInRouting(_) =>{
                        self.redo_network();
                    }
                    _ => ()
                }
            }
            PacketType::Ack(ack) =>{
                let key = (packet.session_id,ack.fragment_index);
                self.packet_sent.lock().expect("Failed to lock").remove(&key);
            }
        }
    }
    pub fn redo_network(&mut self){
        self.network.clear();
        let redo_discovery = Arc::clone(&self.redo_discovery);
        {
            let (lock, _cvar) = &*self.redo_discovery;
            let mut done = lock.lock().expect("Failed to lock");
            *done = false; // Ensure fresh discovery state
        }
        self.discover_network();
        thread::spawn(move || {
            let (lock, cvar) = &*redo_discovery;
            let mut done = lock.lock().expect("Failed to lock");
            thread::sleep(Duration::from_millis(300));
            *done = true;
            cvar.notify_all();
        });
        let (lock, cvar) = &*self.redo_discovery;
        let mut done = lock.lock().expect("Failed to lock");
        while !*done{
            done = cvar.wait(done).expect("Failed to get value");
        }
    }

    pub fn run(&mut self) {
        init_logger();
        //Initialize network field
        self.discover_network();
        let receiver_channel = self.receiver_channel.clone();

        //Packet handle part
        loop {
            select_biased! {
            // Handle packets in the meantime
                recv(receiver_channel) -> packet =>{
                    match packet{
                            Ok(packet) => {
                                    self.handle_packet(packet);
                            },
                            Err(e) => ()//println!("Err1: {e}")
                    }
                }
                recv(self.cmd_rcv) -> cmd => {
                    match cmd {
                        Ok(cmd) => {
                            match self.handle_command(cmd.clone()).as_str() {
                                "CLIENT1: OK" => (),
                                e => () //println!("Err2: {e}")
                            }
                        }
                        Err(_) =>  ()//println!("Err3: {e}") // Normal that prints at the end, the UI is closed
                    }
                }
                recv(self.crashed_drone_rcv) -> crashed_drone_id => {
                    match crashed_drone_id{
                        Ok(id) =>{
                            self.sender_channels.remove(&id);
                            self.redo_network();
                        }
                        Err(_) => ()
                    }
                }
            }
        }
    }
}

// Tests for bfs and network update based on FloodResponses
#[cfg(test)]
mod test {
    use super::*;
    use Client1;
    #[test]
    fn test_bfs_shortest_path() {
        let (snd, rcv) = unbounded::<Packet>();
        let (_,rcv_id) = unbounded::<NodeId>();
        let mut cl = Client1::new(1, HashMap::new(), rcv,rcv_id);
        cl.0.sender_channels.insert(19, snd);
        cl.0.network.insert(1, vec![2, 3]);
        cl.0.other_client_ids.lock().expect("Failed to lock").push(2);
        cl.0.network.insert(2, vec![1, 4]);
        cl.0.network.insert(3, vec![1, 4]);
        cl.0.network.insert(4, vec![2, 3, 6]);
        cl.0.network.insert(5, vec![1, 4]);
        cl.0.network.insert(6, vec![5, 8, 10]);
        cl.0.network.insert(7, vec![2, 3]);
        cl.0.network.insert(8, vec![7, 9]);
        cl.0.network.insert(9, vec![2, 3]);
        cl.0.network.insert(10, vec![3, 6, 14]);
        cl.0.network.insert(14, vec![10]);
        let test_res1 = Client1::bfs_compute_path(&cl.0.network, 1, 9).unwrap();
        assert_eq!(test_res1, vec![1, 2, 4, 6, 8, 9]);

        let test_res2 = Client1::bfs_compute_path(&cl.0.network, 1, 14).unwrap();
        assert_eq!(test_res2, vec![1, 2, 4, 6, 10, 14]);
    }
    #[test]
    fn test_bfs_no_shortest_path() {
        let (snd, rcv) = unbounded::<Packet>();
        let (_,rcv_id) = unbounded::<NodeId>();
        let mut cl = Client1::new(1, HashMap::new(), rcv,rcv_id);
        cl.0.sender_channels.insert(2, snd);
        cl.0.network.insert(1, vec![2, 3]);
        cl.0.other_client_ids.lock().expect("Failed to lock").push(2);
        cl.0.network.insert(2, vec![1, 4]);
        cl.0.network.insert(3, vec![1, 4]);
        cl.0.network.insert(4, vec![2, 3]);
        cl.0.network.insert(5, vec![1, 4]);
        cl.0.network.insert(6, vec![5, 8]);
        cl.0.network.insert(7, vec![2, 3]);
        cl.0.network.insert(8, vec![7, 9]);
        cl.0.network.insert(9, vec![2, 3]);
        let test_res = Client1::bfs_compute_path(&cl.0.network, 1, 9);
        assert!(test_res.is_none());
    }
    #[test]
    fn test_update_graph() {
        let (snd, rcv) = unbounded::<Packet>();
        let (_,rcv_id) = unbounded::<NodeId>();
        let mut cl = Client1::new(1, HashMap::new(), rcv,rcv_id);
        cl.0.sender_channels.insert(2, snd);
        cl.0.network.insert(1, vec![2]);
        let mut f_req = FloodRequest::new(1234, 1);
        f_req.path_trace.push((2, NodeType::Drone));
        f_req.path_trace.push((3, NodeType::Drone));
        f_req.path_trace.push((4, NodeType::Drone));
        f_req.path_trace.push((5, NodeType::Drone));
        f_req.path_trace.push((6, NodeType::Server));

        let resp = cl.0.create_flood_response(1234, f_req);
        match resp.pack_type {
            PacketType::FloodResponse(fr) => {
                cl.0.update_graph(fr);
            }
            _ => (),
        }
        let test_res = Client1::bfs_compute_path(&cl.0.network, 1, 6).unwrap();
        assert_eq!(test_res, vec![1, 2, 3, 4, 5, 6]);
    }
}