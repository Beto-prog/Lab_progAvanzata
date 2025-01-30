#![allow(warnings)]
mod fragment_reassembler;
mod communication;
use communication::*;
use fragment_reassembler::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use crossbeam_channel::{Receiver, Sender};
use rand::distr::uniform::SampleBorrow;
use wg_2024::packet::*;
use wg_2024::network::*;

type Graph = HashMap<NodeId,Vec<NodeId>>;

pub struct Client {
    node_id: NodeId,
    neighbors: HashSet<NodeId>,
    sender_channels: HashMap<NodeId,Sender<Packet>>,
    receiver_channel: Receiver<Packet>,
    flood_ids: Vec<(u64,NodeId)>,
    network: Graph,
    fragment_reassembler: FragmentReassembler,
    path: String
}

impl Client {
    pub fn new(node_id: NodeId,
               neighbors: HashSet<NodeId>,
               sender_channels:HashMap<NodeId,Sender<Packet>>,
               receiver_channel: Receiver<Packet>
    ) -> Self {
        Self {
            node_id,
            neighbors,
            sender_channels,
            receiver_channel,
            flood_ids: vec![],
            network: Graph::new(),
            fragment_reassembler: FragmentReassembler::new(),
            path: Self::new_path("/src/files")
        }
    }
    // Network discovery
    fn discover_network(&mut self) {
        let request = FloodRequest {
            flood_id: Self::generate_flood_id(),
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };
        let session_id = Self::generate_session_id();
        for &neighbor in &self.neighbors{
            self.sender_channels.get(&neighbor).expect("Didn't find neighbor").send(self.create_flood_request(request.clone(), neighbor,session_id)).expect("Error while sending the FloodRequest");
        }
    }
    // Forward FloodRequest to the neighbors
    fn forward_flood_request(&self, packet: Packet, previous: NodeId, request: FloodRequest) {
        //only one neighbor : the one which sent the FloodRequest. Then created FloodResponse and sent back
        if self.neighbors.iter().count() == 1{
            self.sender_channels.get(&previous).expect("Didn't find neighbor").send(self.create_flood_response(packet.session_id, request)).expect("Error while sending message");
        }
        //More than one neighbor
        else{
            for &neighbor in &self.neighbors {
                if neighbor != previous{
                    self.sender_channels.get(&neighbor).expect("Didn't find neighbor").send(packet.clone()).expect("Error while sending message");
                }
            }
        }
    }
    // Creation of packet with packet.type = FloodResponse
    fn create_flood_response(&self,session_id: u64, request: FloodRequest) -> Packet{
        let mut hops: Vec<NodeId> = vec![];
        for &e in &request.path_trace{
            hops.push(e.0.clone());
        }
        hops.reverse();
        let srh = SourceRoutingHeader::with_first_hop(hops);
        let flood_resp = FloodResponse{
            flood_id: request.flood_id,
            path_trace: request.path_trace.clone()
        };
        Packet::new_flood_response(srh,session_id,flood_resp)

    }
    // Creation of a packet of type FloodRequest
    fn create_flood_request(&self, request: FloodRequest, neighbor: NodeId,session_id: u64) -> Packet{
        let hops: Vec<NodeId> = vec![self.node_id,neighbor]; // TODO check if correct
        let srh = SourceRoutingHeader::with_first_hop(hops);
        Packet::new_flood_request(srh,session_id,request)
    }
    // Function to handle received packets of type FloodRequest
    pub fn handle_flood_request(&mut self,packet: Packet) {
        let packet_clone = packet.clone();
        match packet.pack_type{
            PacketType::FloodRequest(mut request)=>{
                let mut previous = 0;
                match request.path_trace.last(){
                    Some(last) => {
                        previous = last.0;
                        if self.flood_ids.contains(&(request.flood_id,request.initiator_id)){
                            request.path_trace.push((self.node_id.clone(),NodeType::Client));
                            let resp = self.create_flood_response(packet.session_id,request);
                            self.sender_channels.get(&previous).expect("Didn't find neighbor").send(resp).expect("Error while sending message");
                        }
                        else{
                            request.path_trace.push((self.node_id.clone(),NodeType::Client));
                            self.flood_ids.push((request.flood_id,request.initiator_id));
                            self.forward_flood_request(packet_clone,previous,request);
                        }
                    }
                    _ => {panic!("Can't find neighbour who sent this packet {} ", request);}
                }
            }
            _=>{panic!("Wrong packet type received")}
        }
    }

