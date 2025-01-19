use crossbeam_channel::{Receiver, Sender};
use wg_2024::controller::{DroneCommand, DroneEvent};

pub struct SimulationController {
    event_recv: Receiver<DroneEvent>,
    command_send: Sender<DroneCommand>,
}

impl SimulationController {
    pub fn new(event_recv: Receiver<DroneEvent>, command_send: Sender<DroneCommand>) -> Self {
        Self {
            event_recv,
            command_send,
        }
    }

    pub fn run(&self) {
        loop {}
    }
}
