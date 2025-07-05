use base64::{engine::general_purpose, Engine as _};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{ColorImage, TextureHandle};
use eframe::epaint::{Stroke, Vec2};
use egui::{Color32, Context, Frame, RichText, TextEdit, TextStyle};
use image::GenericImageView;
#[allow(warnings)]
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use wg_2024::network::NodeId;

use common::client_ui::ClientUI;

pub struct Client2_UI {
    self_id: NodeId,                              // Node ID for the client
    clients: Arc<Mutex<Vec<NodeId>>>,             // List of clients
    servers: Arc<RwLock<HashMap<NodeId, String>>>, // List of server names
    files_names: Arc<Mutex<Vec<String>>>,         //Storage of the file names
    selected_server: (NodeId, String),            // Selected server name
    selected_client_id: NodeId,                   // Selected client ID
    selected_command: String,                     // Selected command
    selected_content_id: String,                  // Selected file ID
    input_text: String,                           // Input text from the user (in message_for cmd)
    cmd_snd: Option<Sender<String>>,              // Command sender
    msg_rcv: Option<Receiver<String>>,            // Message receiver
    can_show_clients: bool,                       //Response handle
    can_show_response: bool,                      //Response handle
    can_show_file_list: bool,                     //Response handle
    response: String,                             //Response txt
    image: Option<TextureHandle>,                 //Response image
    error: (bool, String),                        //Error check
    communication_server_commands: Vec<String>,   //Commands
    text_server_commands: Vec<String>,            //Commands
    media_server_commands: Vec<String>,           //Commands
    log_messages: Vec<(String, bool)>,            // (message, is_outgoing)
}

impl ClientUI for Client2_UI {
    fn show_ui(&mut self, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        self.client2_stats(ui);
    }

    fn get_viewport_id(&self) -> u64 {
        self.self_id.into()
    }
}

impl Client2_UI {
    pub fn new(
        self_id: NodeId,
        clients: Arc<Mutex<Vec<NodeId>>>,
        servers: Arc<RwLock<HashMap<NodeId, String>>>,
        files_names: Arc<Mutex<Vec<String>>>,
        cmd_snd: Sender<String>,
        msg_rcv: Receiver<String>,
    ) -> Self {
        Self {
            self_id,
            clients,
            servers,
            files_names,
            selected_server: (0, String::new()),
            selected_client_id: 0,
            selected_command: String::from("Select command"),
            selected_content_id: String::new(),
            input_text: String::new(),
            cmd_snd: Some(cmd_snd),
            msg_rcv: Some(msg_rcv),
            can_show_clients: false,
            can_show_response: false,
            can_show_file_list: false,
            response: String::new(),
            image: None,
            error: (false, String::new()),
            text_server_commands: vec!["file?".to_string(), "files_list?".to_string()],
            media_server_commands: vec!["media?".to_string(), "files_list?".to_string()],
            communication_server_commands: vec![
                "message_for?".to_string(),
                "client_list?".to_string(),
            ],
            log_messages: Vec::new(),
        }
    }

    fn add_log_message(&mut self, message: String, is_outgoing: bool) {
        self.log_messages.push((message, is_outgoing));
    }

