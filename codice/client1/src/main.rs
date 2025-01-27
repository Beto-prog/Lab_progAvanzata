use std::collections::{HashMap, HashSet};
use std::ffi::c_long;
use crossbeam_channel::{unbounded, Receiver, Sender};
use wg_2024::packet::*;
use wg_2024::network::*;

pub struct Client {
    node_id: NodeId,
    neighbors: HashSet<NodeId>,
    sender_channels: HashMap<NodeId,Sender<Packet>>,
    receiver_channel: Receiver<Packet>,
    flood_ids: Vec<(u64,NodeId)>
}

impl Client {
    pub fn new(node_id: NodeId,neighbors: HashSet<NodeId>,sender_channels:HashMap<NodeId,Sender<Packet>>, receiver_channel: Receiver<Packet>,flood_ids: Vec<(u64,NodeId)>) -> Self {
        Self {
            node_id,
            neighbors,
            sender_channels,
            receiver_channel,
            flood_ids
        }
    }
    // Network discovery protocol
    fn discover_network(&self) {
        let request = FloodRequest {
            flood_id: self.generate_flood_id(),
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };
        // TODO self.send_flood_request(request, );
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
    pub fn handle_flood_response(&self, response: FloodResponse) {

    }
    pub fn handle_ack(&mut self, ack: PacketType::Ack()){}
    pub fn handle_nack(&mut self,nack: PacketType::Nack()){}
    pub fn handle_msg_fragment(&mut self, msg_fragm: PacketType::MsgFragment()){}
    pub fn generate_flood_id() -> u64{
        rand::random()
    }
    pub fn handle_packet(&mut self, packet: Packet){
        match packet.pack_type{
            Ok(PacketType::FloodRequest(_)) =>{
                self.handle_flood_request(packet);
            }
            Ok(PacketType::FloodResponse(flood_resp)) =>{
                self.handle_flood_response(flood_resp);
            }
            Ok(PacketType::MsgFragment(msg_fragm)) =>{
                self.handle_msg_fragment(msg_fragm);
            }
            Ok(PacketType::Ack(ack)) =>{
                self.handle_ack(ack);
            }
            Ok(PacketType::Nack(nack)) =>{
                self.handle_nack(nack);
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