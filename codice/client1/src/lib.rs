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
#![allow(warnings)]
mod fragment_reassembler;
mod communication;
use fragment_reassembler::*;
use std::collections::{HashMap,VecDeque};
use std::{io};
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use rand::distr::uniform::SampleBorrow;
use wg_2024::packet::*;
use wg_2024::network::*;



//Client struct and functions/methods related. Client has some additional fields to handle more things
type Graph = HashMap<NodeId,Vec<NodeId>>;

pub struct Client1 {
    node_id: NodeId,
    sender_channels: HashMap<NodeId,Sender<Packet>>,
    receiver_channel: Receiver<Packet>,
    flood_ids: Vec<(u64,NodeId)>,
    network: Graph,
    fragment_reassembler: FragmentReassembler, // Used to handle fragments
    received_files: Vec<String>,   // Path where to save files received
    other_client_ids: Vec<NodeId>, // Storage other client IDs
    files_names: Vec<String>,   // Storage of file names
    servers: HashMap<NodeId,String>, // map of servers ID and relative type
}

impl Client1 {
    // Create a new Client with parameters from Network Initializer
    pub fn new(node_id: NodeId,
               sender_channels:HashMap<NodeId,Sender<Packet>>,
               receiver_channel: Receiver<Packet>
    ) -> Self {
        Self {

            node_id,
            sender_channels,
            receiver_channel,
            flood_ids: vec![],
            network: Graph::new(),
            fragment_reassembler: FragmentReassembler::new(),
            received_files: vec![],
            other_client_ids: vec![],
            files_names: vec![],
            servers: HashMap::new(),
        }
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
        for neighbor in neighbors{
            //println!("CLIENT1: Sending flood request to Drone {}",neighbor);
            match self.sender_channels.get(&neighbor).expect("CLIENT1: Didn't find neighbor").send(self.create_flood_request(request.clone(), neighbor,session_id)){
                Ok(_) => (),
                Err(_) =>{self.sender_channels.remove(&neighbor); self.discover_network() }
            }
        }
    }
    // Creation of Packet with packet.type = FloodResponse
    pub fn create_flood_response(&self,session_id: u64, request: FloodRequest) -> Packet{
        let mut hops: Vec<NodeId> = vec![];
        for &e in &request.path_trace{
            hops.push(e.0.clone());
        }
        hops.reverse();
        let srh = SourceRoutingHeader::with_first_hop(hops);
        let flood_resp = FloodResponse{
            flood_id: request.flood_id,
            path_trace: request.path_trace
        };
        Packet::new_flood_response(srh,session_id,flood_resp)

    }
    // Creation of a packet of type FloodRequest
    pub fn create_flood_request(&self, request: FloodRequest, neighbor: NodeId,session_id: u64) -> Packet{
        let hops: Vec<NodeId> = vec![self.node_id,neighbor]; // TODO check if correct
        let srh = SourceRoutingHeader::with_first_hop(hops);
        Packet::new_flood_request(srh,session_id,request)
    }
    // Function to handle received packets of type FloodRequest
    pub fn handle_flood_request(&mut self,packet: Packet) {
        //let packet_clone = packet.clone();
        match packet.pack_type{
            PacketType::FloodRequest(mut request)=>{
                let mut previous = 0;
                match request.path_trace.last(){
                    Some(last) => {
                        previous = last.0;
                        if self.flood_ids.contains(&(request.flood_id,request.initiator_id)){
                            request.path_trace.push((self.node_id.clone(),NodeType::Client));
                            let resp = self.create_flood_response(packet.session_id,request);
                            match self.sender_channels.get(&previous).expect("CLIENT1: Didn't find neighbor").send(resp){
                                Ok(_) => (),
                                Err(_) =>{self.sender_channels.remove(&previous); self.discover_network(); }
                            }
                        }
                        else {
                            request.path_trace.push((self.node_id.clone(), NodeType::Client));
                            self.flood_ids.push((request.flood_id, request.initiator_id));
                            let resp = self.create_flood_response(packet.session_id, request);
                            match self.sender_channels.get(&previous).expect("CLIENT1: Didn't find neighbor").send(resp){
                                Ok(_) => (),
                                Err(_) =>{self.sender_channels.remove(&previous); self.discover_network(); }
                            }
                        }
                    }
                    _ => {println!("CLIENT1: Can't find neighbour who sent this packet {} ", request);}
                }
            }
            _=>{println!("CLIENT1: Wrong packet type received")}
        }
    }
    // Handle received packets of type FloodResponse. Update knowledge of the network
    pub fn handle_flood_response(&mut self, packet: Packet) {
        //println!("CLIENT1: arrived FloodResponse");
        match packet.pack_type{
            PacketType::FloodResponse(response) =>{
                for node in &response.path_trace{
                    if node.1.eq(&NodeType::Server){
                        self.servers.entry(node.0).or_insert("".to_string());
                        println!("CLIENT1: found server with id: {}",node.0);
                    }
                }
                self.update_graph(response);
            }
            _ => {println!("CLIENT1: Wrong packet type received")}
        }
    }
    // Handle received packets of type MsgFragment. Fragments put together and reassembled
    pub fn handle_msg_fragment(&mut self, packet: Packet){
        match packet.pack_type{
            PacketType::MsgFragment(fragment)=>{
                //println!("{:?}",fragment);
                let frag_index = fragment.fragment_index;
                // Check if a fragment with the same (session_id,src_id) has already been received
                match self.fragment_reassembler.add_fragment(packet.session_id,packet.routing_header.hops[0], fragment).expect("CLIENT1: Error while processing fragment"){
                    Some(message) =>{
                        match FragmentReassembler::assemble_string_file(message,&mut self.received_files){
                            // Check FragmentReassembler output and behave accordingly
                            Ok(msg) => {
                                //println!("message before: {}",msg);
                                let mut new_hops = packet.routing_header.hops.clone();
                                let dest_id = new_hops[0];
                                new_hops.reverse();
                                let new_first_hop = new_hops[1];

                                //Handle the reconstructed message
                                println!("{}",self.handle_msg(msg,packet.session_id,new_first_hop,frag_index));
                                // A message is reconstructed: create and send back an Ack
                                let new_pack = Packet::new_ack(
                                    SourceRoutingHeader::with_first_hop(new_hops),packet.session_id,frag_index);

                                match self.sender_channels.get(&new_first_hop).expect("CLIENT1: Didn't find neighbor").send(new_pack){
                                    Ok(_) => (),
                                    Err(_) =>{ // Error: the first node is crashed

                                        self.sender_channels.remove(&new_first_hop);
                                        self.discover_network();

                                        let new_path = Self::bfs_compute_path(&self.network,self.node_id,dest_id).expect("Failed to create path");
                                        let first_hop = new_path[1];

                                        let packet_sent = Packet::new_ack(
                                            SourceRoutingHeader::with_first_hop(new_path),packet.session_id,frag_index);
                                        if let Some(sender) = self.sender_channels.get(&first_hop){
                                            sender.send(packet_sent).expect("CLIENT1: failed to send message");
                                        }
                                    }
                                }
                            },
                            // FragmentReassembler encountered an error
                            Err(e) => println!("{e}")
                        }
                    }
                    // There are still Fragments missing: send back Ack for current fragment in the meantime
                    None => {
                        let mut new_hops = packet.routing_header.hops.clone();
                        let dest_id = new_hops[0];
                        new_hops.reverse();
                        let new_first_hop = new_hops[1];
                        let new_pack = Packet::new_ack(
                            SourceRoutingHeader::with_first_hop(new_hops),packet.session_id,frag_index);
                        match self.sender_channels.get(&new_first_hop).expect("CLIENT1: Didn't find neighbor").send(new_pack){
                            Ok(_) => (),
                            Err(_) =>{ // Error: the first node is crashed

                                self.sender_channels.remove(&new_first_hop);
                                self.discover_network();

                                let new_path = Self::bfs_compute_path(&self.network,self.node_id,dest_id).expect("Failed to create path");
                                let first_hop = new_path[1];

                                let packet_sent = Packet::new_ack(
                                    SourceRoutingHeader::with_first_hop(new_path),packet.session_id,frag_index);
                                if let Some(sender) = self.sender_channels.get(&first_hop){
                                    sender.send(packet_sent).expect("CLIENT1: failed to send message");
                                }
                            }
                        }
                    }
                }
            }
            _ =>{println!("CLIENT1: Wrong packet type received")}
        }
    }
    // Helper functions
    pub fn generate_flood_id() -> u64{
        rand::random()
    }
    pub fn generate_session_id() -> u64{
        rand::random()
    }

