use std::collections::HashSet;
use wg_2024::network::NodeId;

#[derive(Debug, Clone, PartialEq)]
pub struct DroneStats {
    pub neigbours: HashSet<NodeId>,
    pub packets_forwarded: u32,
    pub packets_dropped: u32,
    pub fragments_forwarded: u32,
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
            fragments_forwarded: 0,
            flood_requests_forwarded: 0,
            flood_responses_forwarded: 0,
            acks_forwarded: 0,
            nacks_forwarded: 0,
            crashed: false,
            pdr,
        }
    }
}
