use std::collections::{HashMap, HashSet};
use crossbeam_channel::{unbounded, Receiver, Sender};
use wg_2024::packet::*;
use wg_2024::network::*;


// Client struct
pub struct Client {
    node_id: NodeId,
    neighbors: HashSet<NodeId>,
    sender_channels: HashMap<NodeId, Sender<Packet>>,
    receiver_channel: Receiver<Packet>,
}

impl Client {
    pub fn new(node_id: NodeId,neighbors: HashSet<NodeId>, receiver_channel: Receiver<Packet>) -> Self {
        Self {
            node_id,
            neighbors,
            sender_channels: HashMap::new(),
            receiver_channel,
        }
    }
    // Network discovery protocol
    fn discover_network(&self) {
        let request = FloodRequest {
            flood_id: self.generate_flood_id(),
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };
        self.send_flood_request(request);
    }

    fn send_flood_request(&self, request: FloodRequest) {
        for &neighbor in &self.neighbors {
            if let Some(sender) = self.sender_channels.get(&neighbor) {
                sender.send(Packet {
                    routing_header: SourceRoutingHeader {
                        hop_index: 0,
                        hops: vec![self.node_id, neighbor],
                    },
                    pack_type: PacketType::FloodRequest(request.clone()),
                    session_id: request.flood_id,
                }).unwrap();
            }
        }
    }
    // Handle a received FloodRequest
    pub fn handle_flood_request(&self, request: FloodRequest) -> Option<FloodResponse> {

    }

    // Handle a received FloodResponse
    pub fn handle_flood_response(&self, response: FloodResponse) {

    }

    // Send a message with retry logic
    pub fn send_message(&self, destination: NodeId, message: Vec<u8>) {

    }

    // Send a packet with retry logic
    fn send_packet_with_retry(&self, packet: Packet) {

    }

    // Handle Ack
    pub fn handle_ack(&self, ack: Ack) {

    }

    // Handle Nack
    pub fn handle_nack(&self, nack: Nack) {

    }

    fn send_packet(&self, packet: Packet) {

    }

    fn create_source_routing_header(&self, destination: NodeId) -> SourceRoutingHeader {
        let mut srh = SourceRoutingHeader{
            hop_index: 1,
            hops: vec![self.node_id,destination]
        };
        todo!();
        srh


    }
    fn run(&self){

    }

    // Helpers
    fn generate_flood_id(&self) -> u64 {
        // Generate a unique flood ID
        rand::random()
    }

    fn generate_session_id(&self) -> u64 {
        // Generate a unique session ID
        rand::random()
    }
}

fn main() {
    //TODO
}