    // Update the knowledge of the network based on flood responses
    pub fn update_graph(&mut self, response: FloodResponse){
        let path = &response.path_trace;
        //println!("CLIENT1:{:?}",path);
        // Iterate over consecutive pairs in the path_trace
        for window in path.windows(2) {
            if let [(node1, _type1), (node2, _type2)] = window {

                self.network.entry(*node1).or_insert_with(Vec::new);
                // Add node1 -> node2
                if let Some(neighbors) = self.network.get_mut(&node1){
                    if !neighbors.contains(&node2){
                        neighbors.push(*node2);
                    }
                }
                self.network.entry(*node2).or_insert_with(Vec::new);
                // Add node2 -> node1 (bidirectional edge)
                if let Some(neighbors) = self.network.get_mut(&node2){
                    if !neighbors.contains(&node1){
                        self.network.entry(*node2).or_insert_with(Vec::new).push(*node1);
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
    pub fn handle_packet(&mut self, packet: Packet){
        match packet.pack_type{
            PacketType::FloodRequest(_) =>{
                self.handle_flood_request(packet);
            }
            PacketType::FloodResponse(_) =>{
                self.handle_flood_response(packet);
            }
            PacketType::MsgFragment(_) =>{
                self.handle_msg_fragment(packet);
            }
            _ => ()
        }
    }
    pub fn run(&mut self) {
        //Initialize the network field
        self.discover_network();
        let mut input_buffer = String::new();

        loop {
            // Handle packets in the meantime
            select_biased!{
                recv(self.receiver_channel) -> packet =>{
                    match packet{
                        Ok(packet) => {
                            //println!("CLIENT1: Received packet");
                            self.handle_packet(packet);
                        },

                        Err(e) => println!("CLIENT1: Error: {e}")
                    }
                }
            }
            match self.servers.is_empty(){
                true => (),
                false =>{
                    input_buffer.clear();
                    io::stdin().read_line(&mut input_buffer).expect("CLIENT1: Failed to read line");
                    if input_buffer.eq("OFF"){
                        break;
                    }
                    else if input_buffer.trim().eq("Commands") {
                        println!("CLIENT1:\n
                            server_type?->NodeId #(to a server in general)\n
                            files_list?->NodeId #(to a ContentServer)\n
                            file?(file_id)->NodeId #(to a ContentServer)\n
                            media?(media_id)->NodeId #(to a ContentServer)\n
                            message_for?(client_id, message)->NodeId #(to a ChatServer)\n
                            client_list?->NodeID #(to a ChatServer)\n
                        ");
                    }
                    else{
                        self.handle_command(input_buffer.trim());
                        input_buffer.clear();
                    }
                }
            }
        }
    }
}

// Tests for bfs and network update based on FloodResponses
#[cfg(test)]
mod test{
    use super::*;
    use Client1;
    #[test]
    fn test_bfs_shortest_path() {
        let (snd, rcv) = unbounded::<Packet>();
        let mut cl = Client1::new(1, HashMap::new(), rcv);
        cl.sender_channels.insert(19, snd);
        cl.network.insert(1, vec![2, 3]);
        cl.other_client_ids.push(2);
        cl.network.insert(2, vec![1, 4]);
        cl.network.insert(3, vec![1, 4]);
        cl.network.insert(4, vec![2, 3, 6]);
        cl.network.insert(5, vec![1, 4]);
        cl.network.insert(6, vec![5, 8, 10]);
        cl.network.insert(7, vec![2, 3]);
        cl.network.insert(8, vec![7, 9]);
        cl.network.insert(9, vec![2, 3]);
        cl.network.insert(10, vec![3 ,6, 14]);
        cl.network.insert(14, vec![10]);
        let test_res1 = Client1::bfs_compute_path(&cl.network, 1, 9).unwrap();
        assert_eq!(test_res1, vec![1, 2, 4, 6, 8, 9]);

        let test_res2 = Client1::bfs_compute_path(&cl.network, 1, 14).unwrap();
        assert_eq!(test_res2, vec![1, 2, 4, 6, 10, 14]);
    }
    #[test]
    fn test_bfs_no_shortest_path() {
        let (snd, rcv) = unbounded::<Packet>();
        let mut cl = Client1::new(1, HashMap::new(), rcv);
        cl.sender_channels.insert(2, snd);
        cl.network.insert(1, vec![2, 3]);
        cl.other_client_ids.push(2);
        cl.network.insert(2, vec![1, 4]);
        cl.network.insert(3, vec![1, 4]);
        cl.network.insert(4, vec![2, 3]);
        cl.network.insert(5, vec![1, 4]);
        cl.network.insert(6, vec![5, 8]);
        cl.network.insert(7, vec![2, 3]);
        cl.network.insert(8, vec![7, 9]);
        cl.network.insert(9, vec![2, 3]);
        let test_res = Client1::bfs_compute_path(&cl.network, 1, 9);
        assert!(test_res.is_none());
    }
    #[test]
    fn test_update_graph(){
        let (snd, rcv) = unbounded::<Packet>();
        let mut cl = Client1::new(1, HashMap::new(), rcv);
        cl.sender_channels.insert(2, snd);
        cl.network.insert(1, vec![2]);
        let mut f_req = FloodRequest::new(1234, 1);
        f_req.path_trace.push((2,NodeType::Drone));
        f_req.path_trace.push((3,NodeType::Drone));
        f_req.path_trace.push((4,NodeType::Drone));
        f_req.path_trace.push((5,NodeType::Drone));
        f_req.path_trace.push((6,NodeType::Server));

        let resp = cl.create_flood_response(1234,f_req);
        match resp.pack_type{
            PacketType::FloodResponse(fr)=>{
                cl.update_graph(fr);
            }
            _=> ()
        }
        let test_res = Client1::bfs_compute_path(&cl.network, 1, 6).unwrap();
        assert_eq!(test_res, vec![1, 2, 3, 4, 5, 6]);
    }
}