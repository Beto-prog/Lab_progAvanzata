#![allow(warnings)]
use super::config::NetworkConfig;
use super::validation::validate_config;
use client1::client1_ui::Client1_UI;
use client1::Client1;
use client2::Client2;
use crossbeam_channel::unbounded;
use simulation_controller::node_stats::DroneStats;
use simulation_controller::SimulationController;
use simulation_controller::SimulationControllerUI;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use wg_2024::network::NodeId;

use std::fs::{ File};
use std::io::Write;
use std::path::Path;

pub struct NetworkInitializer;

impl NetworkInitializer {
    pub fn read_config(file_path: &str) -> Result<NetworkConfig, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string(file_path)?;
        let config: NetworkConfig = toml::from_str(&config_str)?;
        Ok(config)
    }

    pub fn initialize_network(config: &NetworkConfig) {
        let (event_sender, event_receiver) = unbounded();
        let mut node_senders = HashMap::new();
        let mut node_receivers = HashMap::new();
        let mut drone_command_senders = HashMap::new();
        let mut drone_command_receivers = HashMap::new();
        let mut drone_stats = HashMap::new();

        // Initialize drones
        for drone_config in &config.drone {
            let (drone_send, drone_recv) = unbounded();
            let (command_send, command_recv) = unbounded();

            node_senders.insert(drone_config.id, drone_send.clone());
            node_receivers.insert(drone_config.id, drone_recv.clone());
            drone_command_senders.insert(drone_config.id, command_send.clone());
            drone_command_receivers.insert(drone_config.id, command_recv.clone());
        }

        // Initialize clients
        for client_config in &config.client {
            let (client_send, client_recv) = unbounded();
            node_senders.insert(client_config.id, client_send.clone());
            node_receivers.insert(client_config.id, client_recv.clone());
        }

        // Initialize servers
        for server_config in &config.server {
            let (server_send, server_recv) = unbounded();
            node_senders.insert(server_config.id, server_send.clone());
            node_receivers.insert(server_config.id, server_recv.clone());
        }

        for (index, drone_config) in config.drone.iter().enumerate() {
            let mut neighbor_senders = HashMap::new();
            for neighbor_id in &drone_config.connected_node_ids {
                neighbor_senders
                    .insert(*neighbor_id, node_senders.get(neighbor_id).unwrap().clone());
            }

            drone_stats.insert(
                drone_config.id,
                DroneStats::new(
                    HashSet::from_iter(drone_config.connected_node_ids.clone()),
                    drone_config.pdr,
                ),
            );

            let mut drone = simulation_controller::get_drone_impl::get_drone_impl(
                u8::try_from(index).unwrap(),
                drone_config.id,
                event_sender.clone(),
                drone_command_receivers
                    .get(&drone_config.id)
                    .unwrap()
                    .clone(),
                node_receivers.get(&drone_config.id).unwrap().clone(),
                neighbor_senders,
                drone_config.pdr,
            );

            thread::spawn(move || drone.run());
        }

        // Client initialization
        let (ui_snd, ui_rcv) = unbounded::<Client1_UI>();
        for (index, client_config) in config.client.iter().enumerate() {
            let mut neighbor_senders = HashMap::new();
            for neighbor_id in &client_config.connected_drone_ids {
                neighbor_senders
                    .insert(*neighbor_id, node_senders.get(neighbor_id).unwrap().clone());
            }

            let client_receiver = node_receivers.get(&client_config.id).unwrap().clone();

            if index % 2 == 0 {
                let mut client = Client1::new(
                    client_config.id,
                    neighbor_senders.clone(),
                    client_receiver,
                    Some(ui_snd.clone()),
                );
                thread::spawn(move || client.run());
            }
        }

        // Server initialization
        for (index, server_config) in config.server.iter().enumerate() {
            let mut neighbor_senders = HashMap::new();
            for neighbor_id in &server_config.connected_drone_ids {
                neighbor_senders
                    .insert(*neighbor_id, node_senders.get(neighbor_id).unwrap().clone());
            }

            let packet_receiver = node_receivers.get(&server_config.id).unwrap();
            let base_path = if cfg!(target_os = "windows") {
                "C:\\Temp\\ServerTxt"
            } else {
                "/tmp/ServerTxt"
            };
            let mut server = match index % 3 {
                1 => server::Server::new(
                    server_config.id,
                    packet_receiver.clone(),
                    neighbor_senders,
                    Box::new(server::file_system::ChatServer::new()),
                    None
                ),
                0 => {
                    Self::prepare_files(base_path);
                    server::Server::new(
                        server_config.id,
                        packet_receiver.clone(),
                        neighbor_senders,
                        Box::new(server::file_system::ContentServer::new(
                            base_path,
                            server::file_system::ServerType::TextServer,
                        )),
                        Some(base_path.to_string()),
                    )
                }
                _ => {
                    Self::prepare_files(base_path);
                    server::Server::new(
                        server_config.id,
                        packet_receiver.clone(),
                        neighbor_senders,
                        Box::new(server::file_system::ContentServer::new(
                            base_path,
                            server::file_system::ServerType::MediaServer,
                        )),
                        Some(base_path.to_string()),
                    )
                }
            };

            thread::spawn(move || server.run());
        }

        let network_topology = Self::get_network_topology(config);
        let drone_stats_arc = Arc::new(Mutex::new(drone_stats));
        let (ui_command_sender, ui_command_receiver) = unbounded();
        let (ui_response_sender, ui_response_receiver) = unbounded();

        let mut simulation_controller = SimulationController::new(
            drone_command_senders,
            node_senders,
            event_receiver,
            event_sender,
            network_topology,
            config.drone.iter().map(|c| c.id).collect(),
            config.client.iter().map(|c| c.id).collect(),
            config.server.iter().map(|c| c.id).collect(),
            u8::try_from(config.drone.len() % 10).unwrap(),
            drone_stats_arc.clone(),
            ui_command_receiver,
            ui_response_sender,
        );

        thread::spawn(move || simulation_controller.run());

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
            ..Default::default()
        };

        if let Err(error) = eframe::run_native(
            "Simulation Controller",
            native_options,
            Box::new(|cc| {
                Ok(Box::new(SimulationControllerUI::new(
                    cc,
                    drone_stats_arc.clone(),
                    ui_command_sender,
                    ui_response_receiver,
                    ui_rcv,
                )))
            }),
        ) {
            println!("Error: {}", error);
        }
    }
    fn prepare_files(base_path: &str) -> std::io::Result<()> {
        // Crea la directory se non esiste
        if !Path::new(base_path).exists() {
            fs::create_dir_all(base_path)?;
        }

        // Controlla se ci sono già file nella directory
        let entries = fs::read_dir(base_path)?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .count();

        if entries == 0 {
            // Se la cartella è vuota, crea alcuni file di esempio
            let sample_contents = vec!["123", "456", "789"];
            for (i, content) in sample_contents.iter().enumerate() {
                let file_path = format!("{}/text{}.txt", base_path, i + 1);
                let mut file = File::create(file_path)?;
                file.write_all(content.as_bytes())?;
            }
        }

        Ok(())
    }

    fn get_network_topology(config: &NetworkConfig) -> HashMap<NodeId, HashSet<NodeId>> {
        let mut topology = HashMap::new();

        for drone in &config.drone {
            topology.insert(drone.id, drone.connected_node_ids.iter().cloned().collect());
        }

        for client in &config.client {
            topology.insert(
                client.id,
                client.connected_drone_ids.iter().cloned().collect(),
            );
        }

        for server in &config.server {
            topology.insert(
                server.id,
                server.connected_drone_ids.iter().cloned().collect(),
            );
        }

        topology
    }

    pub fn run(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config = Self::read_config(file_path)?;
        validate_config(&config)?;
        Self::initialize_network(&config);
        Ok(())
    }
}
