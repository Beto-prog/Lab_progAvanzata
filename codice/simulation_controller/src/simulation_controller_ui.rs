use crate::node_stats::{ClientStats, DroneStats, ServerStats};
use eframe::egui;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wg_2024::network::NodeId;

#[derive(Default)]
pub struct SimulationControllerUI {
    drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,
    client_stats: Arc<Mutex<HashMap<NodeId, ClientStats>>>,
    server_stats: Arc<Mutex<HashMap<NodeId, ServerStats>>>,
}

impl SimulationControllerUI {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,
        client_stats: Arc<Mutex<HashMap<NodeId, ClientStats>>>,
        server_stats: Arc<Mutex<HashMap<NodeId, ServerStats>>>,
    ) -> Self {
        env_logger::init();

        Self {
            drone_stats,
            client_stats,
            server_stats,
        }
    }
}

impl eframe::App for SimulationControllerUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.label(&format!(
                "drone stats: {:?}",
                self.drone_stats.lock().unwrap().get(&1).unwrap().pdr,
            ));
        });
    }
}
