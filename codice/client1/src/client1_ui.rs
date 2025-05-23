#[allow(warnings)]
use std::collections::HashMap;
use base64::{engine::general_purpose, Engine as _};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use eframe::egui::{ColorImage, TextureHandle};
use image::GenericImageView;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{Receiver, Sender};
use eframe::epaint::{Stroke, Vec2};
use egui::{Color32, Context, Frame, RichText, TextEdit, TextStyle};
use wg_2024::network::NodeId;

pub struct Client1_UI {
    self_id: NodeId,                    // Node ID for the client
    clients: Arc<Mutex<Vec<NodeId>>>,               // List of clients
    servers: Arc<Mutex<HashMap<NodeId,String>>>,    // List of server names
    files_names: Arc<Mutex<Vec<String>>>,           //Storage of the file names
    selected_server: (NodeId,String),    // Selected server name
    selected_client_id: NodeId, // Selected client ID
    selected_command: String,  // Selected command
    selected_content_id: String,   // Selected file ID
    input_text: String,             // Input text from the user (in message_for cmd)
    cmd_snd: Option<Sender<String>>,    // Command sender
    msg_rcv: Option<Receiver<String>>,   // Message receiver
    can_show_clients: bool,                 //Response handle
    can_show_response: bool,                //Response handle
    can_show_file_list: bool,               //Response handle
    response: String,                       //Response txt
    image: Option<TextureHandle>,           //Response image
    error: (bool,String),                   //Error check
    communication_server_commands: Vec<String>,     //Commands
    text_server_commands: Vec<String>,              //Commands
    media_server_commands: Vec<String>,             //Commands
}