    pub fn client2_stats(&mut self, ui: &mut egui::Ui) {

        let servers = {
            let client_servers = self.servers.read().unwrap();
            client_servers.clone() // Clone while holding the lock
        };

        // Update local server selection if needed
        if !servers.contains_key(&self.selected_server.0) {
            self.selected_server = (0, String::new());
        }

        // Handle incoming messages
        while let Ok(msg) = self.msg_rcv.as_ref().unwrap().try_recv() {

            if msg == "REFRESH_UI" {
                // Short-circuit the loop to force a re-render
                //println!("REFRESH_UI");
                break;
            }


            if msg.starts_with("Message sent to client") {
                continue;
            }

            if !self.log_messages.iter().any(|(m, _)| m == &format!("received {}", msg)) {
                self.add_log_message(format!("received {}", msg), false);
                self.can_show_response = true;
                self.response = msg;
            }
        }

        egui::SidePanel::left("command_panel")
            .resizable(true)
            .default_width(300.0)
            .show_inside(ui, |ui| {
                ui.heading(RichText::new("Commands").color(Color32::GREEN));
                ui.add_space(10.0);

                egui::Grid::new("server_grid").spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Server:");
                    egui::ComboBox::new("Select server", "")
                        .selected_text(format!("{}", self.selected_server.0))
                        .show_ui(ui, |ui| {
                            let s = self.servers.read().expect("Failed to lock");
                            let servers = s.keys().collect::<Vec<_>>();
                            for server in servers {
                                if ui.selectable_value(
                                    &mut self.selected_server.0,
                                    *server,
                                    server.to_string(),
                                ).clicked() {
                                    // Reset command when server changes
                                    self.selected_command = "Select command".to_string();
                                    self.selected_client_id = 0;
                                    self.selected_content_id = "".to_string();
                                    self.input_text.clear();
                                }
                            }
                        });
                    ui.end_row();

                    if self.selected_server.0 != 0 {
                        let server_type = self
                            .servers
                            .read()
                            .expect("Failed to lock")
                            .get(&self.selected_server.0)
                            .cloned()
                            .unwrap_or_default();
                        self.selected_server.1 = server_type.clone();
                        ui.label("Type:");
                        ui.label(RichText::new(format!("{}", server_type)).color(Color32::RED));
                        ui.end_row();
                    }

                    if self.selected_server.0 != 0 {
                        ui.label("Command:");
                        let command_list = match self.selected_server.1.as_str() {
                            "CommunicationServer" => &self.communication_server_commands,
                            "TextServer" => &self.text_server_commands,
                            "MediaServer" => &self.media_server_commands,
                            _ => &vec![],
                        };
                        egui::ComboBox::new("Select command", "")
                            .selected_text(format!("{}", self.selected_command))
                            .show_ui(ui, |ui| {
                                for command in command_list {
                                    ui.selectable_value(
                                        &mut self.selected_command,
                                        command.clone(),
                                        command.clone(),
                                    );
                                }
                            });
                        ui.end_row();
                    }
                });

                ui.add_space(10.0);
                match self.selected_command.as_str() {
                    "message_for?" => {
                        if !self.clients.lock().expect("Failed to lock").is_empty() {
                            ui.collapsing("Message for Client", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Message:");
                                    ui.add(TextEdit::singleline(&mut self.input_text).desired_width(150.0));
                                    ui.label("Client:");
                                    egui::ComboBox::new("Select client", "")
                                        .selected_text(format!("{}", self.selected_client_id))
                                        .show_ui(ui, |ui| {
                                            let clients = self.clients.lock().expect("Failed to lock");
                                            for cl in clients.iter() {
                                                ui.selectable_value(&mut self.selected_client_id, *cl, cl.to_string());
                                            }
                                        });
                                });
                            });
                        } else {
                            self.error = (true, "No clients available.\nCheck for available clients first".to_string());
                        }
                    }
                    "file?" | "media?" => {
                        let label = if self.selected_command == "file?" { "File" } else { "Media" };
                        if !self.files_names.lock().expect("Failed to lock").is_empty() {
                            ui.collapsing(label, |ui| {
                                let files = self.files_names.lock().expect("Failed to lock");
                                egui::ComboBox::new("Select content", "")
                                    .selected_text(format!("{}", self.selected_content_id))
                                    .show_ui(ui, |ui| {
                                        for id in files.iter() {
                                            ui.selectable_value(&mut self.selected_content_id, (*id).clone(), id.to_string());
                                        }
                                    });
                            });
                        } else {
                            self.error = (true, format!("No content available.\nCheck for available {} first", label.to_lowercase()));
                        }
                    }
                    _ => {}
                }

                ui.add_space(20.0);
                if self.selected_server.0 != 0 && self.selected_command != "Select command" && !self.error.0 {
                    ui.horizontal(|ui| {
                        if ui.add_sized([100.0, 40.0], egui::Button::new(RichText::new("\u{1F680} SEND").size(20.0))).clicked() {
                            let cmd = self.create_command();
                            self.handle_response_show(cmd);
                        }
                    });
                }

                ui.separator();
                ui.label(RichText::new("Summary").color(Color32::GREEN));
                ui.add_space(5.0);
                ui.label(format!("Client ID: {}", self.self_id));
                ui.label(format!("Server ID: {}", self.selected_server.0));
                if self.selected_command == "message_for?" && self.selected_client_id != 0 {
                    ui.label(format!("Other client ID: {}", self.selected_client_id));
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading(RichText::new("Responses").color(Color32::GREEN));
            ui.add_space(10.0);
            if self.error.0 {
                self.show_response(ui, Some(self.error.1.clone()));
            } else {
                if self.can_show_clients {
                    self.show_response(ui, Some("Clients".to_string()));
                }
                if self.can_show_response {
                    self.show_response(ui, Some("Response".to_string()));
                }
                if self.can_show_file_list {
                    self.show_response(ui, Some("Files".to_string()));
                }
            }

            ui.separator();
            ui.heading(RichText::new("Log Console").color(Color32::YELLOW));
            Frame::none()
                .fill(Color32::from_rgb(30, 30, 30))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for (message, is_outgoing) in &self.log_messages {
                            let color = if *is_outgoing {
                                Color32::LIGHT_BLUE
                            } else {
                                Color32::LIGHT_GREEN
                            };
                            ui.colored_label(color, message);
                        }
                    });
                    if ui.button("Clear Log").clicked() {
                        self.log_messages.clear();
                    }
                });
        });
    }

    pub fn create_command(&mut self) -> String {
        if self
            .communication_server_commands
            .contains(&self.selected_command)
        {
            if self.selected_command.eq("client_list?") {
                Client2_UI::create_simple_command(&self.selected_command, self.selected_server.0)
            } else {
                Client2_UI::create_message_for_command(
                    &self.selected_command,
                    self.selected_client_id,
                    &self.input_text,
                    self.selected_server.0,
                )
            }
        } else {
            if self.selected_command.eq("files_list?") {
                Client2_UI::create_simple_command(&self.selected_command, self.selected_server.0)
            } else {
                Client2_UI::create_complex_command(
                    &self.selected_command,
                    &self.selected_content_id,
                    self.selected_server.0,
                )
            }
        }
    }
    pub fn create_simple_command(selected_c: &String, selected_s: NodeId) -> String {
        let mut r = String::from(selected_c);
        r.push_str("->");
        r.push_str(selected_s.to_string().as_str());
        r
    }
    pub fn create_complex_command(
        selected_c: &String,
        selected_id: &String,
        selected_s: NodeId,
    ) -> String {
        let mut r = String::from(selected_c);
        r.push_str("(");
        r.push_str(selected_id.as_str());
        r.push_str(")");
        r.push_str("->");
        r.push_str(selected_s.to_string().as_str());
        r
    }
    pub fn create_message_for_command(
        selected_c: &String,
        selected_id: NodeId,
        msg: &String,
        selected_s: NodeId,
    ) -> String {
        let mut r = String::from(selected_c);
        r.push_str("(");
        r.push_str(selected_id.to_string().as_str());
        r.push_str(",");
        r.push_str(msg);
        r.push_str(")");
        r.push_str("->");
        r.push_str(selected_s.to_string().as_str());
        r
    }

    pub fn handle_response_show(&mut self, cmd: String) {
        if !cmd.starts_with("server_type!(") {
            self.add_log_message(format!("sent {}", cmd), true);

            // For message_for commands, show immediate confirmation
            if self.selected_command == "message_for?" {
                self.can_show_response = true;
                self.response = format!("✓ Sent to client {}: '{}'",
                                        self.selected_client_id,
                                        self.input_text);
                self.error = (false, String::new());

                // Clear the input field after sending
                self.input_text.clear();

                // Send the actual command
                self.cmd_snd.as_ref().unwrap()
                    .send(cmd)
                    .expect("Failed to send");
                return;
            }
        }

        if self.selected_command.eq("client_list?") {
            self.cmd_snd
                .as_ref()
                .expect("Failed to get value")
                .send(cmd)
                .expect("Failed to send");
            self.can_show_clients = true;
            self.can_show_response = false;
            self.can_show_file_list = false;
        } else if self.selected_command.eq("files_list?") {
            self.cmd_snd
                .as_ref()
                .expect("Failed to get value")
                .send(cmd)
                .expect("Failed to send");
            self.can_show_clients = false;
            self.can_show_response = false;
            self.can_show_file_list = true;
        } else if self.error.0 {
            self.add_log_message(format!("error {}", self.error.1), false);
            self.can_show_clients = false;
            self.can_show_response = false;
            self.can_show_file_list = false;
        } else {
            self.cmd_snd
                .as_ref()
                .expect("Failed to get value")
                .send(cmd)
                .expect("Failed to send");
            self.can_show_clients = false;
            self.can_show_file_list = false;
        }
    }

    pub fn show_response(&mut self, ui: &mut egui::Ui, message: Option<String>) {
        let message = message.expect("Failed to get value");

        if message == "Response" && self.response.starts_with("server_type!(") {
            return;
        }

        if message == "Response" && self.response.starts_with("✓ Sent to client") {
            let frame = Frame::none()
                .fill(Color32::from_rgba_premultiplied(25, 50, 25, 240))
                .rounding(egui::Rounding::same(6))
                .stroke(Stroke::new(1.0, Color32::GREEN))
                .inner_margin(egui::Margin::symmetric(20, 16));

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::GREEN, &self.response);
                });
            });
            return;
        }

        // Common frame settings
        let frame = Frame::none()
            .fill(egui::Color32::from_rgba_premultiplied(25, 25, 35, 240)) // Darker background
            .rounding(egui::Rounding::same(6)) // Smoother rounding
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 70))) // Subtle border
            .inner_margin(egui::Margin::symmetric(20, 16));

        frame.show(ui, |ui| {
            ui.style_mut().override_text_style = Some(TextStyle::Monospace);

            // Header with icon based on message type
            ui.horizontal(|ui| {
                let (icon, color) = match message.as_str() {
                    "Response" => ("🖥️", egui::Color32::LIGHT_GREEN),
                    "Clients" => ("👥", egui::Color32::LIGHT_BLUE),
                    "Files" => ("📁", egui::Color32::LIGHT_YELLOW),
                    _ => ("❌", egui::Color32::RED),
                };

                ui.colored_label(color, icon);
                ui.label(
                    egui::RichText::new(message.as_str())
                        .color(color)
                        .text_style(TextStyle::Heading),
                );
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(12.0);

            // Content area
            match message.as_str() {
                "Response" => {
                    if self.selected_server.1.eq("CommunicationServer") {
                        ui.colored_label(
                            egui::Color32::LIGHT_GREEN,
                            format!(
                                "{}",
                                self.response
                            ),
                        );
                    } else {
                        ui.colored_label(
                            egui::Color32::LIGHT_GREEN,
                            format!("{:?}", self.response)
                        );
                    }
                }
                "Clients" => {
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        format!(
                            "Connected clients: {:?}",
                            self.clients.lock().expect("Failed to lock")
                        ),
                    );
                }
                "Files" => {
                    ui.colored_label(
                        egui::Color32::LIGHT_YELLOW,
                        format!(
                            "Available files: {:?}",
                            self.files_names.lock().expect("Failed to lock")
                        ),
                    );
                }
                _ => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Error: {}", self.error.1)
                    );
                }
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("Clear").fill(egui::Color32::from_rgb(70, 70, 80)))
                    .clicked()
                {
                    match message.as_str() {
                        "Response" => {
                            self.can_show_response = false;
                            self.response = String::new();
                        }
                        "Clients" => {
                            self.can_show_clients = false;
                        }
                        "Files" => {
                            self.can_show_file_list = false;
                        }
                        _ => {
                            self.error.0 = false;
                            self.error.1 = String::new();
                        }
                    }
                }

                // Optional: Add more buttons here if needed
            });
        });
    }
    pub fn string_to_bytes(encoded: String) -> Result<Vec<u8>, String> {
        general_purpose::STANDARD
            .decode(&encoded)
            .map_err(|e| format!("Base64 decode err: {}", e))
    }
    pub fn bytes_to_image(bytes: Vec<u8>) -> Result<ColorImage, String> {
        let image = image::load_from_memory(&bytes)
            .map_err(|e| format!("Image decoding failed: {}", e))?
            .to_rgba8();

        let (w, h) = image.dimensions();
        let pixels = image.into_raw(); // RGBA u8 flat vec
        let color_image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
        Ok(color_image)
    }
}
