use std::collections::HashSet;
use wg_2024::network::NodeId;

pub struct DroneStats {
    pub neigbours: HashSet<NodeId>,
    pub packets_forwarded: u32,
    pub packets_dropped: u32,
    pub flood_requests_forwarded: u32,
    pub flood_responses_forwarded: u32,
    pub acks_forwarded: u32,
    pub nacks_forwarded: u32,
    pub crashed: bool,
    pub pdr: f32,
}

impl DroneStats {
    pub fn new(neigbours: HashSet<NodeId>, pdr: f32) -> Self {
        Self {
            neigbours,
            packets_forwarded: 0,
            packets_dropped: 0,
            flood_requests_forwarded: 0,
            flood_responses_forwarded: 0,
            acks_forwarded: 0,
            nacks_forwarded: 0,
            crashed: false,
            pdr,
        }
    }
}

pub struct ClientStats {
    pub neigbours: HashSet<NodeId>,
    packets_sent: u32,
    packets_received: u32,
    acks_sent: u32,
}

impl ClientStats {
    pub fn new(neigbours: HashSet<NodeId>) -> Self {
        Self {
            neigbours,
            packets_sent: 0,
            packets_received: 0,
            acks_sent: 0,
        }
    }
}

pub struct ServerStats {
    neigbours: HashSet<NodeId>,
    packets_sent: u32,
    packets_received: u32,
    acks_sent: u32,
}

impl ServerStats {
    pub fn new(neigbours: HashSet<NodeId>) -> Self {
        Self {
            neigbours,
            packets_sent: 0,
            packets_received: 0,
            acks_sent: 0,
        }
    }
}