impl Client1_UI {
    pub fn new(self_id: NodeId, clients: Arc<Mutex<Vec<NodeId>>>, servers: Arc<Mutex<HashMap<NodeId, String>>>,files_names: Arc<Mutex<Vec<String>>>, cmd_snd: Sender<String>, msg_rcv: Receiver<String>) -> Self {
        Self {
            self_id,
            clients,
            servers,
            files_names,
            selected_server: (0,String::new()),
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
            error: (false,String::new()),
            text_server_commands: vec!["file?".to_string(),"files_list?".to_string()],
            media_server_commands: vec!["media?".to_string(), "files_list?".to_string()],
            communication_server_commands: vec!["message_for?".to_string(), "client_list?".to_string()]

        }
    }
    pub fn client1_stats(&mut self, ui: &mut egui::Ui, ctx: &Context){

        ui.separator();
        ui.columns(2,|columns|{
            columns[0].set_max_width(250.0);
            columns[1].set_max_width(250.0);

            columns[0].allocate_space(egui::vec2(250.0,0.0));
            columns[0].group(|ui|{

                ui.set_max_width(100.0);
                ui.heading(RichText::new(format!("Commands")).color(Color32::GREEN));
                ui.add_space(10.0);
                ui.horizontal(|ui|{
                    ui.label("Send command to server: ");
                    egui::ComboBox::new("Select server","")
                        .selected_text(format!("{}",self.selected_server.0))
                        .show_ui(ui,|ui| {
                            let s = self.servers.lock().expect("Failed to lock");
                            let servers = s.keys().collect::<Vec<_>>();
                            for server in servers{
                                ui.selectable_value(&mut self.selected_server.0, *server, server.to_string());
                            }
                        });
                    ui.add_space(5.0);
                    if self.selected_server.0 != 0{
                        let server_type = self.servers
                            .lock()
                            .expect("Failed to lock")
                            .get(&self.selected_server.0)
                            .cloned()
                            .expect("Failed to get value")
                            ;
                        ui.label("of type");
                        ui.add_space(5.0);
                        ui.label(RichText::new(format!("{}",server_type)).color(Color32::RED));
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
                                        for command in &self.communication_server_commands {
                                            ui.selectable_value(&mut self.selected_command, command.clone(), command.clone());
                                        }
                                    });
                            }
                            "TextServer" => {
                                egui::ComboBox::new("Select text_server_commands", "")
                                    .selected_text(format!("{}",self.selected_command))
                                    .show_ui(ui, |ui| {
                                        for command in &self.text_server_commands {
                                            ui.selectable_value(&mut self.selected_command, command.clone(), command.clone());
                                        }
                                    });
                            }
                            "MediaServer" => {
                                egui::ComboBox::new("Select media_server_commands", "")
                                    .selected_text(format!("{}",self.selected_command))
                                    .show_ui(ui, |ui| {
                                        for command in &self.media_server_commands {
                                            ui.selectable_value(&mut self.selected_command, command.clone(), command.clone());
                                        }
                                    });
                            }
                            _ => ()
                        }
                        ui.add_space(10.0);
                    });
                    ui.vertical(|ui|{
                        ui.add_space(10.0);
                        match self.selected_command.as_str() {
                            "message_for?" => {
                                if !self.clients.lock().expect("Failed to lock").is_empty(){
                                    ui.horizontal(|ui|{
                                        //self.selected_command = "message_for?".to_string();
                                        ui.label("Write a message:");
                                        ui.add(TextEdit::singleline(&mut self.input_text).desired_width(200.0));
                                        ui.label("to client:");
                                        egui::ComboBox::new("Select client", "")
                                            .selected_text(format!("{}", self.selected_client_id))
                                            .show_ui(ui, |ui| {
                                                let binding = self.clients.lock().expect("Failed to lock");
                                                let clients = binding.iter().clone();
                                                for cl in clients {
                                                    ui.selectable_value(&mut self.selected_client_id, *cl, cl.to_string());
                                                }
                                            });
                                    });
                                }
                                else{
                                    self.error = (true,"No clients available.\nCheck for available clients first".to_string());
                                }

                            }
                            "file?" => {
                                if !self.files_names.lock().expect("Failed to lock").is_empty(){
                                    let binding = self.files_names.lock().expect("Failed to lock");
                                    let ids = binding.iter().clone();
                                    ui.horizontal(|ui|{
                                        ui.label("File");
                                        ui.add_space(10.0);
                                        egui::ComboBox::new("Select file", "")
                                            .selected_text(format!("{}", self.selected_content_id))
                                            .show_ui(ui, |ui| {
                                                for id in ids {
                                                    ui.selectable_value(&mut self.selected_content_id, (*id).clone(), id.to_string());
                                                }
                                            });
                                    });
                                }
                                else{
                                    self.error = (true,"No content available.\nCheck for available files first".to_string());
                                }

                            },
                            "media?" => {
                                if !self.files_names.lock().expect("Failed to lock").is_empty() {
                                    let binding = self.files_names.lock().expect("Failed to lock");
                                    let ids = binding.iter().clone();
                                    ui.horizontal(|ui|{
                                        ui.label("Media");
                                        ui.add_space(10.0);
                                        egui::ComboBox::new("Select media", "")
                                            .selected_text(format!("{}", self.selected_content_id))
                                            .show_ui(ui, |ui| {
                                                for id in ids {
                                                    ui.selectable_value(&mut self.selected_content_id, (*id).clone(), id.to_string());
                                                }
                                            });
                                    });
                                }
                                else{
                                    self.error = (true,"No content available.\nCheck for available media first".to_string());
                                }

                            }
                            "files_list?" => {
                                self.selected_command = "files_list?".to_string();
                            }
                            "client_list?" => {
                                self.selected_command = "client_list?".to_string();
                            }
                            _ => ()
                        }
                    });
                    ui.add_space(30.0);
                    ui.heading(RichText::new(format!("Summary")).color(Color32::GREEN));
                    ui.add_space(10.0);
                    ui.horizontal(|ui|{
                        if self.selected_server.0 != 0{
                            Frame::none()
                                .fill(Color32::LIGHT_GREEN)
                                .inner_margin(egui::Margin::symmetric(8.0, 12.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("Client ID: {}",self.self_id)).color(Color32::BLACK));
                                });
                            ui.allocate_ui(Vec2::new(50.0, 50.0), |ui| {
                                let (rect, response) = ui.allocate_exact_size(Vec2::new(50.0, 50.0), egui::Sense::hover());
                                let painter = ui.painter_at(rect);

                                let start = rect.left_center();
                                let end = rect.right_center();

                                painter.arrow(start, end - start, Stroke::new(4.0, Color32::RED));
                            });
                            Frame::none()
                                .fill(Color32::LIGHT_GREEN)
                                .inner_margin(egui::Margin::symmetric(8.0, 12.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("Server ID: {}",self.selected_server.0)).color(Color32::BLACK));
                                });

                            if self.selected_command.eq("message_for?") && self.selected_client_id != 0{
                                ui.allocate_ui(Vec2::new(50.0, 50.0), |ui| {
                                    let (rect, response) = ui.allocate_exact_size(Vec2::new(50.0, 50.0), egui::Sense::hover());
                                    let painter = ui.painter_at(rect);

                                    let start = rect.left_center();
                                    let end = rect.right_center();

                                    painter.arrow(start, end - start, Stroke::new(4.0, Color32::RED));
                                });
                                Frame::none()
                                    .fill(Color32::LIGHT_GREEN)
                                    .inner_margin(egui::Margin::symmetric(8.0, 12.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(format!("Other client ID: {}",self.selected_client_id)).color(Color32::BLACK));
                                    });
                            }
                        }

                    });
                    ui.add_space(20.0);
                    let cmd = self.create_command();
                    if !self.error.0 && !self.selected_command.eq("Select command"){ // TODO check what is the best choice to do
                        if ui.add_sized(egui::vec2(50.0,50.0),egui::Button::new("SEND")).clicked(){
                            self.handle_response_show(cmd);
                        }
                    }
                }

            });
            columns[1].allocate_space(egui::vec2(250.0,0.0));
            columns[1].group(|ui|{
                ui.heading(RichText::new(format!("Responses")).color(Color32::GREEN));
                ui.add_space(10.0);
                if !self.error.0{
                    if self.can_show_clients{
                        self.show_response(ui,Some("Clients".to_string()),ctx);
                    }
                    if self.can_show_response{
                        self.show_response(ui, Some("Response".to_string()),ctx );
                    }
                    if self.can_show_file_list{
                        self.show_response(ui, Some("Files".to_string()),ctx );
                    }
                }
                else{
                    self.show_response(ui, Some(self.error.1.clone()),ctx);
                }
            });
        });
    }
    pub fn create_command(&mut self) -> String {

        if self.communication_server_commands.contains(&self.selected_command){
            if self.selected_command.eq("client_list?"){
                Client1_UI::create_simple_command(&self.selected_command,self.selected_server.0)
            }
            else{
                Client1_UI::create_message_for_command(&self.selected_command,self.selected_client_id,&self.input_text,self.selected_server.0)
            }
        }
        else {
            if self.selected_command.eq("files_list?"){
                Client1_UI::create_simple_command(&self.selected_command,self.selected_server.0)
            }
            else{
                Client1_UI::create_complex_command(&self.selected_command,&self.selected_content_id,self.selected_server.0)
            }
        }
    }
    pub fn create_simple_command(selected_c: &String, selected_s: NodeId) -> String{
        let mut r = String::from(selected_c);
        r.push_str("->");
        r.push_str(selected_s.to_string().as_str());
        r
    }
    pub fn create_complex_command(selected_c: &String,selected_id: &String,selected_s: NodeId) -> String{
        let mut r = String::from(selected_c);
        r.push_str("(");
        r.push_str(selected_id.as_str());
        r.push_str(")");
        r.push_str("->");
        r.push_str(selected_s.to_string().as_str());
        r
    }
    pub fn create_message_for_command(selected_c: &String,selected_id: NodeId,msg: &String,selected_s: NodeId) -> String{
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
    pub fn handle_response_show(&mut self,cmd: String){
        if self.selected_command.eq("client_list?"){
            self.cmd_snd.as_ref().expect("Failed to get value").send(cmd).expect("Failed to send");
            self.can_show_clients = true;
            self.can_show_response = false;
            self.can_show_file_list = false;
        }
        else if self.selected_command.eq("files_list?"){
            self.cmd_snd.as_ref().expect("Failed to get value").send(cmd).expect("Failed to send");
            self.can_show_clients = false;
            self.can_show_response = false;
            self.can_show_file_list = true;
        }
        else if self.error.0{
            self.can_show_clients = false;
            self.can_show_response = false;
            self.can_show_file_list = false;
        }
        else{
            self.cmd_snd.as_ref().expect("Failed to get value").send(cmd).expect("Failed to send");
            self.can_show_clients = false;
            self.can_show_file_list = false;
            if let Ok(response) = self.msg_rcv.as_ref().expect("Failed to get value").recv() {
                self.can_show_response = true;
                self.response = response;
            }
        }
    }
    pub fn show_response(&mut self, ui: &mut egui::Ui, message: Option<String>, ctx: &Context){

            match message.expect("Failed to get value").as_str() {
                "Response" => {
                    if self.selected_server.1.eq("CommunicationServer") {
                        Frame::none()
                            .fill(Color32::BLACK)
                            .rounding(egui::Rounding::same(3.0))
                            .stroke(egui::Stroke::new(1.0, Color32::DARK_GRAY))
                            .inner_margin(egui::Margin::same(20.0))
                            .show(ui, |ui| {
                                ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                                ui.colored_label(Color32::LIGHT_GREEN, format!("Message from client {} : {}",self.selected_client_id ,self.response));
                                ui.add_space(10.0);
                                if ui.add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear")).clicked() {
                                    self.can_show_response = false;
                                    self.response = String::new();
                                }
                            });
                    } else {
                        Frame::none()
                            .fill(Color32::BLACK)
                            .rounding(egui::Rounding::same(3.0))
                            .stroke(Stroke::new(1.0, Color32::DARK_GRAY))
                            .inner_margin(egui::Margin::same(20.0))
                            .show(ui, |ui| {
                                ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                                ui.colored_label(Color32::LIGHT_GREEN, format!("{:?}", self.response));
                                ui.add_space(10.0);
                                if ui.add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear")).clicked() {
                                    self.can_show_response = false;
                                    self.response = String::new();
                                }
                            });


                    }
                },
                "Clients" => {
                        Frame::none()
                            .fill(Color32::BLACK)
                            .rounding(egui::Rounding::same(3.0))
                            .stroke(egui::Stroke::new(1.0, Color32::DARK_GRAY))
                            .inner_margin(egui::Margin::same(20.0))
                            .show(ui, |ui| {
                                ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                                ui.colored_label(Color32::LIGHT_GREEN, format!("Clients: {:?}", self.clients.lock().expect("Failed to lock")));
                                ui.add_space(10.0);
                                if ui.add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear")).clicked() {
                                    self.can_show_clients = false;
                                }
                            });
                    },
                    "Files" => {
                        Frame::none()
                            .fill(Color32::BLACK)
                            .rounding(egui::Rounding::same(3.0))
                            .stroke(egui::Stroke::new(1.0, Color32::DARK_GRAY))
                            .inner_margin(egui::Margin::same(20.0))
                            .show(ui, |ui| {
                                ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                                ui.colored_label(Color32::LIGHT_GREEN, format!("Files: {:?}", self.files_names.lock().expect("Failed to lock")));
                                ui.add_space(10.0);
                                if ui.add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear")).clicked() {
                                    self.can_show_file_list = false;
                                }
                            });
                    },
                    _ => {
                        Frame::none()
                            .fill(Color32::BLACK)
                            .rounding(egui::Rounding::same(3.0))
                            .stroke(egui::Stroke::new(1.0, Color32::DARK_GRAY))
                            .inner_margin(egui::Margin::same(20.0))
                            .show(ui, |ui| {
                                ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                                ui.colored_label(Color32::RED, format!("{}", self.error.1));
                                ui.add_space(10.0);
                                if ui.add_sized(egui::vec2(50.0, 50.0), egui::Button::new("Clear")).clicked() {
                                    self.error.0 = false;
                                    self.error.1 = String::new();
                                    //self.selected_command = String::new();
                                }
                            });
                    }
                }
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

//TODO check with other client the send message part.