    // Handle received packets of type FloodResponse. Update knowledge of the network
    pub fn handle_flood_response(&mut self, packet: Packet) {
        match packet.pack_type{
            PacketType::FloodResponse(response) =>{
                self.update_graph(response);
            }
            _ => {panic!("Wrong packet type received")}
        }
    }
    // Handle received packets of type Ack
    /*
    pub fn handle_ack(&mut self,packet: Packet){
        //take sender of the Ack and remove it from packets field of Client
        //let sender_id:NodeId = packet.routing_header.hops[packet.routing_header.hops.len()-1].clone();
        self.sent_packets.remove(&(packet.session_id)).expect("Error: sender not found");

    }
    // Handle received packets of type Nack. Simply sends again the message to the dest_id
    pub fn handle_nack(&mut self, packet: Packet){
        match packet.pack_type{
            PacketType::Nack(nack) =>{
                match nack.nack_type {
                    _ => {
                        self.discover_network();
                        let original_packet = self.sent_packets.get(&packet.session_id).expect("Failed to retrieve original packet");
                        let original_dest = original_packet.routing_header.hops[original_packet.routing_header.hops.len()-1];
                        let new_packet = Packet{
                            routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,original_dest).unwrap()),
                            session_id: original_packet.session_id.clone(),
                            pack_type: original_packet.pack_type.clone(),
                        };
                        let neighbor = new_packet.routing_header.hops[0];
                        self.sender_channels.get(&neighbor).expect("Didn't find neighbor").send(new_packet).expect("Error while sending packet");
                    }
                }
            }
            _ =>{panic!("Wrong packet type received")}
        }

    }
     */
    // Handle received packets of type MsgFragment. Fragments put together and reassembled
    pub fn handle_msg_fragment(&mut self, packet: Packet){
        match packet.pack_type{
            PacketType::MsgFragment(fragment)=>{
                //check if a fragment with the same (session_id,src_id) has already been received
                match self.fragment_reassembler.add_fragment(packet.session_id,packet.routing_header.hops[0], fragment).expect("Error while processing fragment"){
                    Some(message) =>{
                        match FragmentReassembler::assemble_string_file(message,self.path.as_str()){ //TODO check the output path
                            Ok(msg) => {
                                let mut new_hops = packet.routing_header.hops.clone();
                                new_hops.reverse();
                                Packet::new_ack(
                                    SourceRoutingHeader::with_first_hop(new_hops),packet.session_id,0);
                            },
                            Err(e) => println!("Error: {e}")
                        }
                    }
                    None => ()
                }

            }

            _ =>{panic!("Wrong packet type received")}
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
    pub fn update_graph(&mut self, response: FloodResponse){ //TODO: check if value is effectively updated
        let path = &response.path_trace;

        // Iterate over consecutive pairs in the path_trace
        for window in path.windows(2) {
            if let [(node1, _type1), (node2, _type2)] = window {
                // Add node1 -> node2
                self.network.entry(*node1).or_insert_with(Vec::new).push(*node2);

                // Add node2 -> node1 (bidirectional edge)
                self.network.entry(*node2).or_insert_with(Vec::new).push(*node1);
            }
        }
    }
    // Calculates shortest path between two nodes. Complexity: O(V+E)
    fn bfs_compute_path(graph: &Graph, start: NodeId, end: NodeId) -> Option<Vec<NodeId>> {
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();
        let mut parent = HashMap::new();

        queue.push_back(start);
        visited.insert(start, true);

        while let Some(current) = queue.pop_front() {
            if current == end {
                // Reconstruct of the path
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
    // Send message (fragmented data) to a dest_id using bfs
    pub fn send_message(&mut self, dest_id: NodeId, data: &str) {
        let fragments = FragmentReassembler::generate_fragments(data).expect("Error while creating fragments");
        let session_id =  Self::generate_session_id();
        for fragment in fragments {
            if let Some(sender) = self.sender_channels.get(&dest_id) {
                let packet_sent = Packet {
                    routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,dest_id).unwrap()),
                    pack_type: PacketType::MsgFragment(fragment),
                    session_id
                };
                sender.send(packet_sent).unwrap();
                // After sending a fragment wait until an Ack returns back. If Nack received, proceed to send again the fragment with updated network and new route
                'internal: loop {
                    match self.receiver_channel.recv(){
                        Ok(packet) =>{
                            match packet.pack_type{
                                PacketType::Ack(_) => break 'internal,
                                PacketType::Nack(_) =>{
                                    //self.discover_network();
                                    let packet = Packet {
                                        routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,dest_id).unwrap()),
                                        pack_type: packet.pack_type,
                                        session_id
                                    };
                                    sender.send(packet.clone()).unwrap();
                                }
                                _=> ()
                            }
                        }
                        Err(e) => panic!("{e}")
                    }
                }
            }
        }
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
            /*
            PacketType::Nack(_) =>{
                self.handle_nack(packet);
            }
            PacketType::Ack(_) =>{
                self.handle_ack(packet);
            }

             */
        }
    }
    pub fn run(&mut self) {
        // Call FileSystem::new() and Client::new() from NetworkInitializer then client.run()
        loop {
            match self.receiver_channel.recv(){
                Ok(packet) =>{self.handle_packet(packet)},
                Err(error) =>{panic!("{error}")}
            }
        }
    }
}
fn main() {}
