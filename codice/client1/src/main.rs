use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::c_long;
use crossbeam_channel::{unbounded, Receiver, Sender};
use wg_2024::packet::*;
use wg_2024::network::*;

type Graph = HashMap<NodeId,Vec<NodeId>>;
pub struct Client {
    node_id: NodeId,
    neighbors: HashSet<NodeId>,
    sender_channels: HashMap<NodeId,Sender<Packet>>,
    receiver_channel: Receiver<Packet>,
    flood_ids: Vec<(u64,NodeId)>,
    network: Graph
}

impl Client {
    pub fn new(node_id: NodeId,neighbors: HashSet<NodeId>,sender_channels:HashMap<NodeId,Sender<Packet>>, receiver_channel: Receiver<Packet>) -> Self {
        Self {
            node_id,
            neighbors,
            sender_channels,
            receiver_channel,
            flood_ids: vec![],
            network: Graph::new()
        }
    }
    // Network discovery
    fn discover_network(&self) {
        let request = FloodRequest {
            flood_id: self.generate_flood_id(),
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };
        for &neighbor in &self.neighbors{
            self.sender_channels.get(&neighbor).send(self.create_flood_request(request.clone(),neighbor));
        }
    }
    //forward FloodRequest to the neighbors
    fn forward_flood_request(&self, packet: Packet, previous: NodeId, request: FloodRequest) {
        //only one neighbor : the one which sent the FloodRequest. Then created FloodResponse and sent back
        if self.neighbors.iter().count() == 1{
            self.sender_channels.get(&previous).send(self.create_flood_response(packet.session_id,request));
        }
        //more than one neighbor
        else{
            for &neighbor in &self.neighbors {
                if neighbor != previous{
                    self.sender_channels.get(&neighbor).send(packet.clone());
                }
            }
        }
    }
    //creation of packet with packet.type = FloodResponse
    fn create_flood_response(&self,session_id: u64, request: FloodRequest) -> Packet{
        let mut hops: Vec<NodeId> = vec![];
        for &e in &request.path_trace{
            hops.push(e.0.clone());
        }
        hops.reverse();
        let srh = SourceRoutingHeader::with_first_hop(hops); //TODO check if correct
        let flood_resp = FloodResponse{
            flood_id: request.flood_id,
            path_trace: request.path_trace.clone()
        };
        Packet::new_flood_response(srh,session_id,flood_resp)

    }
    //creation of packet with packet.type = FloodRequest
    fn create_flood_request(&self, request: FloodRequest, neighbor: NodeId) -> Packet{
        let hops: Vec<NodeId> = vec![self.node_id,neighbor]; // TODO check if correct
        let srh = SourceRoutingHeader::with_first_hop(hops);
        Packet::new_flood_request(srh,Self::generate_session_id(),request)
    }
    // Handle a received FloodRequest
    pub fn handle_flood_request(&self,packet: Packet) {
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
                            self.sender_channels.get(&previous).send(resp);
                        }
                        else{
                            request.path_trace.push((self.node_id.clone(),NodeType::Client));
                            self.forward_flood_request(packet_clone, previous,request);
                        }
                    }
                    _ => {panic!("Can't find neighbour who sent this packet {} ", request);}
                }
            }
            _=>()
        }
    }

    // Handle a received FloodResponse
    pub fn handle_flood_response(&self, packet: Packet) {
        match packet.pack_type{
            PacketType::FloodResponse(response) =>{

            }
            _ => {panic!("Error")}
            //TODO
        }
    }
    pub fn handle_ack(&mut self, ack: PacketType::Ack()){}
    pub fn handle_nack(&mut self,){}
    pub fn handle_msg_fragment(&mut self, msg_fragm: PacketType::MsgFragment()){}
    pub fn generate_flood_id() -> u64{
        rand::random()
    }
    pub fn generate_session_id() -> u64{
        rand::random()
    }
    pub fn update_graph(graph: &mut Graph, response: FloodResponse){
        let path = &response.path_trace;

        // Iterate over consecutive pairs in the path_trace
        for window in path.windows(2) {
            if let [(node1, _type1), (node2, _type2)] = window {
                // Add node1 -> node2
                graph.entry(*node1).or_insert_with(Vec::new).push(*node2);

                // Add node2 -> node1 (bidirectional edge)
                graph.entry(*node2).or_insert_with(Vec::new).push(*node1);
            }
        }
    }
    fn bfs_compute_path(graph: &HashMap<NodeId, Vec<NodeId>>, start: NodeId, end: NodeId) -> Option<Vec<NodeId>> {
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
    pub fn handle_packet(&mut self, packet: Packet){
        match packet.pack_type{
            Ok(PacketType::FloodRequest(_)) =>{
                self.handle_flood_request(packet);
            }
            Ok(PacketType::FloodResponse(_)) =>{
                self.handle_flood_response(packet);
            }
            Ok(PacketType::MsgFragment(msg_fragm)) =>{
                self.handle_msg_fragment(msg_fragm);
            }
            Ok(PacketType::Ack(ack)) =>{
                self.handle_ack(ack);
            }
            Ok(PacketType::Nack(nack)) =>{
                self.handle_nack(packet);
            }
            _ => {println!("Message not valid");} //TODO not sure about that
        }
    }
    pub fn run(&mut self) {
        //TODO UI part and command Sender part
        loop {
            match self.receiver_channel.recv(){
                Ok(packet) =>{self.handle_packet(packet)},
                Err(()) =>{} //TODO not sure about that
            }
        }
    }
}

fn main() {}