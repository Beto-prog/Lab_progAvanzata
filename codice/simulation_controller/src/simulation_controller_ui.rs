#![allow(clippy::too_many_lines)]

use crate::node_stats::DroneStats;
use crate::ui_commands::{UICommand, UIResponse};
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use eframe::egui;
use egui::Id;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wg_2024::network::NodeId;

pub struct SimulationControllerUI {
    drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,
    selected_tab: usize,
    ui_command_sender: Sender<UICommand>,
    ui_response_receiver: Receiver<UIResponse>,
    new_pdr: HashMap<NodeId, f32>,
    selected_add_neighbour: HashMap<NodeId, NodeId>,
    selected_remove_neighbour: HashMap<NodeId, NodeId>,
    snackbar: Option<(String, f64)>,
    snackbar_duration: f64,
    client1_selected: bool,
    client2_selected: bool,
}

impl SimulationControllerUI {
    /// # Panics
    pub fn new(
        drone_stats: Arc<Mutex<HashMap<NodeId, DroneStats>>>,
        ui_command_sender: Sender<UICommand>,
        ui_response_receiver: Receiver<UIResponse>,
    ) -> Self {
        env_logger::init();
        let selected_tab = *drone_stats
            .lock()
            .expect("Should be able to unlock")
            .keys()
            .next()
            .expect("There should always be at least one drone")
            as usize;
        let mut new_pdr = HashMap::new();
        for (drone_id, drone) in drone_stats.lock().expect("Should be able to unlock").iter() {
            new_pdr.insert(*drone_id, drone.pdr);
        }

        let mut selected_add_neighbour = HashMap::new();
        for drone_id in drone_stats.lock().expect("Should be able to unlock").keys() {
            selected_add_neighbour.insert(*drone_id, 0);
        }

        let mut selected_remove_neighbour = HashMap::new();
        for drone_id in drone_stats.lock().expect("Should be able to unlock").keys() {
            selected_remove_neighbour.insert(*drone_id, 0);
        }
        Self {
            selected_tab,
            drone_stats,
            ui_command_sender,
            ui_response_receiver,
            new_pdr,
            selected_add_neighbour,
            selected_remove_neighbour,
            snackbar: None,
            snackbar_duration: 2.0,
            client1_selected: false,
            client2_selected: false,
        }
    }

