use crate::logger::logger::{init_logger, write_log};
use common::client_ui::ClientUI;
use crossbeam_channel::{Receiver, Sender};
use eframe::epaint::{Stroke, Vec2};
use egui::{Color32, Frame, RichText, TextEdit, TextStyle};
use egui_extras::RetainedImage;
use rodio::{Decoder, OutputStream, Sink};
#[allow(warnings)]
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, thread};
use wg_2024::network::NodeId;

pub struct Client1_UI {
    self_id: NodeId,                                       // Node ID for the client
    clients: Arc<Mutex<Vec<NodeId>>>,                      // List of clients
    servers: Arc<Mutex<HashMap<NodeId, String>>>,          // List of server names
    files_names: Arc<Mutex<HashMap<NodeId, Vec<String>>>>, //Storage of the file names
    selected_server: (NodeId, String),                     // Selected server name
    selected_client_id: NodeId,                            // Selected client ID
    selected_command: String,                              // Selected command
    selected_content_id: String,                           // Selected file ID
    input_text: String, // Input text from the user (in message_for cmd)
    cmd_snd: Option<Sender<String>>, // Command sender
    msg_rcv: Option<Receiver<String>>, // Message receiver
    can_show_clients: bool, //Response handle
    can_show_response: bool, //Response handle
    can_show_file_list: bool, //Response handle
    response: String,   //Response txt
    error: (bool, String), //Error check
    communication_server_commands: Vec<String>, //Commands
    text_server_commands: Vec<String>, //Commands
    media_server_commands: Vec<String>, //Commands
    image_response: Option<Result<RetainedImage, String>>,
    audio: Arc<Mutex<bool>>,
    clear_clicked: bool
}

impl ClientUI for Client1_UI {
    fn show_ui(&mut self, _frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        self.client1_stats(ui);
    }

    fn get_viewport_id(&self) -> u64 {
        self.self_id.into()
    }
}

