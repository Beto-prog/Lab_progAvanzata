use std::collections::{HashMap, HashSet};
use crossbeam_channel::{unbounded, Receiver, Sender};
use wg_2024::packet::*;
use wg_2024::network::*;

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
    pub fn handle_packet(&self){}

    // Send a message with retry logic

    fn create_source_routing_header(&self, destination: NodeId) -> SourceRoutingHeader {
        let mut srh = SourceRoutingHeader{
            hop_index: 1,
            hops: vec![self.node_id,destination]
        };
        todo!();
        srh
    }
    pub fn run(&self){

        loop{
            self.handle_flood_response();
            self.handle_flood_request();
            self.handle_packet();

        }
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

fn main() {}