    fn drone_stats_ui(&mut self, ui: &mut egui::Ui, drone_id: NodeId, now: f64) {
        let general_drone_stats = self.drone_stats.lock().expect("Should be able to unlock");
        let drone_stats = general_drone_stats
            .get(&drone_id)
            .expect("Should be able to get the drone");

        ui.separator();

        ui.label(format!("Drone ID: {drone_id}",));
        ui.label(format!("Neighbours: {:?}", drone_stats.neigbours));
        ui.label(format!(
            "Packets forwarded: {}",
            drone_stats.packets_forwarded
        ));
        ui.label(format!("Packets dropped: {}", drone_stats.packets_dropped));
        ui.label(format!(
            "Flood requests forwarded: {}",
            drone_stats.flood_requests_forwarded
        ));
        ui.label(format!(
            "Flood responses forwarded: {}",
            drone_stats.flood_responses_forwarded
        ));
        ui.label(format!("ACKs forwarded: {}", drone_stats.acks_forwarded));
        ui.label(format!("NACKs forwarded: {}", drone_stats.nacks_forwarded));
        ui.label(format!("Crashed: {}", drone_stats.crashed));
        ui.label(format!("PDR: {}", drone_stats.pdr));
        ui.separator();

        if ui.button("Crash").clicked() {
            if drone_stats.crashed {
                self.snackbar = Some((
                    "Drone already crashed".to_string(),
                    self.snackbar_duration + now,
                ));
            } else {
                self.ui_command_sender
                    .send(UICommand::CrashDrone(drone_id))
                    .expect("Should be able to send the command");
            }
        }
        ui.separator();

        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(
                    self.new_pdr
                        .get_mut(&drone_id)
                        .expect("Should be able to get the PDR"),
                    0.0..=1.0,
                )
                .text("PDR"),
            );
            if ui.button("Set PDR").clicked() && !drone_stats.crashed {
                self.ui_command_sender
                    .send(UICommand::SetPDR(
                        drone_id,
                        *self
                            .new_pdr
                            .get(&drone_id)
                            .expect("Should be able to get the PDR"),
                    ))
                    .expect("Should be able to send the command");
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            let selected = self
                .selected_add_neighbour
                .get_mut(&drone_id)
                .expect("Should be able to get the selected neighbour");
            ui.label("Add neighbor: ");

            egui::ComboBox::new(0, "")
                .selected_text(format!("{selected}"))
                .show_ui(ui, |ui| {
                    let mut drones = general_drone_stats.keys().collect::<Vec<_>>();
                    drones.retain(|&e| !drone_stats.neigbours.contains(e) && !e.eq(&drone_id));
                    for drone in drones {
                        ui.selectable_value(selected, *drone, drone.to_string());
                    }
                });
            if ui.button("Add").clicked() && *selected != 0 && !drone_stats.crashed {
                self.ui_command_sender
                    .send(UICommand::AddConnection(drone_id, *selected))
                    .expect("Should be able to send the command");
                *selected = 0;
            }
        });

        ui.horizontal(|ui| {
            let selected = self
                .selected_remove_neighbour
                .get_mut(&drone_id)
                .expect("Should be able to get the selected neighbour");
            ui.label("Remove neighbor: ");

            egui::ComboBox::new(1, "")
                .selected_text(format!("{selected}"))
                .show_ui(ui, |ui| {
                    let drone_ids = general_drone_stats.keys().collect::<Vec<_>>();
                    let mut drones = drone_stats.neigbours.iter().collect::<Vec<_>>();
                    drones.retain(|&e| drone_ids.contains(&e) && !e.eq(&drone_id));

                    for drone in drones {
                        ui.selectable_value(selected, *drone, drone.to_string());
                    }
                });
            if ui.button("Remove").clicked() && *selected != 0 && !drone_stats.crashed {
                self.ui_command_sender
                    .send(UICommand::RemoveConnection(drone_id, *selected))
                    .expect("Should be able to send the command");
                *selected = 0;
            }
        });
    }

    pub fn show_ui(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Simulation Controller");
            ui.separator();
            ui.horizontal(|ui| {
                for drone in self
                    .drone_stats
                    .lock()
                    .expect("Should be able to unlock")
                    .keys()
                {
                    if ui.button(format!("Drone {drone}")).clicked() {
                        self.selected_tab = *drone as usize;
                        self.client1_selected = false;
                        self.client2_selected = false;
                    }
                }
            });

            let now = ctx.input(|i| i.time);

            self.drone_stats_ui(
                ui,
                NodeId::try_from(self.selected_tab).expect("Should always be able to convert"),
                now,
            );
            if let Some((ref message, expires)) = self.snackbar {
                if now < expires {
                    // Draw the snackbar at the bottom center of the window.
                    egui::Area::new(Id::new("snackbar"))
                        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -12.0))
                        .show(ctx, |ui| {
                            let frame = egui::Frame::none()
                                .fill(egui::Color32::from_rgba_unmultiplied(50, 50, 50, 200))
                                .rounding(egui::Rounding::same(8.0))
                                .inner_margin(egui::Margin::symmetric(12.0, 8.0));
                            frame.show(ui, |ui| {
                                ui.label(egui::RichText::new(message).size(28.0));
                            });
                        });
                } else {
                    // Remove the snackbar when its time expires.
                    self.snackbar = None;
                }
            }

            if let Ok(response) = self.ui_response_receiver.try_recv() {
                match response {
                    UIResponse::Success(message) | UIResponse::Falure(message) => {
                        self.snackbar = Some((message, self.snackbar_duration + now));
                    }
                }
            }
        });
    }
}
