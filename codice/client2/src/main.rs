use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// Define types and data structures
type NodeId = u8;

#[derive(Clone, Debug)]
pub enum NodeType {
    Client,
    Drone,
    Server,
}

#[derive(Debug)]
pub struct FloodRequest {
    flood_id: u64,
    initiator_id: NodeId,
    path_trace: Vec<(NodeId, NodeType)>,
}

#[derive(Debug)]
pub struct FloodResponse {
    flood_id: u64,
    path_trace: Vec<(NodeId, NodeType)>,
}

#[derive(Debug)]
pub struct SourceRoutingHeader {
    hop_index: usize,
    hops: Vec<NodeId>,
}

#[derive(Debug)]
pub enum PacketType {
    MsgFragment(Fragment),
    Ack(Ack),
    Nack(Nack),
    FloodRequest(FloodRequest),
    FloodResponse(FloodResponse),
}

#[derive(Debug)]
pub struct Packet {
    pack_type: PacketType,
    routing_header: SourceRoutingHeader,
    session_id: u64,
}

#[derive(Debug)]
pub struct Fragment {
    fragment_index: u64,
    total_n_fragments: u64,
    length: u8,
    data: [u8; 128],
}

#[derive(Debug)]
pub struct Ack {
    fragment_index: u64,
}

#[derive(Debug)]
pub struct Nack {
    fragment_index: u64,
    nack_type: NackType,
}

#[derive(Debug)]
pub enum NackType {
    ErrorInRouting(NodeId),
    DestinationIsDrone,
    Dropped,
    UnexpectedRecipient(NodeId),
}

// Client struct
pub struct Client {
    node_id: NodeId,
    discovered_nodes: Arc<Mutex<HashMap<NodeId, NodeType>>>,
    unacknowledged_packets: Arc<Mutex<HashSet<u64>>>, // Track unacknowledged fragments
}

impl Client {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            discovered_nodes: Arc::new(Mutex::new(HashMap::new())),
            unacknowledged_packets: Arc::new(Mutex::new(Default::default())),
        }
    }

    // Network discovery protocol
    pub fn discover_network(&self) {
        let flood_id = self.generate_flood_id();
        let request = FloodRequest {
            flood_id,
            initiator_id: self.node_id,
            path_trace: vec![(self.node_id, NodeType::Client)],
        };

        self.send_flood_request(request);
    }

    fn send_flood_request(&self, request: FloodRequest) {
        println!("Sending flood request: {:?}", request);

        // Simulate sending to neighbors (this part needs integration with drones)
        // Example: Iterate over discovered neighbors and forward the request
        let mut nodes = self.discovered_nodes.lock().unwrap();
        for (neighbor_id, _) in nodes.iter() {
            println!("Forwarding FloodRequest to Node {}", neighbor_id);
            // Simulate forwarding
        }
    }

    // Handle a received FloodRequest
    pub fn handle_flood_request(&self, request: FloodRequest) -> Option<FloodResponse> {
        let mut nodes = self.discovered_nodes.lock().unwrap();

        // Check for duplicate flood_id and initiator_id
        if nodes.contains_key(&request.initiator_id) {
            println!("Duplicate FloodRequest ignored: {:?}", request);
            return None;
        }

        // Add this node to the path trace
        let mut new_path_trace = request.path_trace.clone();
        new_path_trace.push((self.node_id, NodeType::Client));

        // Forward to neighbors if possible
        if !nodes.is_empty() {
            self.send_flood_request(FloodRequest {
                flood_id: request.flood_id,
                initiator_id: request.initiator_id,
                path_trace: new_path_trace.clone(),
            });
        }

        // If no neighbors left, generate a response
        Some(FloodResponse {
            flood_id: request.flood_id,
            path_trace: new_path_trace,
        })
    }

    // Handle a received FloodResponse
    pub fn handle_flood_response(&self, response: FloodResponse) {
        println!("Received flood response: {:?}", response);
        let mut nodes = self.discovered_nodes.lock().unwrap();
        for (node_id, node_type) in response.path_trace {
            nodes.insert(node_id, node_type);
        }
    }

    // Send a message with retry logic
    pub fn send_message(&self, destination: NodeId, message: Vec<u8>) {
        let session_id = self.generate_session_id();
        let total_n_fragments = (message.len() as u64 + 127) / 128;

        for (i, chunk) in message.chunks(128).enumerate() {
            let mut data = [0u8; 128];
            data[..chunk.len()].copy_from_slice(chunk);

            let fragment = Fragment {
                fragment_index: i as u64,
                total_n_fragments,
                length: chunk.len() as u8,
                data,
            };

            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header: self.create_source_routing_header(destination),
                session_id,
            };

            self.send_packet_with_retry(packet);
        }
    }

    // Send a packet with retry logic
    fn send_packet_with_retry(&self, packet: Packet) {
        let session_id = packet.session_id;
        self.unacknowledged_packets.lock().unwrap().insert(session_id);

        println!("Sending packet: {:?}", packet);

        // Simulate sending the packet (replace with actual networking code)
        // After sending, start a timer to retry if no Ack is received
        let retry_interval = std::time::Duration::from_secs(2);

        // Simulate retry logic (this could be implemented as an async task)
        std::thread::spawn({
            let unacknowledged = self.unacknowledged_packets.clone();
            move || {
                std::thread::sleep(retry_interval);
                if unacknowledged.lock().unwrap().contains(&session_id) {
                    println!("Retrying packet: {:?}", packet);
                    // Resend the packet
                }
            }
        });
    }

    // Handle Ack
    pub fn handle_ack(&self, ack: Ack) {
        println!("Ack received for fragment: {}", ack.fragment_index);
        self.unacknowledged_packets.lock().unwrap().remove(&ack.fragment_index);
    }

    // Handle Nack
    pub fn handle_nack(&self, nack: Nack) {
        println!("Nack received: {:?}", nack);

        match nack.nack_type {
            NackType::Dropped => {
                println!("Resending dropped packet...");
                // Retry sending the packet here
            }
            _ => println!("Unhandled Nack type."),
        }
    }

    fn send_packet(&self, packet: Packet) {
        println!("Sending packet: {:?}", packet);
        // Simulate sending the packet over the network
    }

    fn create_source_routing_header(&self, destination: NodeId) -> SourceRoutingHeader {
        // Placeholder route calculation
        let hops = vec![self.node_id, destination];
        SourceRoutingHeader { hop_index: 1, hops }
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
    let client = Client::new(1);
    client.discover_network();
    //client.send_message(2, "Hello, Server!".to_vec());

    //Send a message to a server
    // client.request_file_list(2);
    //
    //Simulate receiving a response
    // client.handle_high_level_response(b"files_list!file1,file2,file3".to_vec());
}
