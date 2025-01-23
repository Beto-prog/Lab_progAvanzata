use std::collections::HashMap;

use crossbeam_channel::{Receiver, Sender};
use wg_2024::{
    controller::{DroneCommand, DroneEvent},
    network::NodeId,
};

// Placeholder for simulation controller while we wait
pub struct SimulationController {
    event_recv: Receiver<DroneEvent>,
    drones_command_send: HashMap<NodeId, Sender<DroneCommand>>,
    drone_ids: Vec<NodeId>,
    client_ids: Vec<NodeId>,
    server_ids: Vec<NodeId>,
}

impl SimulationController {
    pub fn new(
        event_recv: Receiver<DroneEvent>,
        drones_command_send: HashMap<NodeId, Sender<DroneCommand>>,
        drone_ids: Vec<NodeId>,
        client_ids: Vec<NodeId>,
        server_ids: Vec<NodeId>,
    ) -> Self {
        Self {
            event_recv,
            drones_command_send,
            drone_ids,
            client_ids,
            server_ids,
        }
    }

    pub fn run(&self) {
        loop {}
    }
}
