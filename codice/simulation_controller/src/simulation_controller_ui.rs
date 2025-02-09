use crate::node_stats::DroneStats;
use crate::ui_commands::UICommand;
use crossbeam_channel::Sender;
use eframe::egui;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wg_2024::network::NodeId;

pub struct SimulationControllerUI {
    drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,
    selected_tab: usize,
    ui_command_sender: Sender<UICommand>,
    new_pdr: HashMap<NodeId, f32>,
}

impl SimulationControllerUI {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,
        ui_command_sender: Sender<UICommand>,
    ) -> Self {
        env_logger::init();
        let selected_tab = drone_stats.lock().unwrap().keys().next().unwrap().clone() as usize;
        let mut new_pdr = HashMap::new();
        for drone in drone_stats.lock().unwrap().keys() {
            new_pdr.insert(*drone, 0.0);
        }

        Self {
            selected_tab: selected_tab as usize,
            drone_stats,
            ui_command_sender,
            new_pdr,
        }
    }

    fn drone_stats_ui(&mut self, ui: &mut egui::Ui, drone_id: NodeId) {
        let drone_stats = self.drone_stats.lock().unwrap();
        let drone_stats = drone_stats.get(&drone_id).unwrap();

        ui.label(&format!("Drone ID: {}", drone_id));
        ui.label(&format!("Neighbours: {:?}", drone_stats.neigbours));
        ui.label(&format!(
            "Packets forwarded: {}",
            drone_stats.packets_forwarded
        ));
        ui.label(&format!("Packets dropped: {}", drone_stats.packets_dropped));
        ui.label(&format!(
            "Flood requests forwarded: {}",
            drone_stats.flood_requests_forwarded
        ));
        ui.label(&format!(
            "Flood responses forwarded: {}",
            drone_stats.flood_responses_forwarded
        ));
        ui.label(&format!("ACKs forwarded: {}", drone_stats.acks_forwarded));
        ui.label(&format!("NACKs forwarded: {}", drone_stats.nacks_forwarded));
        ui.label(&format!("Crashed: {}", drone_stats.crashed));
        ui.label(&format!("PDR: {}", drone_stats.pdr));

        if ui.button("Crash").clicked() {
            if !drone_stats.crashed {
                self.ui_command_sender
                    .send(UICommand::CrashDrone(drone_id))
                    .unwrap();
            }
        }

        ui.add(egui::Slider::new(self.new_pdr.get_mut(&drone_id).unwrap(), 0.0..=1.0).text("PDR"));

        if ui.button("Set PDR").clicked() {
            if !drone_stats.crashed {
                self.ui_command_sender
                    .send(UICommand::SetPDR(
                        drone_id,
                        self.new_pdr.get(&drone_id).unwrap().clone(),
                    ))
                    .unwrap();
            }
        }
    }
}

impl eframe::App for SimulationControllerUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Simulation Controller");
            ui.horizontal(|ui| {
                for drone in self.drone_stats.lock().unwrap().keys() {
                    if ui.button(&format!("Drone {}", drone)).clicked() {
                        self.selected_tab = *drone as usize;
                    }
                }
            });
            self.drone_stats_ui(ui, self.selected_tab as NodeId);

            // Show content based on the selected tab
        });
    }
}