impl Client1_UI {
    pub fn new(
        self_id: NodeId,
        clients: Arc<Mutex<Vec<NodeId>>>,
        servers: Arc<Mutex<HashMap<NodeId, String>>>,
        files_names: Arc<Mutex<HashMap<NodeId, Vec<String>>>>,
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
            error: (false, String::new()),
            text_server_commands: vec!["file?".to_string(), "files_list?".to_string()],
            media_server_commands: vec!["media?".to_string(), "files_list?".to_string()],
            communication_server_commands: vec![
                "message_for?".to_string(),
                "client_list?".to_string(),
            ],
            image_response: None,
            audio: Arc::new(Mutex::new(true)),
            clear_clicked : true
        }
    }
    pub fn client1_stats(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        init_logger();
        if let Ok(response) = self
            .msg_rcv
            .as_ref()
            .expect("Failed to get value")
            .try_recv()
        {
            self.can_show_response = true;
            self.response = response;
        }
        ui.columns(2, |columns| {
            columns[0].set_max_width(250.0);
            columns[1].set_max_width(250.0);

            columns[0].allocate_space(egui::vec2(250.0, 0.0));
            columns[0].group(|ui| {
                ui.set_max_width(100.0);
                ui.heading(RichText::new(format!("Commands")).color(Color32::GREEN));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label("Send command to server: ");
                    egui::ComboBox::new("Select server", "")
                        .selected_text(format!("{}", self.selected_server.0))
                        .show_ui(ui, |ui| {
                            let s = self.servers.lock().expect("Failed to lock");
                            let servers = s.keys().collect::<Vec<_>>();
                            if self.clear_clicked{
                                for server in servers {
                                    ui.selectable_value(
                                        &mut self.selected_server.0,
                                        *server,
                                        server.to_string(),
                                    );
                                }
                            }
                            else{
                                ui.colored_label(Color32::RED, "Press 'Clear' to execute commands");
                            }
                        });
                    ui.add_space(5.0);
                    if self.selected_server.0 != 0 {
                        let server_type = self
                            .servers
                            .lock()
                            .expect("Failed to lock")
                            .get(&self.selected_server.0)
                            .cloned()
                            .expect("Failed to get value");
                        ui.label("of type");
                        ui.add_space(5.0);
                        ui.label(RichText::new(format!("{}", server_type)).color(Color32::RED));
                        self.selected_server.1 = server_type;
                    }
                });
                ui.add_space(10.0);
                if self.selected_server.0 != 0 {
                    ui.horizontal(|ui| {
                        ui.label("Command");
                        ui.add_space(10.0);
                        match self.selected_server.1.as_str() {
                            "CommunicationServer" => {
                                egui::ComboBox::new("Select communication_server_command", "")
                                    .selected_text(format!("{}", self.selected_command))
                                    .show_ui(ui, |ui| {
                                        if self.clear_clicked{
                                            for command in &self.communication_server_commands {
                                                ui.selectable_value(
                                                    &mut self.selected_command,
                                                    command.clone(),
                                                    command.clone(),
                                                );
                                            }
                                        }
                                        else{
                                            ui.colored_label(Color32::RED, "Press 'Clear' to execute commands");
                                        }
                                    });
                            }
                            "TextServer" => {
                                egui::ComboBox::new("Select text_server_commands", "")
                                    .selected_text(format!("{}", self.selected_command))
                                    .show_ui(ui, |ui| {
                                        if self.clear_clicked{
                                            for command in &self.text_server_commands {
                                                ui.selectable_value(
                                                    &mut self.selected_command,
                                                    command.clone(),
                                                    command.clone(),
                                                );
                                            }
                                        }
                                        else{
                                            ui.colored_label(Color32::RED, "Press 'Clear' to execute commands");
                                        }
                                    });
                            }
                            "MediaServer" => {
                                egui::ComboBox::new("Select media_server_commands", "")
                                    .selected_text(format!("{}", self.selected_command))
                                    .show_ui(ui, |ui| {
                                        if self.clear_clicked{
                                            for command in &self.media_server_commands {
                                                ui.selectable_value(
                                                    &mut self.selected_command,
                                                    command.clone(),
                                                    command.clone(),
                                                );
                                            }
                                        }
                                        else{
                                            ui.colored_label(Color32::RED, "Press 'Clear' to execute commands");
                                        }
                                    });
                            }
                            _ => (),
                        }
                        ui.add_space(10.0);
                    });
                    ui.vertical(|ui| {
                        ui.add_space(10.0);
                        if self.clear_clicked{
                            match self.selected_command.as_str() {
                                "message_for?" => {
                                    if !self.clients.lock().expect("Failed to lock").is_empty() {
                                        ui.horizontal(|ui| {
                                            //self.selected_command = "message_for?".to_string();
                                            ui.label("Write a message:");
                                            ui.add(
                                                TextEdit::singleline(&mut self.input_text)
                                                    .desired_width(200.0),
                                            );
                                            ui.label("to client:");
                                            egui::ComboBox::new("Select client", "")
                                                .selected_text(format!("{}", self.selected_client_id))
                                                .show_ui(ui, |ui| {
                                                    let binding =
                                                        self.clients.lock().expect("Failed to lock");
                                                    let clients = binding.iter().clone();
                                                    for cl in clients {
                                                        ui.selectable_value(
                                                            &mut self.selected_client_id,
                                                            cl.clone(),
                                                            cl.clone().to_string(),
                                                        );
                                                    }
                                                });
                                        });
                                    } else {
                                        self.error = (
                                            true,
                                            "No clients available.\nCheck for available clients first"
                                                .to_string(),
                                        );
                                    }
                                }
                                "file?" => {
                                    if !self.files_names.lock().expect("Failed to lock").is_empty() {
                                        let files = self.files_names.lock().expect("Failed to lock");
                                        if let Some(binding) = files.get(&self.selected_server.0){
                                            let ids = binding.iter().clone();
                                            ui.horizontal(|ui| {
                                                ui.label("File");
                                                ui.add_space(10.0);
                                                egui::ComboBox::new("Select file", "")
                                                    .selected_text(format!("{}", self.selected_content_id))
                                                    .show_ui(ui, |ui| {
                                                        for id in ids {
                                                            ui.selectable_value(
                                                                &mut self.selected_content_id,
                                                                (*id).clone(),
                                                                id.to_string(),
                                                            );
                                                        }
                                                    });
                                            });
                                        }

                                    } else {
                                        self.error = (
                                            true,
                                            "No content available.\nCheck for available files first"
                                                .to_string(),
                                        );
                                    }
                                }
                                "media?" => {
                                    if !self.files_names.lock().expect("Failed to lock").is_empty() {
                                        let files = self.files_names.lock().expect("Failed to lock");
                                        if let Some(binding) = files.get(&self.selected_server.0){
                                            let ids = binding.iter().clone();
                                            ui.horizontal(|ui| {
                                                ui.label("Media");
                                                ui.add_space(10.0);
                                                egui::ComboBox::new("Select media", "")
                                                    .selected_text(format!("{}", self.selected_content_id))
                                                    .show_ui(ui, |ui| {
                                                        for id in ids {
                                                            ui.selectable_value(
                                                                &mut self.selected_content_id,
                                                                (*id).clone(),
                                                                id.to_string(),
                                                            );
                                                        }
                                                    });
                                            });
                                        }

                                    } else {
                                        self.error = (
                                            true,
                                            "No content available.\nCheck for available media first"
                                                .to_string(),
                                        );
                                    }
                                }
                                "files_list?" => {
                                    self.selected_command = "files_list?".to_string();
                                }
                                "client_list?" => {
                                    self.selected_command = "client_list?".to_string();
                                }
                                _ => (),
                            }
                        }

                    });
                    ui.add_space(30.0);
                    ui.heading(RichText::new(format!("Summary")).color(Color32::GREEN));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if self.selected_server.0 != 0 {
                            Frame::none()
                                .fill(Color32::LIGHT_GREEN)
                                .inner_margin(egui::Margin::symmetric(8, 12))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("Client ID: {}", self.self_id))
                                            .color(Color32::BLACK),
                                    );
                                });
                            ui.allocate_ui(Vec2::new(50.0, 50.0), |ui| {
                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::new(50.0, 50.0),
                                    egui::Sense::hover(),
                                );
                                let painter = ui.painter_at(rect);

                                let start = rect.left_center();
                                let end = rect.right_center();

                                painter.arrow(start, end - start, Stroke::new(4.0, Color32::RED));
                            });
                            Frame::none()
                                .fill(Color32::LIGHT_GREEN)
                                .inner_margin(egui::Margin::symmetric(8, 12))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "Server ID: {}",
                                            self.selected_server.0
                                        ))
                                        .color(Color32::BLACK),
                                    );
                                });

                            if self.selected_command.eq("message_for?")
                                && self.selected_client_id != 0
                            {
                                ui.allocate_ui(Vec2::new(50.0, 50.0), |ui| {
                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(50.0, 50.0),
                                        egui::Sense::hover(),
                                    );
                                    let painter = ui.painter_at(rect);

                                    let start = rect.left_center();
                                    let end = rect.right_center();

                                    painter.arrow(
                                        start,
                                        end - start,
                                        Stroke::new(4.0, Color32::RED),
                                    );
                                });
                                Frame::none()
                                    .fill(Color32::LIGHT_GREEN)
                                    .inner_margin(egui::Margin::symmetric(8, 12))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "Other client ID: {}",
                                                self.selected_client_id
                                            ))
                                            .color(Color32::BLACK),
                                        );
                                    });
                            }
                        }
                    });
                    ui.add_space(20.0);
                    let cmd = self.create_command();
                    if !self.error.0 && !self.selected_command.eq("Select command") {
                        if self.clear_clicked{
                            if ui
                                .add_sized(egui::vec2(50.0, 50.0), egui::Button::new("SEND"))
                                .clicked()
                            {
                                self.handle_response_show(cmd);
                                self.clear_clicked = false;
                            }
                        }
                    }
                }
            });
            columns[1].allocate_space(egui::vec2(250.0, 0.0));
            columns[1].group(|ui| {
                ui.heading(RichText::new(format!("Responses")).color(Color32::GREEN));
                ui.add_space(10.0);
                if !self.error.0 {
                    if self.can_show_clients {
                        let content = self.retrieve_content("Clients", self.selected_server.0);
                        self.show_response(ui, content);
                    }
                    if self.can_show_response {
                        let content = self.retrieve_content("Response", self.selected_server.0);
                        self.show_response(ui, content);
                    }
                    if self.can_show_file_list {
                        let content = self.retrieve_content("Files", self.selected_server.0);
                        self.show_response(ui, content);
                    }
                } else {
                    let content = self.retrieve_content("Error", self.selected_server.0);
                    self.show_response(ui, content);
                }
            });
        });
    }

    pub fn handle_response_show(&mut self, cmd: String) {
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
            if let Ok(response) = self
                .msg_rcv
                .as_ref()
                .expect("Failed to get value")
                .try_recv()
            {
                self.can_show_response = true;
                self.response = response;
            }
        }
    }
    pub fn show_response(&mut self, ui: &mut egui::Ui, content: String) {
        if self.selected_server.1.eq("MediaServer") && self.selected_command.eq("media?") {
            Frame::none()
                .fill(Color32::BLACK)
                .rounding(egui::Rounding::same(3))
                .stroke(egui::Stroke::new(1.0, Color32::DARK_GRAY))
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                    if self.selected_content_id.ends_with(".mp3") {
                        ui.colored_label(Color32::LIGHT_GREEN, "Playing audio");

                        let path = env::current_dir().expect("Failed to get current_dir value");
                        let mut file_path = path;
                        let path2 = self.selected_content_id.clone();
                        file_path.push(path2.as_str());

                        let audio_on = Arc::clone(&self.audio);
                        thread::spawn(move || {
                            let (_stream, stream_handle) = OutputStream::try_default()
                                .expect("Failed to get the default audio output stream");
                            //write_log(file_path.as_path().to_str().expect("Failed to convert"));
                            let audio = File::open(
                                file_path.as_path().to_str().expect("Failed to convert"),
                            )
                            .expect("Failed to open audio file");
                            let reader = BufReader::new(audio);
                            let src = Decoder::new(reader).expect("Failed to decode audio file");

                            let sink =
                                Sink::try_new(&stream_handle).expect("Failed to create audio sink");
                            sink.append(src);

                            while *audio_on.lock().expect("Failed to lock"){
                                if sink.empty(){
                                    break;
                                }
                                thread::sleep(Duration::from_millis(100));
                            }
                            sink.stop();
                            /*
                            loop {
                                // If the flag is false, stop the sink.
                                if !*audio_on.lock().expect("Failed to lock") {
                                    sink.stop();
                                    break;
                                }

                                // Exit loop if playback is finished.
                                if sink.empty() {
                                    break;
                                }
                            }
                             */
                        });
                    } else {
                        self.load_image();
                        if let Some(image_result) = &self.image_response {
                            match image_result {
                                Ok(retained_image) => {
                                    // Happy path: The image was loaded and parsed successfully.
                                    retained_image.show_size(ui, ui.available_size());
                                }
                                Err(error_message) => {
                                    // Error path: Display the error message in the UI.
                                    ui.colored_label(ui.visuals().error_fg_color, error_message);
                                }
                            }
                        }
                    }
                    if ui
                        .add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear"))
                        .clicked()
                    {
                        let mut audio = self.audio.lock().expect("Failed to lock");
                        *audio = false;
                        self.error.0 = false;
                        self.error.1 = String::new();
                        self.can_show_clients = false;
                        self.can_show_file_list = false;
                        self.can_show_response = false;
                        self.response = String::new();
                        self.clear_clicked = true;
                    }
                });
        } else {
            Frame::none()
                .fill(Color32::BLACK)
                .rounding(egui::Rounding::same(3))
                .stroke(egui::Stroke::new(1.0, Color32::DARK_GRAY))
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                    let color = if self.error.0 {
                        Color32::RED
                    } else {
                        Color32::LIGHT_GREEN
                    };
                    ui.colored_label(color, content);
                    ui.add_space(10.0);
                    if ui
                        .add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear"))
                        .clicked()
                    {
                        self.reset_fields_state();
                    }
                });
        }
    }
    pub fn reset_fields_state(&mut self){
        self.error.0 = false;
        self.error.1 = String::new();
        self.can_show_clients = false;
        self.can_show_file_list = false;
        self.can_show_response = false;
        self.response = String::new();
        self.clear_clicked = true;
    }
    pub fn retrieve_content(&self, content_type: &str, id: NodeId) -> String {
        match content_type {
            "Clients" => {
                format!(
                    "Clients: {:?}",
                    self.clients.lock().expect("Failed to lock")
                )
            }
            "Files" => {
                if let Some(value) = self.files_names.lock().expect("Failed to lock").get(&id) {
                    format!("Files: {:?}", value)
                } else {
                    format!("")
                }
            }
            "Response" => {
                format!("{}", self.response)
            }
            "Error" => {
                format!("Error: {}", self.error.1)
            }
            _ => "Error: not a valid content type".to_string(),
        }
    }
    pub fn create_command(&mut self) -> String {
        if self
            .communication_server_commands
            .contains(&self.selected_command)
        {
            if self.selected_command.eq("client_list?") {
                Client1_UI::create_simple_command(&self.selected_command, self.selected_server.0)
            } else {
                Client1_UI::create_message_for_command(
                    &self.selected_command,
                    self.selected_client_id,
                    &self.input_text,
                    self.selected_server.0,
                )
            }
        } else {
            if self.selected_command.eq("files_list?") {
                Client1_UI::create_simple_command(&self.selected_command, self.selected_server.0)
            } else {
                Client1_UI::create_complex_command(
                    &self.selected_command,
                    &self.selected_content_id,
                    self.selected_server.0,
                )
            }
        }
    }
    pub fn load_image(&mut self) {
        let path = env::current_dir().expect("Failed to get current_dir value");
        let mut file_path = path;
        let path = self.selected_content_id.clone();
        file_path.push(path.as_str());

        let image_result =
            match std::fs::read(file_path.as_path().to_str().expect("Failed to convert")) {
                Ok(bytes) => {
                    RetainedImage::from_image_bytes(file_path.to_string_lossy().to_string(), &bytes)
                }

                Err(e) => {
                    // Failed to read the file.
                    let error_message = format!("Failed to read file: {}", e);
                    // Log the error to the console for easier debugging.
                    eprintln!("{}", error_message);
                    Err(error_message)
                }
            };
        self.image_response = Some(image_result);
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
}
