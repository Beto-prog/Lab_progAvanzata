#![allow(clippy::too_many_lines)]

use crate::forwarded_event::ForwardedEvent;
use crate::network_graph::NetworkGraph;
use crate::node_stats::DroneStats;
use crate::packet_animation::AnimationType;
use crate::ui_commands::{UICommand, UIResponse};
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use eframe::egui;
use egui::{Id, ScrollArea};
use std::collections::{HashMap, HashSet};
use wg_2024::network::NodeId;
use wg_2024::packet::{Packet, PacketType};

pub struct SimulationControllerUI {
    drone_stats: HashMap<NodeId, DroneStats>,
    selected_tab: usize,
    ui_command_sender: Sender<UICommand>,
    ui_response_receiver: Receiver<UIResponse>,
    forwarded_event_receiver: Receiver<ForwardedEvent>,
    new_pdr: HashMap<NodeId, f32>,
    selected_add_neighbour: HashMap<NodeId, NodeId>,
    selected_remove_neighbour: HashMap<NodeId, NodeId>,
    snackbar: Option<(String, f64)>,
    snackbar_duration: f64,
    network_graph: NetworkGraph,
    packet_debug: bool,
}

impl SimulationControllerUI {
    /// # Panics
    pub fn new(
        drone_stats: HashMap<NodeId, DroneStats>,
        ui_command_sender: Sender<UICommand>,
        ui_response_receiver: Receiver<UIResponse>,
        forwarded_event_receiver: Receiver<ForwardedEvent>,
        clients: Vec<NodeId>,
        servers: Vec<NodeId>,
    ) -> Self {
        env_logger::init();

        let mut new_pdr = HashMap::new();
        for (drone_id, drone) in drone_stats.iter() {
            new_pdr.insert(*drone_id, drone.pdr);
        }

        let mut selected_add_neighbour = HashMap::new();
        for drone_id in drone_stats.keys() {
            selected_add_neighbour.insert(*drone_id, 0);
        }

        let mut selected_remove_neighbour = HashMap::new();
        for drone_id in drone_stats.keys() {
            selected_remove_neighbour.insert(*drone_id, 0);
        }

        let drones = drone_stats.keys().cloned().collect::<Vec<_>>();

        let mut canonical_edges = HashSet::new();

        for (&u, drone) in &drone_stats {
            for &v in drone.neigbours.iter() {
                let start = u.min(v);
                let end = u.max(v);

                canonical_edges.insert((start, end));
            }
        }

        let edges = canonical_edges.into_iter().collect();

        Self {
            selected_tab: 0,
            drone_stats,
            ui_command_sender,
            ui_response_receiver,
            forwarded_event_receiver,
            new_pdr,
            selected_add_neighbour,
            selected_remove_neighbour,
            snackbar: None,
            snackbar_duration: 2.0,
            network_graph: NetworkGraph::new(drones, clients, servers, edges),
            packet_debug: false,
        }
    }

    fn handle_forwarded_event(&mut self, event: ForwardedEvent) {
        match event {
            ForwardedEvent::PacketSent(packet) => {
                let node_id = match packet.pack_type {
                    PacketType::MsgFragment(_) | PacketType::Ack(_) | PacketType::Nack(_) => {
                        let (start, dest) = Self::get_start_dest_from_packet(&packet);
                        let packet_id = (packet.get_fragment_index(), packet.session_id);
                        let animation_type = match packet.pack_type {
                            PacketType::MsgFragment(_) => AnimationType::Fragment,
                            PacketType::Ack(_) => AnimationType::Ack,
                            PacketType::Nack(_) => AnimationType::Nack,
                            _ => AnimationType::Fragment,
                        };
                        if packet.routing_header.hop_index == 2 {
                            let start = packet.routing_header.hops[0];
                            let end = packet.routing_header.hops[1];
                            self.network_graph.add_packet_animation(
                                packet_id,
                                start,
                                end,
                                animation_type,
                            );
                        }
                        self.network_graph.add_packet_animation(
                            packet_id,
                            start,
                            dest,
                            animation_type,
                        );
                        packet
                            .routing_header
                            .previous_hop()
                            .expect("there should always be a previous hop")
                    }
                    PacketType::FloodRequest(ref flood_request) => {
                        flood_request
                            .path_trace
                            .last()
                            .expect("Flood request should always have a last hop")
                            .0
                    }
                    PacketType::FloodResponse(ref flood_response) => {
                        flood_response
                            .path_trace
                            .last()
                            .expect("Flood response should always have a last hop")
                            .0
                    }
                };
                if let Some(stats) = self.drone_stats.get_mut(&node_id) {
                    stats.packets_forwarded += 1;
                    match packet.pack_type {
                        PacketType::MsgFragment(_) => stats.fragments_forwarded += 1,
                        PacketType::Ack(_) => stats.acks_forwarded += 1,
                        PacketType::Nack(_) => stats.nacks_forwarded += 1,
                        PacketType::FloodRequest(_) => stats.flood_requests_forwarded += 1,
                        PacketType::FloodResponse(_) => stats.flood_responses_forwarded += 1,
                    }
                }
                self.drone_stats
                    .get_mut(&node_id)
                    .expect("Drone should exist")
                    .packets_sent
                    .push(packet);
            }
            ForwardedEvent::PacketDropped(packet) => {
                let node_id = packet
                    .routing_header
                    .previous_hop()
                    .expect("Previous hop should always be valid");
                if let Some(stats) = self.drone_stats.get_mut(&node_id) {
                    stats.packets_dropped += 1;
                }
                self.drone_stats
                    .get_mut(&node_id)
                    .expect("Drone should exist")
                    .packets_sent
                    .push(packet);
            }
            ForwardedEvent::PDRSet(node_id, pdr) => {
                if let Some(stats) = self.drone_stats.get_mut(&node_id) {
                    stats.pdr = pdr;
                }
            }
            ForwardedEvent::DroneCrashed(node_id) => {
                if let Some(stats) = self.drone_stats.get_mut(&node_id) {
                    stats.crashed = true;
                }
                self.network_graph.crash_drone(node_id);
            }
            ForwardedEvent::ConnectionAdded(node1, node2) => {
                if let Some(stats) = self.drone_stats.get_mut(&node1) {
                    stats.neigbours.insert(node2);
                }
                if let Some(stats) = self.drone_stats.get_mut(&node2) {
                    stats.neigbours.insert(node1);
                }

                self.network_graph.add_connection(node1, node2);
            }
            ForwardedEvent::ConnectionRemoved(node1, node2) => {
                if let Some(stats) = self.drone_stats.get_mut(&node1) {
                    stats.neigbours.remove(&node2);
                }
                if let Some(stats) = self.drone_stats.get_mut(&node2) {
                    stats.neigbours.remove(&node1);
                }
                self.network_graph.remove_connection(node1, node2);
            }
        }
    }

    fn drone_stats_ui(&mut self, ui: &mut egui::Ui, drone_id: NodeId, now: f64) {
        let drone_stats = self
            .drone_stats
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
            if ui.button("Set PDR").clicked() {
                if drone_stats.crashed {
                    self.snackbar = Some((
                        "Can't send command to crashed drone".to_string(),
                        self.snackbar_duration + now,
                    ));
                } else {
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
                    let mut drones = self.drone_stats.keys().collect::<Vec<_>>();
                    drones.retain(|&e| !drone_stats.neigbours.contains(e) && !e.eq(&drone_id));
                    for drone in drones {
                        ui.selectable_value(selected, *drone, drone.to_string());
                    }
                });
            if ui.button("Add").clicked() && *selected != 0 {
                if drone_stats.crashed {
                    self.snackbar = Some((
                        "Can't send command to crashed drone".to_string(),
                        self.snackbar_duration + now,
                    ));
                } else {
                    self.ui_command_sender
                        .send(UICommand::AddConnection(drone_id, *selected))
                        .expect("Should be able to send the command");
                    *selected = 0;
                }
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
                    let drone_ids = self.drone_stats.keys().collect::<Vec<_>>();
                    let mut drones = drone_stats.neigbours.iter().collect::<Vec<_>>();
                    drones.retain(|&e| drone_ids.contains(&e) && !e.eq(&drone_id));

                    for drone in drones {
                        ui.selectable_value(selected, *drone, drone.to_string());
                    }
                });
            if ui.button("Remove").clicked() && *selected != 0 {
                if drone_stats.crashed {
                    self.snackbar = Some((
                        "Can't send command to crashed drone".to_string(),
                        self.snackbar_duration + now,
                    ));
                } else {
                    self.ui_command_sender
                        .send(UICommand::RemoveConnection(drone_id, *selected))
                        .expect("Should be able to send the command");
                    *selected = 0;
                }
            }
        });
        ui.separator();

        ui.checkbox(&mut self.packet_debug, "Toggle packet debug");

        if self.packet_debug {
            ui.separator();
            //Show received packets
            ScrollArea::vertical().show(ui, |ui| {
                for (index, item) in drone_stats.packets_sent.iter().enumerate() {
                    ui.label(format!("{}: {}", index, item));
                }
            });
        }
    }

    pub fn show_ui(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        // Handle any forwarded events
        while let Ok(event) = self.forwarded_event_receiver.try_recv() {
            self.handle_forwarded_event(event);
        }

        ui.separator();

        ui.horizontal(|ui| {
            if (ui.button("Visualization")).clicked() {
                self.selected_tab = 0;
            }
            for drone in self.drone_stats.keys() {
                if ui.button(format!("Drone {drone}")).clicked() {
                    self.selected_tab = *drone as usize;
                }
            }
        });

        let now = ctx.input(|i| i.time);

        if self.selected_tab == 0 {
            self.network_graph.show_ui(ui, now);
        } else {
            self.drone_stats_ui(
                ui,
                NodeId::try_from(self.selected_tab).expect("Should always be able to convert"),
                now,
            );
        }

        if let Some((ref message, expires)) = self.snackbar {
            if now < expires {
                // Draw the snackbar at the bottom center of the window.
                egui::Area::new(Id::new("snackbar"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -12.0))
                    .show(ctx, |ui| {
                        let frame = egui::Frame::NONE
                            .fill(egui::Color32::from_rgba_unmultiplied(50, 50, 50, 200))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(12, 8));
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
    }

    fn get_start_dest_from_packet(packet: &Packet) -> (NodeId, NodeId) {
        match packet.pack_type {
            PacketType::MsgFragment(_) | PacketType::Ack(_) | PacketType::Nack(_) => (
                packet
                    .routing_header
                    .previous_hop()
                    .expect("There should always be a previous hop"),
                packet
                    .routing_header
                    .current_hop()
                    .expect("There should always be a current hop"),
            ),
            _ => (0, 0),
        }
    }
}
